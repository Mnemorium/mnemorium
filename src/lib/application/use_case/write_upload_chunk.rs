use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Utc;
use tracing::error;

use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
use crate::application::port::write_upload_chunk::WriteUploadChunkError;
use crate::application::port::write_upload_chunk::WriteUploadChunkResponse;
use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;
use crate::application::use_case::upload_session::caller_file_id;
use crate::application::use_case::upload_session::expiry;
use crate::application::use_case::upload_session::received_bitmap;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::content_hasher::ContentHasher;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::unit_of_work::UnitOfWork as _;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::UploadFilter;
use crate::domain::port::upload_repository::UploadRepository as _;

/// Use case implementation for writing one chunk of an upload session.
pub struct WriteUploadChunk<F, S, H> {
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter writing the chunk bytes.
    file_storage: Arc<S>,
    /// Content hasher computing the chunk digest.
    hasher: H,
    /// Factory opening the unit of work wrapping the chunk write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage, H: ContentHasher> WriteUploadChunk<F, S, H> {
    /// Create a new use case.
    #[must_use]
    pub fn new(
        unit_of_work_factory: Arc<F>,
        file_storage: Arc<S>,
        hasher: H,
        expiry_seconds: u64,
    ) -> Self {
        Self {
            expiry_seconds,
            file_storage,
            hasher,
            unit_of_work_factory,
        }
    }
}

impl<F, S, H> WriteUploadChunkUseCase for WriteUploadChunk<F, S, H>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
    H: ContentHasher,
{
    fn execute<'future>(
        &'future self,
        command: WriteUploadChunkCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<WriteUploadChunkResponse, WriteUploadChunkError>>
                + Send
                + 'future,
        >,
    > {
        let expiry_seconds = self.expiry_seconds;
        let file_storage = Arc::clone(&self.file_storage);
        let unit_of_work_factory = Arc::clone(&self.unit_of_work_factory);
        let hasher = &self.hasher;

        Box::pin(async move {
            match execute_attempt(
                &unit_of_work_factory,
                &file_storage,
                hasher,
                &command,
                expiry_seconds,
            )
            .await
            {
                Ok(value) => Ok(value),
                Err(AttemptError::Business(error)) => Err(error),
                Err(AttemptError::Expired) => Err(WriteUploadChunkError::Expired),
            }
        })
    }
}

/// Outcome of the attempt at writing a chunk.
enum AttemptError {
    /// A business error the caller must see.
    Business(WriteUploadChunkError),
    /// The upload expired: its row and staged file have been deleted, so the
    /// unit of work must be committed before returning `Expired`.
    Expired,
}

