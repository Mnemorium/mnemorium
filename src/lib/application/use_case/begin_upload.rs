use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{Duration, Utc};

use crate::application::port::{
    BeginUploadCommand, BeginUploadError, BeginUploadResponse, BeginUploadUseCase,
};
use crate::application::security_event;
use crate::application::use_case::{media_root, staged_path};
use crate::domain::NumericID;
use crate::domain::model::{ChunkBitmap, MAX_TOTAL_CHUNKS, Upload, UploadError};
use crate::domain::port::{
    AssetUnitOfWork, FileStorage, MimeTypeFilter, MimeTypeRepository as _, UnitOfWork as _,
    UnitOfWorkFactory, UploadRepository as _, UserFilter, UserRepository as _, UserUnitOfWork,
};

/// Use case implementation for beginning an upload session.
pub(crate) struct BeginUpload<F, S> {
    /// Fixed size of every chunk except the last, in bytes.
    chunk_size: u64,
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter creating the staging file.
    file_storage: Arc<S>,
    /// Maximum size of a single uploaded file, in bytes.
    max_file_size_bytes: u64,
    /// Factory opening the unit of work wrapping the upload creation.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> BeginUpload<F, S> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        chunk_size: u64,
        expiry_seconds: u64,
        max_file_size_bytes: u64,
    ) -> Self {
        Self {
            chunk_size,
            expiry_seconds,
            file_storage,
            max_file_size_bytes,
            unit_of_work_factory,
        }
    }
}

