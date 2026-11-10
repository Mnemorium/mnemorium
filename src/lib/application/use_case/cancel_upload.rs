use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use tracing::{info, warn};

use crate::application::port::cancel_upload::{
    CancelUploadCommand, CancelUploadError, CancelUploadUseCase,
};
use crate::application::use_case::upload_layout::staged_path;
use crate::application::use_case::upload_session::expiry;
use crate::domain::alias::NumericID;
use crate::domain::model::permission::Action;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::unit_of_work::{UnitOfWork as _, UnitOfWorkFactory};
use crate::domain::port::upload_repository::{UploadFilter, UploadRepository as _};
use crate::domain::port::user_repository::{UserFilter, UserRepository as _};
use crate::domain::port::user_unit_of_work::UserUnitOfWork;
use crate::domain::service::authorization::{Authorization, Decision};

/// Outcome of the business logic wrapped by the unit-of-work lifecycle.
enum Flow<T, E> {
    /// The business logic failed and the unit of work must be rolled back.
    Failed(E),
    /// The upload expired: its row and staged file have been lazily deleted, so
    /// the unit of work must be committed before reporting the session as
    /// unknown.
    Reaped,
    /// The business logic succeeded.
    Succeeded(T),
}

/// Use case implementation for cancelling an upload session.
pub(crate) struct CancelUpload<F, S> {
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter deleting the staged file.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the cancellation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> CancelUpload<F, S> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        expiry_seconds: u64,
    ) -> Self {
        Self {
            expiry_seconds,
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> CancelUploadUseCase for CancelUpload<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + UserUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: CancelUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), CancelUploadError>> + Send + 'future>> {
        let expiry_seconds = self.expiry_seconds;
        let file_storage = Arc::clone(&self.file_storage);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            // The unit of work is write-intent: cancelling deletes the session
            // row and an expired session is removed on the way out.
            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| CancelUploadError::Unknown(error.into()))?;

            let flow = async {
                let caller = match unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                {
                    Ok(found) => match found.into_iter().next() {
                        Some(caller) => caller,
                        None => return Flow::Failed(CancelUploadError::NoSuchCaller),
                    },
                    Err(error) => return Flow::Failed(CancelUploadError::Unknown(error.into())),
                };

                let found = match unit_of_work
                    .uploads()
                    .search(&UploadFilter {
                        upload_id: Some(command.upload_id()),
                        ..UploadFilter::default()
                    })
                    .await
                {
                    Ok(found) => found,
                    Err(error) => return Flow::Failed(CancelUploadError::Unknown(error.into())),
                };
                let Some(upload) = found.into_iter().next() else {
                    return Flow::Failed(CancelUploadError::NoSuchUpload);
                };

                // Owner-only: a caller who is not the upload's owner must not
                // see it. The denial is a security decision the use case owns,
                // so it logs the catalogued `authz_failed` event once
                // (`OBS-002`, `OBS-006`); the caller still receives the
                // not-found result, never a disclosure of the owner.
                if let Decision::Deny(denial) =
                    Authorization::decide(&caller, &[], Action::UploadWrite, &upload)
                {
                    warn!(
                        target: "security",
                        event = "authz_failed",
                        actor = caller.id(),
                        action = %denial.action().verb(),
                        target = %denial.action().target(),
                        "denied an action"
                    );
                    return Flow::Failed(CancelUploadError::NoSuchUpload);
                }

                let expires_at = match expiry(expiry_seconds, upload.created_at()) {
                    Ok(expires_at) => expires_at,
                    Err(error) => return Flow::Failed(CancelUploadError::Unknown(error)),
                };
                if !upload.is_finished() && Utc::now().naive_utc() > expires_at {
                    // The reaper removes an expired session in the background;
                    // this lazy delete releases the row and the staged file on
                    // the next access, and the caller sees the same `404` as for
                    // an unknown session.
                    discard_staged(&file_storage, command.upload_id()).await;
                    if unit_of_work
                        .uploads()
                        .delete(command.upload_id())
                        .await
                        .is_err()
                    {
                        // The repository adapter owns the failure log (`OBS-002`).
                    }
                    return Flow::Reaped;
                }

                // A finished session's content is already the caller's library
                // content, and a session being verified is mid-completion:
                // neither can be cancelled.
                if upload.is_finished() || upload.is_verifying() {
                    return Flow::Failed(CancelUploadError::NotCancellable);
                }

                // The staged content is removed before the row so no session
                // ever points at a file that is already gone; a failed deletion
                // is best-effort and the reaper removes the orphan later.
                discard_staged(&file_storage, upload.upload_id()).await;
                match unit_of_work.uploads().delete(upload.upload_id()).await {
                    Ok(true) => Flow::Succeeded(()),
                    Ok(false) => Flow::Failed(CancelUploadError::NoSuchUpload),
                    Err(error) => Flow::Failed(CancelUploadError::Unknown(error.into())),
                }
            }
            .await;

            match flow {
                Flow::Succeeded(()) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CancelUploadError::Unknown(error.into()))?;
                    info!(
                        target: "security",
                        event = "upload",
                        actor = command.user_id(),
                        upload = %command.upload_id().to_string(),
                        outcome = %"cancelled",
                        "upload activity"
                    );
                    Ok(())
                }
                Flow::Reaped => {
                    // The expired upload row and staged file were already
                    // deleted inside the transaction; commit so the deletion
                    // survives, then report the session as unknown. A commit
                    // failure is a server error and takes precedence.
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CancelUploadError::Unknown(error.into()))?;
                    Err(CancelUploadError::NoSuchUpload)
                }
                Flow::Failed(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    Err(error)
                }
            }
        })
    }
}

