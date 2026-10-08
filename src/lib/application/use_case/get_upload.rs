use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;

use crate::application::port::get_upload::GetUploadCommand;
use crate::application::port::get_upload::GetUploadError;
use crate::application::port::get_upload::GetUploadResponse;
use crate::application::port::get_upload::GetUploadUseCase;
use crate::application::use_case::upload_layout::staged_path;
use crate::application::use_case::upload_session::caller_file_id;
use crate::application::use_case::upload_session::expiry;
use crate::application::use_case::upload_session::received_bitmap;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::UploadFilter;
use crate::domain::port::upload_repository::UploadRepository as _;

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

/// Use case implementation for fetching the state of an upload session.
pub struct GetUpload<F, S> {
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter deleting the staged file of an expired upload.
    file_storage: Arc<S>,
    /// Root directory holding the upload and file folders.
    root: PathBuf,
    /// Factory opening the unit of work wrapping the read.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> GetUpload<F, S> {
    /// Create a new use case.
    #[must_use]
    pub fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        root: PathBuf,
        expiry_seconds: u64,
    ) -> Self {
        Self {
            expiry_seconds,
            file_storage,
            root,
            unit_of_work_factory,
        }
    }
}

impl<F, S> GetUploadUseCase for GetUpload<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: GetUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetUploadResponse, GetUploadError>> + Send + 'future>>
    {
        let expiry_seconds = self.expiry_seconds;
        let file_storage = Arc::clone(&self.file_storage);
        let root = self.root.clone();
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| GetUploadError::Unknown(error.into()))?;

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
                    Err(error) => return Flow::Failed(GetUploadError::Unknown(error.into())),
                };
                let Some(upload) = found.into_iter().next() else {
                    return Flow::Failed(GetUploadError::NoSuchUpload);
                };

                // Owner-only: a caller who is not the upload's owner must not see it.
                if upload.user_id() != command.user_id() {
                    return Flow::Failed(GetUploadError::NoSuchUpload);
                }

                let expires_at = match expiry(expiry_seconds, upload.created_at()) {
                    Ok(expires_at) => expires_at,
                    Err(error) => return Flow::Failed(GetUploadError::Unknown(error)),
                };
                if !upload.is_finished() && Utc::now().naive_utc() > expires_at {
                    return expire(&mut unit_of_work, &file_storage, &root, command.upload_id())
                        .await;
                }

                let bitmap = received_bitmap(&upload);
                // The caller's file for the session digest is reported whether
                // or not the session is finished: a caller-scoped duplicate
                // completion leaves the session open and returns this same id,
                // so the two endpoints must agree.
                let file_id =
                    match caller_file_id(&mut unit_of_work, &upload, command.user_id()).await {
                        Ok(file_id) => file_id,
                        Err(error) => {
                            return Flow::Failed(GetUploadError::Unknown(error.into()));
                        }
                    };

                Flow::Succeeded(GetUploadResponse::new(
                    bitmap,
                    upload.total_chunks(),
                    expires_at,
                    upload.is_finished(),
                    file_id,
                ))
            }
            .await;

            match flow {
                Flow::Succeeded(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetUploadError::Unknown(error.into()))?;
                    Ok(value)
                }
                Flow::Reaped => {
                    // The expired upload row and staged file were already
                    // deleted inside the transaction; commit so the deletion
                    // survives, then report the session as unknown. A commit
                    // failure is a server error and takes precedence.
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| GetUploadError::Unknown(error.into()))?;
                    Err(GetUploadError::NoSuchUpload)
                }
                Flow::Failed(error) => {
                    if unit_of_work.rollback().await.is_err() {
                        // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    }
                    Err(error)
                }
            }
        })
    }
}

