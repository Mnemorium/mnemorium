use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use tracing::error;

use crate::application::port::complete_upload::CompleteUploadCommand;
use crate::application::port::complete_upload::CompleteUploadError;
use crate::application::port::complete_upload::CompleteUploadResponse;
use crate::application::port::complete_upload::CompleteUploadUseCase;
use crate::application::use_case::upload_session::expiry;
use crate::domain::alias::NumericID;
use crate::domain::model::file::File;
use crate::domain::model::upload::Upload;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_repository::FileFilter;
use crate::domain::port::file_repository::FileRepository as _;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::UploadFilter;
use crate::domain::port::upload_repository::UploadRepository as _;

/// Outcome of the business logic wrapped by the unit-of-work lifecycle.
enum Flow<T, E> {
    /// The upload expired: its row and staged file have been deleted, so the
    /// unit of work must be committed before returning the error.
    Expired,
    /// The business logic failed and the unit of work must be rolled back.
    Failed(E),
    /// The business logic succeeded.
    Succeeded(T),
}

/// Use case implementation for completing an upload session.
pub struct CompleteUpload<F, S> {
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter promoting the staged file.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the completion.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> CompleteUpload<F, S> {
    /// Create a new use case.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<F>, file_storage: Arc<S>, expiry_seconds: u64) -> Self {
        Self {
            expiry_seconds,
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> CompleteUploadUseCase for CompleteUpload<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: CompleteUploadCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CompleteUploadResponse, CompleteUploadError>>
                + Send
                + 'future,
        >,
    > {
        let expiry_seconds = self.expiry_seconds;
        let file_storage = Arc::clone(&self.file_storage);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
            let mut promoted: Option<(NumericID, String)> = None;

            let flow = async {
                let found = match unit_of_work
                    .uploads()
                    .search(&UploadFilter {
                        id: Some(command.upload_id()),
                        ..UploadFilter::default()
                    })
                    .await
                {
                    Ok(found) => found,
                    Err(error) => {
                        return Flow::Failed(CompleteUploadError::Unknown(error.into()));
                    }
                };
                let Some(upload) = found.into_iter().next() else {
                    return Flow::Failed(CompleteUploadError::NoSuchUpload);
                };

                // Owner-only: a caller who is not the upload's owner must not see it.
                if upload.user_id() != command.user_id() {
                    return Flow::Failed(CompleteUploadError::NoSuchUpload);
                }

                let expires_at = match expiry(expiry_seconds, upload.created_at()) {
                    Ok(expires_at) => expires_at,
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error)),
                };
                if !upload.is_finished() && Utc::now().naive_utc() > expires_at {
                    return expire(&mut unit_of_work, &file_storage, command.upload_id()).await;
                }

                let existing = find_file(&mut unit_of_work, &upload, command.user_id()).await;

                // Idempotent completion: an already finished upload returns the
                // caller's file for the same digest.
                if upload.is_finished() {
                    return match existing {
                        Ok(Some(file)) => {
                            Flow::Succeeded(CompleteUploadResponse::new(file.id(), true))
                        }
                        Ok(None) => Flow::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                            "the finished upload has no matching file"
                        ))),
                        Err(error) => Flow::Failed(error),
                    };
                }

                if !upload.can_finish() {
                    return Flow::Failed(CompleteUploadError::Incomplete);
                }

                // Deduplication is caller-scoped and resolved before the staged
                // file is promoted, so a cross-user conflict never leaves a
                // promoted orphan behind.
                let matching_file = match existing {
                    Ok(deduplicated) => deduplicated,
                    Err(error) => return Flow::Failed(error),
                };

                if let Some(own_file) = matching_file {
                    // The caller already owns a file with this digest, so no new
                    // file is created and the session is left open. The staged
                    // content is still verified so a completion never reports
                    // success for bytes that do not match the declared digest;
                    // the staged file is left for the reaper.
                    if let Err(error) = verify_integrity(&file_storage, &upload).await {
                        return Flow::Failed(error);
                    }
                    return Flow::Succeeded(CompleteUploadResponse::new(own_file.id(), false));
                }

                if let Err(error) = verify_integrity(&file_storage, &upload).await {
                    return Flow::Failed(error);
                }

                let path = match file_storage
                    .promote(upload.upload_id(), upload.file_name())
                    .await
                {
                    Ok(path) => path,
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error.into())),
                };
                promoted = Some((upload.upload_id(), upload.file_name().to_owned()));

                let pending = match File::try_new(
                    0,
                    path,
                    command.user_id(),
                    false,
                    upload.mime_type_id().to_owned(),
                    Utc::now().date_naive(),
                    upload.integrity_hash().clone(),
                ) {
                    Ok(pending) => pending,
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error.into())),
                };
                let file = match unit_of_work.files().create(pending).await {
                    Ok(file) => file,
                    Err(error) => return Flow::Failed(CompleteUploadError::Unknown(error.into())),
                };

                let mut finished = upload;
                finished.finish();
                match unit_of_work.uploads().save(finished).await {
                    Ok(Some(_)) => Flow::Succeeded(CompleteUploadResponse::new(file.id(), true)),
                    Ok(None) => Flow::Failed(CompleteUploadError::NoSuchUpload),
                    Err(RepositoryError::ConcurrentModification) => {
                        Flow::Failed(CompleteUploadError::Unknown(anyhow::anyhow!(
                            "the upload was modified while completing"
                        )))
                    }
                    Err(error) => Flow::Failed(CompleteUploadError::Unknown(error.into())),
                }
            }
            .await;

            match flow {
                Flow::Succeeded(value) => {
                    if let Err(error) = unit_of_work.commit().await {
                        restore_promoted(&file_storage, promoted.as_ref()).await;
                        return Err(CompleteUploadError::Unknown(error.into()));
                    }
                    Ok(value)
                }
                Flow::Expired => {
                    // The expired upload row and staged file were already
                    // deleted inside the transaction; commit so the deletion
                    // survives, then report the expiry. A commit failure is a
                    // server error and takes precedence.
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
                    Err(CompleteUploadError::Expired)
                }
                Flow::Failed(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the complete upload unit of work"
                        );
                    }
                    restore_promoted(&file_storage, promoted.as_ref()).await;
                    Err(error)
                }
            }
        })
    }
}

