use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Duration;
use chrono::Utc;
use tracing::error;

use crate::application::port::begin_upload::BeginUploadCommand;
use crate::application::port::begin_upload::BeginUploadError;
use crate::application::port::begin_upload::BeginUploadResponse;
use crate::application::port::begin_upload::BeginUploadUseCase;
use crate::domain::model::file::MD5_INTEGRITY_LENGTH;
use crate::domain::model::upload::ChunkBitmap;
use crate::domain::model::upload::Upload;
use crate::domain::model::upload::UploadError;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::mime_type_repository::MimeTypeRepository as _;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::UploadRepository as _;

/// Use case implementation for beginning an upload session.
pub struct BeginUpload<F, S> {
    /// Fixed size of every chunk except the last, in bytes.
    chunk_size: u64,
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter creating the staging file.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the upload creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> BeginUpload<F, S> {
    /// Create a new use case.
    #[must_use]
    pub fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        chunk_size: u64,
        expiry_seconds: u64,
    ) -> Self {
        Self {
            chunk_size,
            expiry_seconds,
            file_storage,
            unit_of_work_factory,
        }
    }
}

impl<F, S> BeginUploadUseCase for BeginUpload<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
{
    fn execute<'future>(
        &'future self,
        command: BeginUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<BeginUploadResponse, BeginUploadError>> + Send + 'future>>
    {
        let chunk_size = self.chunk_size;
        let expiry_seconds = self.expiry_seconds;
        let file_storage = Arc::clone(&self.file_storage);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let md5 = command.md5().to_owned();
            let validated_md5 = validate_md5(&md5)?;

            let mut unit_of_work = unit_of_work_factory
                .begin()
                .await
                .map_err(|error| BeginUploadError::Unknown(error.into()))?;

            let result = async {
                let content_type_supported = unit_of_work
                    .mime_types()
                    .exists(command.content_type())
                    .await
                    .map_err(|error| BeginUploadError::Unknown(error.into()))?;
                if !content_type_supported {
                    return Err(BeginUploadError::UnsupportedMediaType);
                }

                let created_at = Utc::now().naive_utc();
                let file_size = command.file_size();
                let total = file_size.div_ceil(chunk_size);
                let total_chunks = usize::try_from(total)
                    .map_err(|error| BeginUploadError::Unknown(anyhow::anyhow!(error)))?;
                let chunk_bitmap = ChunkBitmap::try_new(total_chunks)
                    .map_err(|error| BeginUploadError::Unknown(anyhow::Error::new(error)))?;

                let pending = Upload::try_new(
                    0,
                    command.user_id(),
                    command.file_name().to_owned(),
                    file_size,
                    command.content_type().to_owned(),
                    chunk_size,
                    validated_md5,
                    chunk_bitmap,
                    false,
                    0,
                    created_at,
                )
                .map_err(|error| match error {
                    UploadError::InvalidFileName => BeginUploadError::InvalidFileName,
                    UploadError::InvalidFileSize => BeginUploadError::InvalidFileSize,
                    UploadError::Md5IntegrityInvalidLength => BeginUploadError::InvalidMd5,
                    other @ (UploadError::InvalidChunkSize
                    | UploadError::ChunkNumberOutOfRange
                    | UploadError::UploadIncomplete
                    | UploadError::Unknown(_)) => {
                        BeginUploadError::Unknown(anyhow::Error::new(other))
                    }
                })?;

                let upload = unit_of_work
                    .uploads()
                    .create(pending)
                    .await
                    .map_err(|error| BeginUploadError::Unknown(error.into()))?;

                // TODO(reaper): a commit failure after this point leaves an
                // orphaned staging file behind. No reaper exists yet; delete it
                // explicitly once one lands.
                file_storage
                    .create_upload_file(upload.upload_id(), upload.file_size())
                    .await
                    .map_err(|error| BeginUploadError::Unknown(error.into()))?;

                let expires_at = upload
                    .created_at()
                    .checked_add_signed(Duration::seconds(
                        i64::try_from(expiry_seconds)
                            .map_err(|error| BeginUploadError::Unknown(anyhow::anyhow!(error)))?,
                    ))
                    .ok_or_else(|| {
                        BeginUploadError::Unknown(anyhow::anyhow!(
                            "the upload expiry overflows the created_at timestamp"
                        ))
                    })?;

                Ok(BeginUploadResponse::new(
                    upload.upload_id(),
                    chunk_size,
                    expires_at,
                ))
            }
            .await;

            match result {
                Ok(value) => {
                    unit_of_work
                        .commit()
                        .await
                        .map_err(|error| BeginUploadError::Unknown(error.into()))?;
                    Ok(value)
                }
                Err(error) => {
                    if let Err(rollback_error) = unit_of_work.rollback().await {
                        error!(
                            error = ?rollback_error,
                            "failed to roll back the begin upload unit of work"
                        );
                    }
                    Err(error)
                }
            }
        })
    }
}