/// Best-effort delete the expired upload's staged file and row, then signal the
/// caller to commit before reporting the session as unknown.
///
/// The read path treats an expired session exactly like a missing one, so the
/// caller never distinguishes the two; the deletion is an internal side effect.
/// The row deletion is left to the reaper once one exists.
#[expect(
    clippy::single_call_fn,
    reason = "the expiry cleanup is named after the rule it enforces"
)]
async fn expire<U, S>(
    unit_of_work: &mut U,
    file_storage: &Arc<S>,
    root: &Path,
    upload_id: i64,
) -> Flow<GetUploadResponse, GetUploadError>
where
    U: AssetUnitOfWork,
    S: FileStorage,
{
    // TODO(reaper): move the expiry cleanup to a background task; this lazy
    // delete keeps the row and the staged file only until the next access.
    // The file-storage and repository adapters own the failure logs
    // (`OBS-002`), so a failed cleanup is swallowed here.
    if file_storage
        .delete_upload_file(&staged_path(root, upload_id))
        .await
        .is_err()
    {
        // The file-storage adapter owns the failure log (`OBS-002`).
    }
    if unit_of_work.uploads().delete(upload_id).await.is_err() {
        // The repository adapter owns the failure log (`OBS-002`).
    }
    Flow::Reaped
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use chrono::NaiveDate;
    use chrono::NaiveDateTime;

    use crate::application::port::get_upload::GetUploadCommand;
    use crate::application::port::get_upload::GetUploadError;
    use crate::application::port::get_upload::GetUploadUseCase as _;
    use crate::domain::model::file::File;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::upload_repository::MockUploadRepository;
    use crate::test_helpers::TestFactory;
    use crate::test_helpers::asset_factory;

    use super::GetUpload;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const TTL_SECONDS: u64 = 3600;
    const CHUNK_SIZE: u64 = 4;
    /// Storage root the use case composes paths against.
    const ROOT: &str = "/storage";

    type UseCase = GetUpload<TestFactory, MockFileStorage>;

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
            use_case: GetUpload::new(
                Arc::clone(&harness.factory),
                Arc::new(file_storage),
                PathBuf::from(ROOT),
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
        created_at: NaiveDateTime,
    ) -> Result<Upload, RepositoryError> {
        let total_chunks = usize::try_from(file_size.div_ceil(CHUNK_SIZE)).unwrap_or(0);
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

    #[expect(
        clippy::single_call_fn,
        reason = "the failure fixture is named for readability"
    )]
    fn expect_upload_failure(uploads: &mut MockUploadRepository) {
        uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
    }

    fn file(id: i64, user_id: i64) -> Result<File, RepositoryError> {
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

    #[tokio::test]
    async fn get_upload_partial_upload_returns_bitmap() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 10, &[0, 2], false, timestamp())?);
        let mut files = MockFileRepository::new();
        files
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(uploads, files, MockFileStorage::new());
        let command = GetUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.bitmap(), "101");
        assert_eq!(response.total_chunks(), 3);
        assert!(!response.is_finished());
        assert_eq!(response.file_id(), None);
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_unfinished_with_owned_digest_returns_file_id() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 10, &[0, 2], false, timestamp())?);
        let mut files = MockFileRepository::new();
        files.expect_search().times(1).returning(|_| {
            let stored = file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let harness = use_case_with(uploads, files, MockFileStorage::new());
        let command = GetUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert!(!response.is_finished());
        assert_eq!(response.file_id(), Some(11));
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_finished_upload_returns_file_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], true, timestamp())?);
        let mut files = MockFileRepository::new();
        files.expect_search().times(1).returning(|_| {
            let stored = file(11, 3);
            Box::pin(async move { stored.map(|file| vec![file]) })
        });
        let harness = use_case_with(uploads, files, MockFileStorage::new());
        let command = GetUploadCommand::new(5, 3);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert!(response.is_finished());
        assert_eq!(response.file_id(), Some(11));
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_unknown_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let harness = use_case_with(uploads, MockFileRepository::new(), MockFileStorage::new());
        let command = GetUploadCommand::new(999, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUploadError::NoSuchUpload)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_other_user_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false, timestamp())?);
        let harness = use_case_with(uploads, MockFileRepository::new(), MockFileStorage::new());
        let command = GetUploadCommand::new(5, 99);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUploadError::NoSuchUpload)));
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_expired_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expired_at = chrono::Utc::now()
            .naive_utc()
            .checked_sub_signed(chrono::Duration::seconds(
                i64::try_from(TTL_SECONDS).unwrap_or(0) + 1,
            ))
            .unwrap_or_else(timestamp);
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false, expired_at)?);
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
        let command = GetUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUploadError::NoSuchUpload)));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_expiry_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expired_at = chrono::Utc::now()
            .naive_utc()
            .checked_sub_signed(chrono::Duration::seconds(
                i64::try_from(TTL_SECONDS).unwrap_or(0) + 1,
            ))
            .unwrap_or_else(timestamp);
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], false, expired_at)?);
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
        let command = GetUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUploadError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn get_upload_search_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload_failure(&mut uploads);
        let harness = use_case_with(uploads, MockFileRepository::new(), MockFileStorage::new());
        let command = GetUploadCommand::new(5, 3);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(GetUploadError::Unknown(_))));
        Ok(())
    }
}