/// Best-effort delete the expired upload's staged file and row, then signal the
/// caller to commit before returning the expiry error.
///
/// The row deletion is left to the reaper once one exists.
#[expect(
    clippy::single_call_fn,
    reason = "the expiry cleanup is named after the rule it enforces"
)]
async fn expire<U, S>(
    unit_of_work: &mut U,
    file_storage: &Arc<S>,
    upload_id: i64,
) -> Flow<CompleteUploadResponse, CompleteUploadError>
where
    U: AssetUnitOfWork,
    S: FileStorage,
{
    // TODO(reaper): move the expiry cleanup to a background task; this lazy
    // delete keeps the row and the staged file only until the next access.
    if let Err(error) = file_storage.delete_upload_file(upload_id).await {
        error!(error = ?error, "failed to delete the expired upload staged file");
    }
    if let Err(error) = unit_of_work.uploads().delete(upload_id).await {
        error!(error = ?error, "failed to delete the expired upload row");
    }
    Flow::Expired
}

/// Find the caller's file matching the upload digest.
///
/// # Errors
///
/// Returns [`CompleteUploadError::Conflict`] when the digest is already owned by
/// another user, and [`CompleteUploadError::Unknown`] on a repository failure.
#[expect(
    clippy::single_call_fn,
    reason = "the caller-scoped deduplication lookup is named for readability"
)]
async fn find_file<U>(
    unit_of_work: &mut U,
    upload: &Upload,
    user_id: i64,
) -> Result<Option<File>, CompleteUploadError>
where
    U: AssetUnitOfWork,
{
    let files = unit_of_work
        .files()
        .search(&FileFilter {
            integrity_hash: Some(upload.integrity_hash().as_str().to_owned()),
            ..FileFilter::default()
        })
        .await
        .map_err(|error| CompleteUploadError::Unknown(error.into()))?;

    let mut own_file = None;
    for file in files {
        if file.user_id() == user_id {
            if own_file.is_none() {
                own_file = Some(file);
            }
        } else {
            return Err(CompleteUploadError::Conflict);
        }
    }
    Ok(own_file)
}