/// Best-effort delete the staged file of `upload_id`.
///
/// A missing file is success, and the file-storage adapter owns the failure log
/// (`OBS-002`), so a failed deletion is swallowed here.
async fn discard_staged<S>(file_storage: &Arc<S>, upload_id: NumericID)
where
    S: FileStorage,
{
    if file_storage
        .delete_upload_file(&staged_path(upload_id))
        .await
        .is_err()
    {
        // The file-storage adapter owns the failure log (`OBS-002`).
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::use_case::test_fixtures::TTL_SECONDS;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::upload::{ChunkBitmap, Upload};
    use crate::domain::port::error::{RepositoryError, StorageError};
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::upload_repository::MockUploadRepository;
    use crate::test_helpers::*;
    use chrono::NaiveDateTime;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};

    type UseCase = CancelUpload<TestFactory, MockFileStorage>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    fn use_case_with(uploads: MockUploadRepository, file_storage: MockFileStorage) -> Harness {
        let unit_of_work = TestUnitOfWork {
            users: any_caller(),
            uploads,
            ..TestUnitOfWork::default()
        };
        let harness = AssetFactoryHarness::new(unit_of_work);
        Harness {
            use_case: CancelUpload::new(
                Arc::clone(&harness.factory),
                Arc::new(file_storage),
                TTL_SECONDS,
            ),
            committed: harness.committed,
            rolled_back: harness.rolled_back,
        }
    }

    fn timestamp() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    fn expire(moment: NaiveDateTime) -> NaiveDateTime {
        let ttl_seconds = i64::try_from(TTL_SECONDS).unwrap_or(i64::MAX);
        moment
            .checked_sub_signed(chrono::Duration::seconds(ttl_seconds))
            .and_then(|instant| instant.checked_sub_signed(chrono::Duration::seconds(1)))
            .unwrap_or(moment)
    }

    fn upload(
        user_id: i64,
        is_finished: bool,
        is_verifying: bool,
        created_at: NaiveDateTime,
    ) -> Result<Upload, RepositoryError> {
        let mut bitmap = ChunkBitmap::try_new(1).map_err(|_| RepositoryError::OperationFailed)?;
        bitmap
            .mark_received(0)
            .map_err(|_| RepositoryError::OperationFailed)?;
        Upload::try_new(
            5,
            user_id,
            "clip.mp4".to_owned(),
            4,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            is_finished,
            is_verifying,
            None,
            created_at,
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    fn expect_upload(uploads: &mut MockUploadRepository, upload: Upload) {
        uploads.expect_search().times(1).returning(move |_| {
            let stored = upload.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
    }

    fn expect_staged_delete(file_storage: &mut MockFileStorage) {
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
    }

    #[tokio::test]
    async fn cancel_upload_pending_session_deletes_the_session_and_its_content()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(3, false, false, timestamp())?);
        uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        expect_staged_delete(&mut file_storage);
        let harness = use_case_with(uploads, file_storage);

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_upload_unknown_session_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(uploads, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(result, Err(CancelUploadError::NoSuchUpload)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_upload_other_user_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(3, false, false, timestamp())?);
        let harness = use_case_with(uploads, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 99))
            .await;

        // Assert
        assert!(matches!(result, Err(CancelUploadError::NoSuchUpload)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_upload_finished_session_returns_not_cancellable() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(3, true, false, timestamp())?);
        let harness = use_case_with(uploads, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 3))
            .await;

        // Assert: the completed file is untouched and the session survives.
        assert!(matches!(result, Err(CancelUploadError::NotCancellable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_upload_verifying_session_returns_not_cancellable() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(3, false, true, timestamp())?);
        let harness = use_case_with(uploads, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(result, Err(CancelUploadError::NotCancellable)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_upload_expired_session_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(3, false, false, expire(timestamp()))?);
        uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        expect_staged_delete(&mut file_storage);
        let harness = use_case_with(uploads, file_storage);

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 3))
            .await;

        // Assert: the expired session is removed and reported as unknown.
        assert!(matches!(result, Err(CancelUploadError::NoSuchUpload)));
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_upload_staged_delete_failure_still_cancels() -> Result<(), Box<dyn Error>> {
        // Arrange: a failed staged-file deletion is best-effort; the reaper
        // removes the orphan.
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(3, false, false, timestamp())?);
        uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Err(StorageError::OperationFailed) }));
        let harness = use_case_with(uploads, file_storage);

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn cancel_upload_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let harness = use_case_with(uploads, MockFileStorage::new());

        // Act
        let result = harness
            .use_case
            .execute(CancelUploadCommand::new(5, 3))
            .await;

        // Assert
        assert!(matches!(result, Err(CancelUploadError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