/// Validate that `md5` is a 32-character hexadecimal string, returning its
/// lowercase form.
///
/// # Errors
///
/// Returns [`BeginUploadError::InvalidMd5`] when the digest does not match.
#[expect(
    clippy::single_call_fn,
    reason = "the digest validation is named after the rule it enforces"
)]
fn validate_md5(md5: &str) -> Result<String, BeginUploadError> {
    let is_hex = md5.len() == MD5_INTEGRITY_LENGTH
        && md5.chars().all(|character| character.is_ascii_hexdigit());
    if is_hex {
        Ok(md5.to_ascii_lowercase())
    } else {
        Err(BeginUploadError::InvalidMd5)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;

    use crate::application::port::begin_upload::BeginUploadCommand;
    use crate::application::port::begin_upload::BeginUploadError;
    use crate::application::port::begin_upload::BeginUploadUseCase as _;
    use crate::application::use_case::test_support::asset_factory;
    use crate::domain::model::upload::Upload;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::upload_repository::MockUploadRepository;

    use super::BeginUpload;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef";
    const CHUNK_SIZE: u64 = 4;
    const TTL_SECONDS: u64 = 3600;

    type UseCase =
        BeginUpload<super::super::test_support::AssetTestUnitOfWorkFactory, MockFileStorage>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set when the unit of work is committed.
        committed: Arc<AtomicBool>,
        /// Set when the unit of work is rolled back.
        rolled_back: Arc<AtomicBool>,
        /// The use case under test.
        use_case: UseCase,
    }

    fn use_case_with(
        setup: impl FnOnce(
            &mut MockUploadRepository,
            &mut MockMimeTypeRepository,
            &mut MockFileStorage,
        ) -> Result<(), Box<dyn Error>>,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut uploads = MockUploadRepository::new();
        let mut mime_types = MockMimeTypeRepository::new();
        let mut file_storage = MockFileStorage::new();
        setup(&mut uploads, &mut mime_types, &mut file_storage)?;
        let harness = asset_factory(uploads, MockFileRepository::new(), mime_types);
        Ok(Harness {
            use_case: BeginUpload::new(
                Arc::clone(&harness.factory),
                Arc::new(file_storage),
                CHUNK_SIZE,
                TTL_SECONDS,
            ),
            committed: harness.committed,
            rolled_back: harness.rolled_back,
        })
    }

    fn command(file_size: u64) -> BeginUploadCommand {
        BeginUploadCommand::new(
            "clip.mp4".to_owned(),
            file_size,
            "video/mp4".to_owned(),
            DIGEST.to_owned(),
            3,
        )
    }

    fn expect_mime_type(mime_types: &mut MockMimeTypeRepository, exists: bool) {
        mime_types
            .expect_exists()
            .times(1)
            .returning(move |_| Box::pin(async move { Ok(exists) }));
    }

    #[expect(
        clippy::single_call_fn,
        reason = "the test fixture mirrors the persisted aggregate"
    )]
    fn stored_upload(command: &BeginUploadCommand) -> Result<Upload, RepositoryError> {
        use crate::domain::model::upload::ChunkBitmap;

        let total_chunks = usize::try_from(command.file_size().div_ceil(CHUNK_SIZE)).unwrap_or(1);
        let bitmap = ChunkBitmap::try_new(total_chunks.max(1))
            .map_err(|_| RepositoryError::OperationFailed)?;
        Upload::try_new(
            17,
            command.user_id(),
            command.file_name().to_owned(),
            command.file_size(),
            command.content_type().to_owned(),
            CHUNK_SIZE,
            DIGEST.to_owned(),
            bitmap,
            false,
            0,
            chrono::Utc::now().naive_utc(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    #[tokio::test]
    async fn begin_upload_valid_request_creates_staging_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|uploads, mime_types, file_storage| {
            expect_mime_type(mime_types, true);
            uploads
                .expect_create()
                .times(1)
                .returning(|upload| Box::pin(async move { Ok(upload) }));
            file_storage
                .expect_create_upload_file()
                .times(1)
                .returning(|_, _| Box::pin(async { Ok(()) }));
            Ok(())
        })?;
        let command = command(10);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.upload_id(), 0);
        assert_eq!(response.chunk_size(), CHUNK_SIZE);
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_unsupported_media_type_returns_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, mime_types, _| {
            expect_mime_type(mime_types, false);
            Ok(())
        })?;
        let command = command(10);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(BeginUploadError::UnsupportedMediaType)
        ));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_invalid_md5_returns_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, _, _| Ok(()))?;
        let command = BeginUploadCommand::new(
            "clip.mp4".to_owned(),
            10,
            "video/mp4".to_owned(),
            "nope".to_owned(),
            3,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::InvalidMd5)));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_invalid_file_name_returns_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|_, mime_types, _| {
            expect_mime_type(mime_types, true);
            Ok(())
        })?;
        let command = BeginUploadCommand::new(
            "../escape.mp4".to_owned(),
            10,
            "video/mp4".to_owned(),
            DIGEST.to_owned(),
            3,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::InvalidFileName)));
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_single_chunk_file_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|uploads, mime_types, file_storage| {
            expect_mime_type(mime_types, true);
            uploads
                .expect_create()
                .times(1)
                .returning(|upload| Box::pin(async move { Ok(upload) }));
            file_storage
                .expect_create_upload_file()
                .times(1)
                .returning(|_, _| Box::pin(async { Ok(()) }));
            Ok(())
        })?;
        let command = command(4);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.chunk_size(), CHUNK_SIZE);
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_repository_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|uploads, mime_types, _| {
            expect_mime_type(mime_types, true);
            uploads
                .expect_create()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::OperationFailed) }));
            Ok(())
        })?;
        let command = command(10);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_storage_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let harness = use_case_with(|uploads, mime_types, file_storage| {
            expect_mime_type(mime_types, true);
            let stored = stored_upload(&command(10))?;
            uploads.expect_create().times(1).returning(move |_| {
                let created = stored.clone();
                Box::pin(async move { Ok(created) })
            });
            file_storage
                .expect_create_upload_file()
                .times(1)
                .returning(|_, _| Box::pin(async { Err(StorageError::Unavailable) }));
            Ok(())
        })?;
        let command = command(10);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