/// Recompute the staged file's integrity hash and compare it with the hash
/// declared when the upload was initialized.
///
/// # Errors
///
/// Returns [`CompleteUploadError::IntegrityMismatch`] when the staged content
/// does not hash to the declared digest, and [`CompleteUploadError::Unknown`]
/// when the digest cannot be computed.
async fn verify_integrity<S>(
    file_storage: &Arc<S>,
    upload: &Upload,
) -> Result<(), CompleteUploadError>
where
    S: FileStorage,
{
    let computed = file_storage
        .integrity_hash(upload.upload_id())
        .await
        .map_err(|error| CompleteUploadError::Unknown(error.into()))?;
    if computed != *upload.integrity_hash() {
        return Err(CompleteUploadError::IntegrityMismatch);
    }
    Ok(())
}

/// Best-effort move a promoted file back to its staging path after a failed
/// completion, so the upload can be retried and no final file is orphaned.
async fn restore_promoted<S>(file_storage: &Arc<S>, promoted: Option<&(NumericID, String)>)
where
    S: FileStorage,
{
    let Some(identity) = promoted else {
        return;
    };
    if let Err(error) = file_storage.restore(identity.0, identity.1.as_str()).await {
        // TODO(reaper): a failed restore leaves the final file orphaned; delete
        // it once a background reaper exists.
        error!(error = ?error, "failed to restore the promoted file of an incomplete upload");
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::complete_upload::CompleteUploadCommand;
    use crate::application::port::complete_upload::CompleteUploadError;
    use crate::application::port::complete_upload::CompleteUploadUseCase as _;
    use crate::domain::model::file::File;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::upload_repository::MockUploadRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::asset_factory;

    use super::CompleteUpload;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const TTL_SECONDS: u64 = 3600;
    const CHUNK_SIZE: u64 = 4;

    type UseCase = CompleteUpload<TestFactory, MockFileStorage>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set to force `commit` to fail.
        commit_fails: Arc<AtomicBool>,
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    fn use_case_with(
        uploads: MockUploadRepository,
        files: MockFileRepository,
        file_storage: MockFileStorage,
    ) -> Harness {
        let harness = asset_factory(uploads, files, MockMimeTypeRepository::new());
        Harness {
            use_case: CompleteUpload::new(
                Arc::clone(&harness.factory),
                Arc::new(file_storage),
                TTL_SECONDS,
            ),
            committed: harness.committed,
            commit_fails: harness.commit_fails,
            rolled_back: harness.rolled_back,
        }
    }

    fn timestamp() -> NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    fn upload(
        upload_id: i64,
        user_id: i64,
        file_size: u64,
        received: &[usize],
        is_finished: bool,
    ) -> Result<Upload, RepositoryError> {
        let total_chunks = usize::try_from(file_size.div_ceil(CHUNK_SIZE)).unwrap_or(1);
        let mut bitmap = ChunkBitmap::try_new(total_chunks.max(1))
            .map_err(|_| RepositoryError::OperationFailed)?;
        for chunk_number in received {
            bitmap
                .mark_received(*chunk_number)
                .map_err(|_| RepositoryError::OperationFailed)?;
        }
        Upload::try_new(
            upload_id,
            user_id,
            "clip.mp4".to_owned(),
            file_size,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            is_finished,
            0,
            timestamp(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    fn expect_upload(uploads: &mut MockUploadRepository, upload: Upload) {
        uploads.expect_search().times(1).returning(move |_| {
            let stored = upload.clone();
            Box::pin(async move { Ok(vec![stored]) })
        });
    }

    fn stored_file(id: i64, user_id: i64) -> Result<File, RepositoryError> {
        File::try_new(
            id,
            format!("files/{id}_clip.mp4"),
            user_id,
            false,
            "video/mp4".to_owned(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default(),
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    fn expect_integrity_hash(file_storage: &mut MockFileStorage, digest: &'static str) {
        file_storage
            .expect_integrity_hash()
            .times(1)
            .returning(move |_| {
                let hash = IntegrityHash::try_new(digest.to_owned())
                    .map_err(|_| StorageError::OperationFailed);
                Box::pin(async move { hash })
            });
    }

    #[tokio::test]
    async fn complete_upload_complete_upload_promotes_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        files.expect_create().times(1).returning(|mut file| {
            file.set_is_public(false);
            Box::pin(async move { Ok(file) })
        });
        uploads
            .expect_save()
            .times(1)
            .returning(|upload| Box::pin(async move { Ok(Some(upload)) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("files/5_clip.mp4".to_owned()) }));
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 0);
        assert!(response.is_finished());
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_vanished_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        uploads
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Ok(None) }));
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("files/5_clip.mp4".to_owned()) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_integrity_hash_mismatch_returns_integrity_mismatch()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(
            &mut file_storage,
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        );
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(CompleteUploadError::IntegrityMismatch)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_unknown_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(uploads, MockFileRepository::new(), MockFileStorage::new());
        let command = CompleteUploadCommand::new(999, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_other_user_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let harness = use_case_with(uploads, MockFileRepository::new(), MockFileStorage::new());
        let command = CompleteUploadCommand::new(5, 99);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::NoSuchUpload)));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_expired_upload_returns_expired() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expired_at = chrono::Utc::now()
            .naive_utc()
            .checked_sub_signed(chrono::Duration::seconds(
                i64::try_from(TTL_SECONDS).unwrap_or(0) + 1,
            ))
            .unwrap_or_else(timestamp);
        let mut bitmap = ChunkBitmap::try_new(1).map_err(|_| RepositoryError::OperationFailed)?;
        bitmap
            .mark_received(0)
            .map_err(|_| RepositoryError::OperationFailed)?;
        let expired = Upload::try_new(
            5,
            3,
            "clip.mp4".to_owned(),
            4,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            false,
            0,
            expired_at,
        )
        .map_err(|_| RepositoryError::OperationFailed)?;
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, expired);
        uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = use_case_with(uploads, MockFileRepository::new(), file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Expired)));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_expiry_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expired_at = chrono::Utc::now()
            .naive_utc()
            .checked_sub_signed(chrono::Duration::seconds(
                i64::try_from(TTL_SECONDS).unwrap_or(0) + 1,
            ))
            .unwrap_or_else(timestamp);
        let mut bitmap = ChunkBitmap::try_new(1).map_err(|_| RepositoryError::OperationFailed)?;
        bitmap
            .mark_received(0)
            .map_err(|_| RepositoryError::OperationFailed)?;
        let expired = Upload::try_new(
            5,
            3,
            "clip.mp4".to_owned(),
            4,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            false,
            0,
            expired_at,
        )
        .map_err(|_| RepositoryError::OperationFailed)?;
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, expired);
        uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let harness = use_case_with(uploads, MockFileRepository::new(), file_storage);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_incomplete_upload_returns_incomplete() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[0], false)?);
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(uploads, files, MockFileStorage::new());
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Incomplete)));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_incomplete_with_cross_user_digest_returns_incomplete()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[0], false)?);
        let mut files = MockFileRepository::new();
        files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 42);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let harness = use_case_with(uploads, files, MockFileStorage::new());
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Incomplete)));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_idempotent_finished_upload_returns_own_file()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], true)?);
        let mut files = MockFileRepository::new();
        files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let harness = use_case_with(uploads, files, MockFileStorage::new());
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 11);
        assert!(response.is_finished());
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_cross_user_duplicate_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let mut files = MockFileRepository::new();
        files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 42);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let harness = use_case_with(uploads, files, MockFileStorage::new());
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Conflict)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_own_duplicate_returns_existing_file_id() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let mut files = MockFileRepository::new();
        files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.file_id(), 11);
        assert!(!response.is_finished());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_own_duplicate_integrity_mismatch_returns_integrity_mismatch()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let mut files = MockFileRepository::new();
        files.expect_search().times(1).returning(|_| {
            let stored = stored_file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(
            &mut file_storage,
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        );
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(CompleteUploadError::IntegrityMismatch)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_promote_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(StorageError::Conflict) }));
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_save_conflict_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        uploads
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::ConcurrentModification) }));
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("files/5_clip.mp4".to_owned()) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_create_failure_restores_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        files
            .expect_create()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("files/5_clip.mp4".to_owned()) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(uploads, files, file_storage);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn complete_upload_commit_failure_restores_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false)?);
        uploads
            .expect_save()
            .times(1)
            .returning(|upload| Box::pin(async move { Ok(Some(upload)) }));
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        files
            .expect_create()
            .times(1)
            .returning(|file| Box::pin(async move { Ok(file) }));
        let mut file_storage = MockFileStorage::new();
        expect_integrity_hash(&mut file_storage, DIGEST);
        file_storage
            .expect_promote()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok("files/5_clip.mp4".to_owned()) }));
        file_storage
            .expect_restore()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let harness = use_case_with(uploads, files, file_storage);
        harness.commit_fails.store(true, Ordering::SeqCst);
        let command = CompleteUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(CompleteUploadError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