/// Run the chunk-write attempt inside its own unit of work.
///
/// The attempt opens one transaction, persists the chunk, and closes the
/// transaction according to the outcome.
#[expect(
    clippy::single_call_fn,
    reason = "the attempt body is extracted so the attempt owns its unit of work"
)]
async fn execute_attempt<F, S, H>(
    unit_of_work_factory: &Arc<F>,
    file_storage: &Arc<S>,
    hasher: &H,
    command: &WriteUploadChunkCommand,
    expiry_seconds: u64,
) -> Result<WriteUploadChunkResponse, AttemptError>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
    H: ContentHasher,
{
    let mut unit_of_work = unit_of_work_factory
        .begin()
        .await
        .map_err(|error| AttemptError::Business(WriteUploadChunkError::Unknown(error.into())))?;

    let result = async {
        let upload = unit_of_work
            .uploads()
            .search(&UploadFilter {
                id: Some(command.upload_id()),
                ..UploadFilter::default()
            })
            .await
            .map_err(|error| AttemptError::Business(WriteUploadChunkError::Unknown(error.into())))?
            .into_iter()
            .next()
            .ok_or(AttemptError::Business(WriteUploadChunkError::NoSuchUpload))?;

        // Owner-only: a caller who is not the upload's owner must not see it.
        if upload.user_id() != command.user_id() {
            return Err(AttemptError::Business(WriteUploadChunkError::NoSuchUpload));
        }

        let expires_at = expiry(expiry_seconds, upload.created_at())
            .map_err(|error| AttemptError::Business(WriteUploadChunkError::Unknown(error)))?;
        if !upload.is_finished() && Utc::now().naive_utc() > expires_at {
            // TODO(reaper): move the expiry cleanup to a background task; this
            // lazy delete keeps the row and the staged file only until the next
            // access.
            if let Err(error) = file_storage.delete_upload_file(upload.upload_id()).await {
                error!(error = ?error, "failed to delete the expired upload staged file");
            }
            if let Err(error) = unit_of_work.uploads().delete(upload.upload_id()).await {
                error!(error = ?error, "failed to delete the expired upload row");
            }
            return Err(AttemptError::Expired);
        }

        if upload.is_finished() {
            return Err(AttemptError::Business(
                WriteUploadChunkError::AlreadyFinished,
            ));
        }

        let total_chunks = upload.total_chunks();
        let chunk_number = usize::try_from(command.chunk_number()).map_err(|error| {
            AttemptError::Business(WriteUploadChunkError::Unknown(anyhow::anyhow!(error)))
        })?;
        if chunk_number >= total_chunks {
            return Err(AttemptError::Business(
                WriteUploadChunkError::InvalidChunkNumber,
            ));
        }

        let chunk_size = usize::try_from(upload.chunk_size()).map_err(|error| {
            AttemptError::Business(WriteUploadChunkError::Unknown(anyhow::anyhow!(error)))
        })?;
        let remaining = upload
            .file_size()
            .saturating_sub(command.chunk_number().saturating_mul(upload.chunk_size()));
        let expected_len = remaining.min(upload.chunk_size());
        let chunk_len = u64::try_from(command.chunk().len()).map_err(|error| {
            AttemptError::Business(WriteUploadChunkError::Unknown(anyhow::anyhow!(error)))
        })?;
        if chunk_len != expected_len {
            return Err(AttemptError::Business(WriteUploadChunkError::InvalidChunk));
        }
        // The last chunk may be shorter than `chunk_size`; every other chunk
        // must be exactly `chunk_size`.
        if chunk_number < total_chunks.saturating_sub(1) && chunk_size != command.chunk().len() {
            return Err(AttemptError::Business(WriteUploadChunkError::InvalidChunk));
        }

        let declared = command.content_digest();
        // Recompute the digest of the received bytes and compare: a chunk whose
        // digest does not match its `Content-Digest` header is rejected.
        let mut session = hasher.hasher();
        session.update(command.chunk());
        let computed = session.finalize().map_err(|error| {
            AttemptError::Business(WriteUploadChunkError::Unknown(error.into()))
        })?;
        if computed != *declared {
            return Err(AttemptError::Business(
                WriteUploadChunkError::InvalidContentDigest,
            ));
        }

        let offset = command
            .chunk_number()
            .checked_mul(upload.chunk_size())
            .ok_or(AttemptError::Business(
                WriteUploadChunkError::InvalidChunkNumber,
            ))?;
        // The chunk size is the one persisted when the session began, never the
        // current configuration: the range must match the session's geometry.
        if command.start() != offset {
            return Err(AttemptError::Business(
                WriteUploadChunkError::InvalidChunkRange,
            ));
        }

        file_storage
            .add_chunk(upload.upload_id(), offset, command.chunk().to_vec())
            .await
            .map_err(|error| {
                AttemptError::Business(WriteUploadChunkError::Unknown(error.into()))
            })?;

        match unit_of_work
            .uploads()
            .record_chunk(upload.upload_id(), command.chunk_number())
            .await
        {
            Ok(()) => {}
            Err(RepositoryError::Conflict) => {
                return Err(AttemptError::Business(
                    WriteUploadChunkError::AlreadyFinished,
                ));
            }
            Err(error) => {
                return Err(AttemptError::Business(WriteUploadChunkError::Unknown(
                    error.into(),
                )));
            }
        }

        // Re-read the upload inside the same transaction so the response
        // reports the authoritative bitmap, chunk count and status, including a
        // concurrent completion that landed after the upload was read above.
        let saved = unit_of_work
            .uploads()
            .search(&UploadFilter {
                id: Some(upload.upload_id()),
                ..UploadFilter::default()
            })
            .await
            .map_err(|error| AttemptError::Business(WriteUploadChunkError::Unknown(error.into())))?
            .into_iter()
            .next()
            .ok_or(AttemptError::Business(WriteUploadChunkError::NoSuchUpload))?;

        let bitmap = received_bitmap(&saved);
        let chunk_count = saved.total_chunks();
        let is_finished = saved.is_finished();
        let file_id = caller_file_id(&mut unit_of_work, &saved, command.user_id())
            .await
            .map_err(|error| {
                AttemptError::Business(WriteUploadChunkError::Unknown(error.into()))
            })?;
        Ok(WriteUploadChunkResponse::new(
            bitmap,
            expires_at,
            file_id,
            is_finished,
            chunk_count,
        ))
    }
    .await;

    match result {
        Ok(value) => {
            unit_of_work.commit().await.map_err(|error| {
                AttemptError::Business(WriteUploadChunkError::Unknown(error.into()))
            })?;
            Ok(value)
        }
        Err(AttemptError::Expired) => {
            // The expired upload row and staged file were already deleted
            // inside the transaction; commit so the deletion survives, then
            // report the expiry. A commit failure is a server error and takes
            // precedence.
            unit_of_work.commit().await.map_err(|error| {
                AttemptError::Business(WriteUploadChunkError::Unknown(error.into()))
            })?;
            Err(AttemptError::Business(WriteUploadChunkError::Expired))
        }
        Err(error) => {
            if let Err(rollback_error) = unit_of_work.rollback().await {
                error!(
                    error = ?rollback_error,
                    "failed to roll back the write upload chunk unit of work"
                );
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;
    use std::sync::atomic::Ordering;

    use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
    use crate::application::port::write_upload_chunk::WriteUploadChunkError;
    use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase as _;
    use crate::domain::model::integrity_hash::IntegrityHash;
    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::port::content_hasher::MockContentHasher;
    use crate::domain::port::content_hasher::MockContentHasherSession;
    use crate::domain::port::error::ContentHasherError;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::upload_repository::MockUploadRepository;
    use crate::test_helpers::TestUnitOfWorkFactory;
    use crate::test_helpers::asset_factory;
    use crate::test_helpers::asset_unit_of_work;

    use super::WriteUploadChunk;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    /// SHA-256 of `b"1234"`.
    const CHUNK_DIGEST: &str = "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4";
    /// SHA-256 of `b"5678"`.
    const CHUNK_DIGEST_5678: &str =
        "f8638b979b2f4f793ddb6dbd197e0ee25a7a6ea32b0ae22f5e3c5d119d839e75";
    const WRONG_CHUNK_DIGEST: &str =
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    const TTL_SECONDS: u64 = 3600;
    const CHUNK_SIZE: u64 = 4;

    /// Build an [`IntegrityHash`] from a literal digest.
    #[expect(
        clippy::expect_used,
        reason = "the test literals are valid 64-character hexadecimal digests"
    )]
    fn hash(digest: &str) -> IntegrityHash<64> {
        IntegrityHash::try_new(digest.to_owned()).expect("the fixture digest is valid")
    }

    fn timestamp() -> chrono::NaiveDateTime {
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
            hash(DIGEST),
            bitmap,
            is_finished,
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

    /// Queue the results the use case's `search` calls return, in order: the
    /// initial read and, after a successful `record_chunk`, the re-read.
    fn expect_search_results(uploads: &mut MockUploadRepository, results: Vec<Vec<Upload>>) {
        let queue = Arc::new(Mutex::new(VecDeque::from(results)));
        uploads.expect_search().times(1..).returning(move |_| {
            let next = queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop_front()
                .unwrap_or_default();
            Box::pin(async move { Ok(next) })
        });
    }

    #[tokio::test]
    async fn write_upload_chunk_valid_chunk_persists_bitmap() -> Result<(), Box<dyn Error>> {
        let mut hasher = MockContentHasher::new();
        hasher.expect_hasher().times(1).returning(|| {
            let mut session = MockContentHasherSession::new();
            session
                .expect_update()
                .times(1)
                .withf(|bytes: &[u8]| bytes == b"1234")
                .return_const(());
            session
                .expect_finalize()
                .times(1)
                .returning(|| Ok(hash(CHUNK_DIGEST)));
            Box::new(session)
        });
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_search_results(
            &mut uploads,
            vec![
                vec![upload(5, 3, 8, &[], false)?],
                vec![upload(5, 3, 8, &[0], false)?],
            ],
        );
        uploads
            .expect_record_chunk()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let mut file_repository = MockFileRepository::new();
        file_repository
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_add_chunk()
            .times(1)
            .returning(|_, offset, _| {
                Box::pin(async move {
                    assert_eq!(offset, 0);
                    Ok(())
                })
            });
        let (unit_of_work, committed, rolled_back) =
            asset_unit_of_work(uploads, file_repository, MockMimeTypeRepository::new());
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(file_storage),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert_eq!(response.bitmap(), "10");
        assert_eq!(response.total_chunks(), 2);
        assert!(!response.is_finished());
        assert_eq!(response.file_id(), None);
        assert!(response.expires_at() > timestamp());
        assert!(committed.load(Ordering::SeqCst));
        assert!(!rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_vanished_upload_returns_no_such_upload()
    -> Result<(), Box<dyn Error>> {
        let mut hasher = MockContentHasher::new();
        hasher.expect_hasher().times(1).returning(|| {
            let mut session = MockContentHasherSession::new();
            session
                .expect_update()
                .times(1)
                .withf(|bytes: &[u8]| bytes == b"1234")
                .return_const(());
            session
                .expect_finalize()
                .times(1)
                .returning(|| Ok(hash(CHUNK_DIGEST)));
            Box::new(session)
        });
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_search_results(
            &mut uploads,
            vec![vec![upload(5, 3, 8, &[], false)?], Vec::new()],
        );
        uploads
            .expect_record_chunk()
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_add_chunk()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        let (unit_of_work, _committed, rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(file_storage),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::NoSuchUpload)));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_unknown_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>>
    {
        let hasher = MockContentHasher::new();
        // Arrange
        let mut uploads = MockUploadRepository::new();
        uploads
            .expect_search()
            .times(1)
            .returning(|_| Box::pin(async { Ok(Vec::new()) }));
        let (unit_of_work, _committed, rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(999, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::NoSuchUpload)));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_other_user_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        let hasher = MockContentHasher::new();
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 99);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::NoSuchUpload)));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_expired_upload_returns_expired() -> Result<(), Box<dyn Error>> {
        let hasher = MockContentHasher::new();
        // Arrange
        let expired_at = chrono::Utc::now()
            .naive_utc()
            .checked_sub_signed(chrono::Duration::seconds(
                i64::try_from(TTL_SECONDS).unwrap_or(0) + 1,
            ))
            .unwrap_or_else(timestamp);
        let mut bitmap = ChunkBitmap::try_new(2).map_err(|_| RepositoryError::OperationFailed)?;
        bitmap
            .mark_received(0)
            .map_err(|_| RepositoryError::OperationFailed)?;
        let expired = Upload::try_new(
            5,
            3,
            "clip.mp4".to_owned(),
            8,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            hash(DIGEST),
            bitmap,
            false,
            expired_at,
        )
        .map_err(|_| RepositoryError::OperationFailed)?;
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, expired);
        uploads
            .expect_delete()
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let (unit_of_work, committed, rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_delete_upload_file()
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(file_storage),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 1, 4, b"5678".to_vec(), hash(CHUNK_DIGEST_5678), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::Expired)));
        assert!(committed.load(Ordering::SeqCst));
        assert!(!rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_expiry_commit_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        let hasher = MockContentHasher::new();
        // Arrange
        let expired_at = chrono::Utc::now()
            .naive_utc()
            .checked_sub_signed(chrono::Duration::seconds(
                i64::try_from(TTL_SECONDS).unwrap_or(0) + 1,
            ))
            .unwrap_or_else(timestamp);
        let mut bitmap = ChunkBitmap::try_new(2).map_err(|_| RepositoryError::OperationFailed)?;
        bitmap
            .mark_received(0)
            .map_err(|_| RepositoryError::OperationFailed)?;
        let expired = Upload::try_new(
            5,
            3,
            "clip.mp4".to_owned(),
            8,
            "video/mp4".to_owned(),
            CHUNK_SIZE,
            hash(DIGEST),
            bitmap,
            false,
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
        let harness = asset_factory(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        harness.commit_fails.store(true, Ordering::SeqCst);
        let use_case = WriteUploadChunk::new(
            Arc::clone(&harness.factory),
            Arc::new(file_storage),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 1, 4, b"5678".to_vec(), hash(CHUNK_DIGEST_5678), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::Unknown(_))));
        assert!(harness.committed.load(Ordering::SeqCst));
        assert!(!harness.rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_finished_upload_returns_already_finished()
    -> Result<(), Box<dyn Error>> {
        let hasher = MockContentHasher::new();
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], true)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(WriteUploadChunkError::AlreadyFinished)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_out_of_range_returns_invalid_chunk_number()
    -> Result<(), Box<dyn Error>> {
        let hasher = MockContentHasher::new();
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 5, 20, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(WriteUploadChunkError::InvalidChunkNumber)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_start_mismatch_returns_invalid_chunk_range()
    -> Result<(), Box<dyn Error>> {
        let mut hasher = MockContentHasher::new();
        hasher.expect_hasher().times(1).returning(|| {
            let mut session = MockContentHasherSession::new();
            session
                .expect_update()
                .times(1)
                .withf(|bytes: &[u8]| bytes == b"1234")
                .return_const(());
            session
                .expect_finalize()
                .times(1)
                .returning(|| Ok(hash(CHUNK_DIGEST)));
            Box::new(session)
        });
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        // Chunk `0` is persisted with a size of 4, so its start must be 0.
        let command =
            WriteUploadChunkCommand::new(5, 0, 4, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(WriteUploadChunkError::InvalidChunkRange)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_wrong_size_returns_invalid_chunk() -> Result<(), Box<dyn Error>> {
        let hasher = MockContentHasher::new();
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        let command = WriteUploadChunkCommand::new(5, 0, 0, b"12".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::InvalidChunk)));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_wrong_digest_returns_invalid_content_digest()
    -> Result<(), Box<dyn Error>> {
        let mut hasher = MockContentHasher::new();
        hasher.expect_hasher().times(1).returning(|| {
            let mut session = MockContentHasherSession::new();
            session
                .expect_update()
                .times(1)
                .withf(|bytes: &[u8]| bytes == b"1234")
                .return_const(());
            session
                .expect_finalize()
                .times(1)
                .returning(|| Ok(hash(CHUNK_DIGEST)));
            Box::new(session)
        });
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(WRONG_CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(WriteUploadChunkError::InvalidContentDigest)
        ));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_storage_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        let mut hasher = MockContentHasher::new();
        hasher.expect_hasher().times(1).returning(|| {
            let mut session = MockContentHasherSession::new();
            session
                .expect_update()
                .times(1)
                .withf(|bytes: &[u8]| bytes == b"1234")
                .return_const(());
            session
                .expect_finalize()
                .times(1)
                .returning(|| Ok(hash(CHUNK_DIGEST)));
            Box::new(session)
        });
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_add_chunk()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Err(StorageError::Unavailable) }));
        let (unit_of_work, _committed, rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(file_storage),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::Unknown(_))));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_finished_race_returns_already_finished()
    -> Result<(), Box<dyn Error>> {
        let mut hasher = MockContentHasher::new();
        hasher.expect_hasher().times(1).returning(|| {
            let mut session = MockContentHasherSession::new();
            session
                .expect_update()
                .times(1)
                .withf(|bytes: &[u8]| bytes == b"1234")
                .return_const(());
            session
                .expect_finalize()
                .times(1)
                .returning(|| Ok(hash(CHUNK_DIGEST)));
            Box::new(session)
        });
        // Arrange: the finished-guard trigger rejects the chunk insert because a
        // concurrent completion won the race.
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        uploads
            .expect_record_chunk()
            .times(1)
            .returning(|_, _| Box::pin(async { Err(RepositoryError::Conflict) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_add_chunk()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        let (unit_of_work, _committed, rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(file_storage),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(
            result,
            Err(WriteUploadChunkError::AlreadyFinished)
        ));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_hasher_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = TestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let mut hasher = MockContentHasher::new();
        hasher.expect_hasher().times(1).returning(|| {
            let mut session = MockContentHasherSession::new();
            session
                .expect_update()
                .times(1)
                .withf(|bytes: &[u8]| bytes == b"1234")
                .return_const(());
            session
                .expect_finalize()
                .times(1)
                .returning(|| Err(ContentHasherError::OperationFailed));
            Box::new(session)
        });
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            hasher,
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, 0, b"1234".to_vec(), hash(CHUNK_DIGEST), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::Unknown(_))));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }
}