impl<F, S> BeginUploadUseCase for BeginUpload<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork + UserUnitOfWork,
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
        let max_file_size_bytes = self.max_file_size_bytes;
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);

        Box::pin(async move {
            let integrity_hash = command.integrity_hash().clone();

            // Reject oversized requests before opening a transaction, building a
            // bitmap, or preallocating the staging file: nothing is allocated
            // from a client-declared size above the configured maximum.
            let file_size = command.file_size();
            if file_size == 0 {
                security_event::input_validation_failed("file_size", "zero");
                return Err(BeginUploadError::InvalidFileSize);
            }
            if file_size > max_file_size_bytes {
                security_event::suspicious_business_logic(
                    command.user_id(),
                    "begin_upload",
                    "file_size_exceeded",
                );
                return Err(BeginUploadError::FileTooLarge);
            }
            let total = file_size.div_ceil(chunk_size);
            let total_chunks = usize::try_from(total).map_err(|error| {
                security_event::application_error("begin_upload");
                BeginUploadError::Unknown(anyhow::anyhow!(error))
            })?;
            if total_chunks > MAX_TOTAL_CHUNKS {
                security_event::suspicious_business_logic(
                    command.user_id(),
                    "begin_upload",
                    "total_chunks_exceeded",
                );
                return Err(BeginUploadError::FileTooLarge);
            }

            // The library root is the operator's mount and is never created, so
            // a missing root fails the begin before anything is allocated rather
            // than after the client has uploaded the whole file.
            match file_storage.directory_exist(&media_root()).await {
                Ok(true) => {}
                Ok(false) => return Err(BeginUploadError::LibraryUnavailable),
                Err(error) => return Err(BeginUploadError::Unknown(error.into())),
            }

            let mut unit_of_work = unit_of_work_factory
                .begin_write()
                .await
                .map_err(|error| BeginUploadError::Unknown(error.into()))?;

            // Once the upload row exists a staging file may be created; every
            // path that does not commit the row must remove it. A leftover file
            // otherwise collides with the rowid SQLite reuses after a rollback,
            // which wedges every later upload.
            let mut staged_upload_id: Option<NumericID> = None;

            let result = async {
                let caller = unit_of_work
                    .users()
                    .search(&UserFilter {
                        id: Some(command.user_id()),
                        ..UserFilter::default()
                    })
                    .await
                    .map_err(|error| BeginUploadError::Unknown(error.into()))?
                    .into_iter()
                    .next()
                    .ok_or(BeginUploadError::NoSuchCaller)?;

                let content_type_supported = !unit_of_work
                    .mime_types()
                    .search(&MimeTypeFilter {
                        mime_type_id: Some(command.content_type().to_owned()),
                    })
                    .await
                    .map_err(|error| BeginUploadError::Unknown(error.into()))?
                    .is_empty();
                if !content_type_supported {
                    return Err(BeginUploadError::UnsupportedMediaType);
                }

                let created_at = Utc::now().naive_utc();
                let chunk_bitmap = ChunkBitmap::try_new(total_chunks)
                    .map_err(|error| BeginUploadError::Unknown(anyhow::Error::new(error)))?;

                let pending = Upload::try_new(
                    0,
                    caller.id(),
                    command.file_name().to_owned(),
                    file_size,
                    command.content_type().to_owned(),
                    chunk_size,
                    integrity_hash,
                    chunk_bitmap,
                    false,
                    false,
                    None,
                    created_at,
                )
                .map_err(|error| match error {
                    UploadError::InvalidFileName => {
                        security_event::input_validation_failed("file_name", "invalid");
                        BeginUploadError::InvalidFileName
                    }
                    UploadError::InvalidFileSize => BeginUploadError::InvalidFileSize,
                    other @ (UploadError::InvalidChunkSize
                    | UploadError::ChunkNumberOutOfRange
                    | UploadError::IntegrityHash(_)
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

                // From here on a staging file may exist on disk for this
                // identifier; the cleanup below must remove it unless the unit
                // of work commits.
                staged_upload_id = Some(upload.upload_id());

                file_storage
                    .create_upload_file(&staged_path(upload.upload_id()), upload.file_size())
                    .await
                    .map_err(|error| BeginUploadError::Unknown(error.into()))?;

                let expires_at = upload
                    .created_at()
                    .checked_add_signed(Duration::seconds(i64::try_from(expiry_seconds).map_err(
                        |error| {
                            security_event::application_error("begin_upload");
                            BeginUploadError::Unknown(anyhow::anyhow!(error))
                        },
                    )?))
                    .ok_or_else(|| {
                        security_event::application_error("begin_upload");
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
                Ok(value) => match unit_of_work.commit().await {
                    Ok(()) => {
                        security_event::upload(
                            command.user_id(),
                            &value.upload_id().to_string(),
                            "accepted",
                        );
                        Ok(value)
                    }
                    Err(error) => {
                        // The commit outcome is ambiguous: the row may or may
                        // not be persisted. Removing the staging file cannot
                        // wedge later uploads, whereas leaving it behind can.
                        discard_staging(file_storage.as_ref(), staged_upload_id).await;
                        Err(BeginUploadError::Unknown(error.into()))
                    }
                },
                Err(error) => {
                    // The unit-of-work adapter owns the rollback-failure log (OBS-002).
                    let _rollback = unit_of_work.rollback().await;
                    discard_staging(file_storage.as_ref(), staged_upload_id).await;
                    Err(error)
                }
            }
        })
    }
}

/// Best-effort remove the staging file of an upload whose row was not committed.
///
/// A missing staging file is success and an absent identifier is a no-op. A
/// deletion failure is swallowed, never fatal: the file-storage adapter owns the
/// failure log (`OBS-002`) and the caller already carries the error that aborted
/// the begin. A staging file survives only a process death between creating it
/// and committing the row; a future reaper covers that.
async fn discard_staging<S>(file_storage: &S, upload_id: Option<NumericID>)
where
    S: FileStorage,
{
    let Some(staged) = upload_id else {
        return;
    };
    if file_storage
        .delete_upload_file(&staged_path(staged))
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
    use crate::domain::model::{ChunkBitmap, IntegrityHash, MimeType};

    use crate::domain::port::{
        MockFileStorage, MockMimeTypeRepository, MockUploadRepository, MockUserRepository,
        RepositoryError, StorageError,
    };

    use crate::test_helpers::*;
    use std::error::Error;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing_test::traced_test;

    const DEFAULT_MAX_FILE_SIZE: u64 = 1_000_000;

    /// Largest declared file size that still splits into exactly
    /// [`MAX_TOTAL_CHUNKS`] chunks.
    const MAX_TOTAL_CHUNKS_FILE_BYTES: u64 = MAX_TOTAL_CHUNKS as u64 * CHUNK_SIZE;

    type UseCase = BeginUpload<TestFactory, MockFileStorage>;

    /// A use case under test together with its transaction-lifecycle flags.
    struct Harness {
        /// Set to force the unit of work commit to fail.
        commit_fails: Arc<AtomicBool>,
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
        use_case_with_all(setup, TTL_SECONDS, DEFAULT_MAX_FILE_SIZE, true, CHUNK_SIZE)
    }

    /// Build a use case whose configured maximum file size is
    /// `max_file_size_bytes`.
    fn use_case_with_max(
        setup: impl FnOnce(
            &mut MockUploadRepository,
            &mut MockMimeTypeRepository,
            &mut MockFileStorage,
        ) -> Result<(), Box<dyn Error>>,
        max_file_size_bytes: u64,
    ) -> Result<Harness, Box<dyn Error>> {
        use_case_with_all(setup, TTL_SECONDS, max_file_size_bytes, true, CHUNK_SIZE)
    }

    /// Build a use case configured with `expiry_seconds` and
    /// `max_file_size_bytes`.
    fn use_case_with_all(
        setup: impl FnOnce(
            &mut MockUploadRepository,
            &mut MockMimeTypeRepository,
            &mut MockFileStorage,
        ) -> Result<(), Box<dyn Error>>,
        expiry_seconds: u64,
        max_file_size_bytes: u64,
        library_root_exists: bool,
        chunk_size: u64,
    ) -> Result<Harness, Box<dyn Error>> {
        let mut uploads = MockUploadRepository::new();
        let mut mime_types = MockMimeTypeRepository::new();
        let mut file_storage = MockFileStorage::new();
        setup(&mut uploads, &mut mime_types, &mut file_storage)?;
        // The library root is present unless the caller says otherwise. The
        // expectation tolerates being unreached: a request rejected on its
        // declared size or name never looks at the root.
        file_storage
            .expect_directory_exist()
            .times(0..)
            .returning(move |_| Box::pin(async move { Ok(library_root_exists) }));
        let unit_of_work = TestUnitOfWork {
            users: any_caller(),
            mime_types,
            uploads,
            ..TestUnitOfWork::default()
        };
        let harness = AssetFactoryHarness::new(unit_of_work);
        Ok(Harness {
            use_case: BeginUpload::new(
                Arc::clone(&harness.factory),
                Arc::new(file_storage),
                chunk_size,
                expiry_seconds,
                max_file_size_bytes,
            ),
            commit_fails: harness.commit_fails,
            committed: harness.committed,
            rolled_back: harness.rolled_back,
        })
    }

    fn command(file_size: u64) -> BeginUploadCommand {
        BeginUploadCommand::new(
            "clip.mp4".to_owned(),
            file_size,
            "video/mp4".to_owned(),
            hash(DIGEST),
            3,
        )
    }

    fn expect_mime_type(mime_types: &mut MockMimeTypeRepository, exists: bool) {
        mime_types.expect_search().times(1).returning(move |_| {
            let found: Vec<MimeType> = if exists {
                MimeType::try_new("video/mp4".to_owned())
                    .ok()
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            };
            Box::pin(async move { Ok(found) })
        });
    }

    fn stored_upload(command: &BeginUploadCommand) -> Result<Upload, RepositoryError> {
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
            IntegrityHash::try_new(DIGEST.to_owned())
                .map_err(|_| RepositoryError::OperationFailed)?,
            bitmap,
            false,
            false,
            None,
            chrono::Utc::now().naive_utc(),
        )
        .map_err(|_| RepositoryError::OperationFailed)
    }

    #[tokio::test]
    async fn begin_upload_deleted_caller_returns_no_such_caller() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut users = MockUserRepository::new();
        users
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let unit_of_work = TestUnitOfWork {
            users,
            ..TestUnitOfWork::default()
        };
        let harness = AssetFactoryHarness::new(unit_of_work);
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let use_case = BeginUpload::new(
            Arc::clone(&harness.factory),
            Arc::new(file_storage),
            CHUNK_SIZE,
            TTL_SECONDS,
            DEFAULT_MAX_FILE_SIZE,
        );

        // Act
        let result = use_case.execute(command(8)).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::NoSuchCaller)));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
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
    async fn begin_upload_valid_digest_reaches_repository() -> Result<(), Box<dyn Error>> {
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
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(result.is_ok());
        assert!(harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
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
            hash(DIGEST),
            3,
        );

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::InvalidFileName)));
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!(
                    "the invalid-name rejection must emit exactly one warn event (found {count})"
                )
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"input_validation_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "the invalid-name rejection must emit exactly one input_validation_failed event (found {count})"
                ))
        });
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain("field=file_name"),
            "the event must name the rejected field"
        );
        assert!(
            logs_contain("reason=invalid"),
            "the event must classify the reason as an invalid name"
        );
        assert!(
            !logs_contain("escape.mp4"),
            "the event must not echo the client-supplied file name"
        );
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
            // The failed create may have left a file behind, so the rollback
            // must also discard the staging file of the created row.
            file_storage
                .expect_delete_upload_file()
                .times(1)
                .returning(|_| Box::pin(async { Ok(()) }));
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
    async fn begin_upload_failure_after_staging_creation_deletes_staging_file()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        // An expiry that does not fit in `i64` aborts the begin after the row
        // and the staging file exist, exercising the compensation path.
        let harness = use_case_with_all(
            |uploads, mime_types, file_storage| {
                expect_mime_type(mime_types, true);
                uploads
                    .expect_create()
                    .times(1)
                    .returning(|upload| Box::pin(async move { Ok(upload) }));
                file_storage
                    .expect_create_upload_file()
                    .times(1)
                    .returning(|_, _| Box::pin(async { Ok(()) }));
                file_storage
                    .expect_delete_upload_file()
                    .times(1)
                    .returning(|_| Box::pin(async { Ok(()) }));
                Ok(())
            },
            u64::MAX,
            DEFAULT_MAX_FILE_SIZE,
            true,
            CHUNK_SIZE,
        )?;
        let command = command(10);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::Unknown(_))));
        assert!(harness.rolled_back.load(Ordering::SeqCst));
        assert!(!harness.committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_commit_failure_deletes_staging_file() -> Result<(), Box<dyn Error>> {
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
            file_storage
                .expect_delete_upload_file()
                .times(1)
                .returning(|_| Box::pin(async { Ok(()) }));
            Ok(())
        })?;
        harness.commit_fails.store(true, Ordering::SeqCst);
        let command = command(10);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn begin_upload_file_at_max_size_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let max_file_size = 10;
        let harness = use_case_with_max(
            |uploads, mime_types, file_storage| {
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
            },
            max_file_size,
        )?;
        let command = command(max_file_size);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.chunk_size(), CHUNK_SIZE);
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(
            !logs_contain("suspicious_business_logic"),
            "a file at the size limit must not emit a suspicious_business_logic event"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn begin_upload_file_over_max_size_returns_file_too_large() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        // No repository or storage expectation: the rejection must happen before
        // any of them is reached (mockall fails the test on an unexpected call).
        let harness = use_case_with_max(|_, _, _| Ok(()), 10)?;
        let command = command(11);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::FileTooLarge)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!(
                    "the declared-size rejection must emit exactly one warn event (found {count})"
                )
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"suspicious_business_logic\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "the declared-size rejection must emit exactly one suspicious_business_logic event (found {count})"
                ))
        });
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain(" actor=3 "),
            "the event must carry the command's actor"
        );
        assert!(
            logs_contain("action=begin_upload"),
            "the event must carry the use-case action"
        );
        assert!(
            logs_contain("reason=file_size_exceeded"),
            "the event must classify the reason as the declared size"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn begin_upload_zero_file_size_returns_invalid_file_size() -> Result<(), Box<dyn Error>> {
        // Arrange
        // No repository or storage expectation: the rejection must happen before
        // any of them is reached (mockall fails the test on an unexpected call).
        let harness = use_case_with_max(|_, _, _| Ok(()), DEFAULT_MAX_FILE_SIZE)?;
        let command = command(0);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::InvalidFileSize)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("the zero-size rejection must emit exactly one warn event (found {count})")
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"input_validation_failed\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "the zero-size rejection must emit exactly one input_validation_failed event (found {count})"
                ))
        });
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain("field=file_size"),
            "the event must name the rejected field"
        );
        assert!(
            logs_contain("reason=zero"),
            "the event must classify the reason as the declared size"
        );
        assert!(
            !logs_contain("clip.mp4"),
            "the event must not echo the client-supplied file name"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn begin_upload_max_total_chunks_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        // Exactly `MAX_TOTAL_CHUNKS` chunks is the largest file the chunk-count
        // backstop accepts. `max_file_size_bytes` equals the declared size, so
        // the size rule is satisfied at its limit and the chunk rule decides.
        let harness = use_case_with_max(
            |uploads, mime_types, file_storage| {
                expect_mime_type(mime_types, true);
                uploads
                    .expect_create()
                    .times(1)
                    .withf(|upload| upload.total_chunks() == MAX_TOTAL_CHUNKS)
                    .returning(|upload| Box::pin(async move { Ok(upload) }));
                file_storage
                    .expect_create_upload_file()
                    .times(1)
                    .returning(|_, _| Box::pin(async { Ok(()) }));
                Ok(())
            },
            MAX_TOTAL_CHUNKS_FILE_BYTES,
        )?;
        let command = command(MAX_TOTAL_CHUNKS_FILE_BYTES);

        // Act
        let response = harness.use_case.execute(command).await?;

        // Assert
        assert_eq!(response.chunk_size(), CHUNK_SIZE);
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        assert!(
            !logs_contain("suspicious_business_logic"),
            "a file at the chunk-count limit must not emit a suspicious_business_logic event"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn begin_upload_too_many_chunks_returns_file_too_large() -> Result<(), Box<dyn Error>> {
        // Arrange
        // One byte past the largest file that fits in `MAX_TOTAL_CHUNKS`
        // chunks. `max_file_size_bytes` equals the declared size, so the size
        // rule is satisfied and only the chunk-count backstop can reject.
        let file_size = MAX_TOTAL_CHUNKS_FILE_BYTES + 1;
        let harness = use_case_with_max(|_, _, _| Ok(()), file_size)?;
        let command = command(file_size);

        // Act
        let result = harness.use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::FileTooLarge)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("WARN")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!(
                    "the chunk-count rejection must emit exactly one warn event (found {count})"
                )
            })
        });
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"suspicious_business_logic\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "the chunk-count rejection must emit exactly one suspicious_business_logic event (found {count})"
                ))
        });
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        assert!(
            logs_contain(" actor=3 "),
            "the event must carry the command's actor"
        );
        assert!(
            logs_contain("action=begin_upload"),
            "the event must carry the use-case action"
        );
        assert!(
            logs_contain("reason=total_chunks_exceeded"),
            "the event must classify the reason as the chunk-count limit"
        );
        Ok(())
    }

    #[tokio::test]
    async fn begin_upload_absent_library_root_returns_library_unavailable()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the library root is absent and the use case must stop before
        // creating a session or a staging file.
        let harness = use_case_with_all(
            |_uploads, _mime_types, _file_storage| Ok(()),
            TTL_SECONDS,
            DEFAULT_MAX_FILE_SIZE,
            false,
            CHUNK_SIZE,
        )?;

        // Act
        let result = harness.use_case.execute(command(4)).await;

        // Assert
        assert!(matches!(result, Err(BeginUploadError::LibraryUnavailable)));
        assert!(!harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    /// Build a use case whose session geometry is fixed at `chunk_size` bytes.
    fn use_case_with_chunk_size(chunk_size: u64) -> Result<Harness, Box<dyn Error>> {
        use_case_with_all(
            |uploads, mime_types, file_storage| {
                expect_mime_type(mime_types, true);
                uploads
                    .expect_create()
                    .times(1)
                    .withf(move |upload| upload.chunk_size() == chunk_size)
                    .returning(|upload| Box::pin(async move { Ok(upload) }));
                file_storage
                    .expect_create_upload_file()
                    .times(1)
                    .returning(|_, _| Box::pin(async { Ok(()) }));
                Ok(())
            },
            TTL_SECONDS,
            DEFAULT_MAX_FILE_SIZE,
            true,
            chunk_size,
        )
    }

    #[tokio::test]
    async fn begin_upload_keeps_the_chunk_size_it_was_built_with() -> Result<(), Box<dyn Error>> {
        // Arrange: the use case holds the chunk size it was built with and reads
        // no configuration itself, so a runtime change cannot reach a session.
        let harness = use_case_with_chunk_size(8)?;

        // Act
        let response = harness.use_case.execute(command(16)).await?;

        // Assert
        assert_eq!(response.chunk_size(), 8);
        assert_eq!(response.chunk_size().div_ceil(16), 1);
        Ok(())
    }
}
