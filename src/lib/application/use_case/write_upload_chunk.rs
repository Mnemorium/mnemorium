use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::Duration;
use chrono::NaiveDateTime;
use chrono::Utc;
use md5::Digest as _;
use md5::Md5;
use tracing::error;

use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
use crate::application::port::write_upload_chunk::WriteUploadChunkError;
use crate::application::port::write_upload_chunk::WriteUploadChunkResponse;
use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase;
use crate::domain::model::upload::MD5_INTEGRITY_LENGTH;
use crate::domain::model::upload::Upload;
use crate::domain::port::asset_unit_of_work::AssetUnitOfWork;
use crate::domain::port::error::RepositoryError;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::unit_of_work::UnitOfWorkFactory;
use crate::domain::port::upload_repository::UploadFilter;
use crate::domain::port::upload_repository::UploadRepository as _;

/// Maximum number of attempts to persist a chunk bitmap before giving up.
const MAX_CAS_ATTEMPTS: u8 = 3;

/// Use case implementation for writing one chunk of an upload session.
pub struct WriteUploadChunk<F, S> {
    /// Lifetime of an upload session, in seconds.
    expiry_seconds: u64,
    /// Storage adapter writing the chunk bytes.
    file_storage: Arc<S>,
    /// Factory opening the unit of work wrapping the chunk write.
    unit_of_work_factory: Arc<F>,
}

impl<F: UnitOfWorkFactory, S: FileStorage> WriteUploadChunk<F, S> {
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

impl<F, S> WriteUploadChunkUseCase for WriteUploadChunk<F, S>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
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

        Box::pin(async move {
            let mut attempt = 0u8;
            loop {
                let result = execute_attempt(
                    &unit_of_work_factory,
                    &file_storage,
                    &command,
                    expiry_seconds,
                )
                .await;
                match result {
                    Err(AttemptError::ConcurrentModification) if attempt < MAX_CAS_ATTEMPTS - 1 => {
                        attempt = attempt.saturating_add(1);
                    }
                    Err(AttemptError::ConcurrentModification) => {
                        return Err(WriteUploadChunkError::Unknown(anyhow::anyhow!(
                            "the chunk bitmap compare-and-swap kept losing after {MAX_CAS_ATTEMPTS} attempts"
                        )));
                    }
                    Err(AttemptError::Expired) => {
                        // `execute_attempt` commits the expiry cleanup and maps
                        // the outcome to `Business`; this arm keeps the match
                        // exhaustive and stays defensive.
                        return Err(WriteUploadChunkError::Expired);
                    }
                    Ok(value) => return Ok(value),
                    Err(AttemptError::Business(error)) => return Err(error),
                }
            }
        })
    }
}

/// Outcome of one attempt at writing a chunk.
enum AttemptError {
    /// A business error the caller must see.
    Business(WriteUploadChunkError),
    /// The bitmap compare-and-swap lost a race and may be retried.
    ConcurrentModification,
    /// The upload expired: its row and staged file have been deleted, so the
    /// unit of work must be committed before returning `Expired`.
    Expired,
}

/// Best-effort delete the expired upload's staged file and row.
///
/// The row deletion is left to the reaper once one exists.
#[expect(
    clippy::single_call_fn,
    reason = "the expiry cleanup is named after the rule it enforces"
)]
async fn delete_expired<U, S>(file_storage: &Arc<S>, unit_of_work: &mut U, upload_id: i64)
where
    U: AssetUnitOfWork,
    S: FileStorage,
{
    if let Err(error) = file_storage.delete_upload_file(upload_id).await {
        error!(error = ?error, "failed to delete the expired upload staged file");
    }
    if let Err(error) = unit_of_work.uploads().delete(upload_id).await {
        error!(error = ?error, "failed to delete the expired upload row");
    }
}

/// Run one chunk-write attempt inside its own unit of work.
#[expect(
    clippy::single_call_fn,
    reason = "the retry loop body is extracted so the attempt owns its unit of work"
)]
async fn execute_attempt<F, S>(
    unit_of_work_factory: &Arc<F>,
    file_storage: &Arc<S>,
    command: &WriteUploadChunkCommand,
    expiry_seconds: u64,
) -> Result<WriteUploadChunkResponse, AttemptError>
where
    F: UnitOfWorkFactory,
    F::Uow: AssetUnitOfWork,
    S: FileStorage,
{
    let mut unit_of_work = unit_of_work_factory
        .begin()
        .await
        .map_err(|error| AttemptError::Business(WriteUploadChunkError::Unknown(error.into())))?;

    let result = persist_chunk(&mut unit_of_work, file_storage, command, expiry_seconds).await;
    finalize_attempt(unit_of_work, result).await
}

/// Return the instant at which an upload created at `created_at` expires.
#[expect(
    clippy::single_call_fn,
    reason = "the derived expiry rule is named for readability"
)]
fn expiry(expiry_seconds: u64, created_at: NaiveDateTime) -> Result<NaiveDateTime, anyhow::Error> {
    let seconds = i64::try_from(expiry_seconds)
        .map_err(|error| anyhow::anyhow!(error).context("the expiry does not fit in i64"))?;
    created_at
        .checked_add_signed(Duration::seconds(seconds))
        .ok_or_else(|| anyhow::anyhow!("the upload expiry overflows the created_at timestamp"))
}

/// Close the attempt's unit of work according to its outcome.
#[expect(
    clippy::single_call_fn,
    reason = "the transaction lifecycle is named for readability"
)]
async fn finalize_attempt<U>(
    unit_of_work: U,
    result: Result<WriteUploadChunkResponse, AttemptError>,
) -> Result<WriteUploadChunkResponse, AttemptError>
where
    U: UnitOfWork,
{
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
            // report the expiry.
            if let Err(commit_error) = unit_of_work.commit().await {
                error!(
                    error = ?commit_error,
                    "failed to commit the expiry cleanup of the write upload chunk unit of work"
                );
            }
            Err(AttemptError::Business(WriteUploadChunkError::Expired))
        }
        Err(error) => {
            log_rollback(unit_of_work).await;
            Err(error)
        }
    }
}

/// Roll back the unit of work, logging a failure without masking the original
/// error.
#[expect(
    clippy::single_call_fn,
    reason = "the rollback logging is named for readability"
)]
async fn log_rollback<U>(unit_of_work: U)
where
    U: UnitOfWork,
{
    if let Err(rollback_error) = unit_of_work.rollback().await {
        error!(
            error = ?rollback_error,
            "failed to roll back the write upload chunk unit of work"
        );
    }
}

/// Hexadecimal-encode `bytes`, lowercase.
#[expect(
    clippy::single_call_fn,
    reason = "the digest encoding is named for readability"
)]
fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut hex = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _result = write!(hex, "{byte:02x}");
    }
    hex
}

/// Return whether `upload` is unfinished and past `expires_at`.
#[expect(
    clippy::single_call_fn,
    reason = "the expiry predicate is named for readability"
)]
fn is_expired(upload: &Upload, expires_at: NaiveDateTime) -> bool {
    !upload.is_finished() && Utc::now().naive_utc() > expires_at
}

/// Persist one chunk inside an open unit of work.
#[expect(
    clippy::single_call_fn,
    reason = "the per-attempt persistence is named for readability"
)]
async fn persist_chunk<U, S>(
    unit_of_work: &mut U,
    file_storage: &Arc<S>,
    command: &WriteUploadChunkCommand,
    expiry_seconds: u64,
) -> Result<WriteUploadChunkResponse, AttemptError>
where
    U: AssetUnitOfWork,
    S: FileStorage,
{
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
    if is_expired(&upload, expires_at) {
        // TODO(reaper): move the expiry cleanup to a background task; this lazy
        // delete keeps the row and the staged file only until the next access.
        delete_expired(file_storage, unit_of_work, upload.upload_id()).await;
        return Err(AttemptError::Expired);
    }

    if upload.is_finished() {
        return Err(AttemptError::Business(
            WriteUploadChunkError::AlreadyFinished,
        ));
    }

    validate_chunk(command, &upload)?;

    let offset = command
        .chunk_number()
        .checked_mul(upload.chunk_size())
        .ok_or(AttemptError::Business(WriteUploadChunkError::InvalidChunk))?;
    let chunk_number = usize::try_from(command.chunk_number()).map_err(|error| {
        AttemptError::Business(WriteUploadChunkError::Unknown(anyhow::anyhow!(error)))
    })?;

    file_storage
        .add_chunk(upload.upload_id(), offset, command.chunk().to_vec())
        .await
        .map_err(|error| AttemptError::Business(WriteUploadChunkError::Unknown(error.into())))?;

    let mut updated = upload;
    updated
        .mark_chunk_received(chunk_number)
        .map_err(|error| AttemptError::Business(WriteUploadChunkError::Unknown(error.into())))?;

    match unit_of_work.uploads().save(updated).await {
        Ok(_) => Ok(WriteUploadChunkResponse::new(true)),
        Err(RepositoryError::ConcurrentModification) => Err(AttemptError::ConcurrentModification),
        Err(error) => Err(AttemptError::Business(WriteUploadChunkError::Unknown(
            error.into(),
        ))),
    }
}

/// Validate the chunk number, its size, and its declared MD5 digest.
#[expect(
    clippy::single_call_fn,
    reason = "the chunk validation is named after the rules it enforces"
)]
fn validate_chunk(command: &WriteUploadChunkCommand, upload: &Upload) -> Result<(), AttemptError> {
    let total_chunks = upload.total_chunks();
    let chunk_number = usize::try_from(command.chunk_number()).map_err(|error| {
        AttemptError::Business(WriteUploadChunkError::Unknown(anyhow::anyhow!(error)))
    })?;
    if chunk_number >= total_chunks {
        return Err(AttemptError::Business(WriteUploadChunkError::InvalidChunk));
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
    // The last chunk may be shorter than `chunk_size`; every other chunk must
    // be exactly `chunk_size`.
    if chunk_number < total_chunks.saturating_sub(1) && chunk_size != command.chunk().len() {
        return Err(AttemptError::Business(WriteUploadChunkError::InvalidChunk));
    }

    let declared = command.content_md5();
    let is_hex = declared.len() == MD5_INTEGRITY_LENGTH
        && declared
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if !is_hex {
        return Err(AttemptError::Business(WriteUploadChunkError::InvalidChunk));
    }

    // Recompute the digest of the received bytes and compare: a chunk whose
    // digest does not match its `Content-MD5` header is rejected.
    let computed = hex_encode(&Md5::digest(command.chunk()));
    if !computed.eq_ignore_ascii_case(declared) {
        return Err(AttemptError::Business(WriteUploadChunkError::InvalidChunk));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;

    use crate::application::port::write_upload_chunk::WriteUploadChunkCommand;
    use crate::application::port::write_upload_chunk::WriteUploadChunkError;
    use crate::application::port::write_upload_chunk::WriteUploadChunkUseCase as _;
    use crate::application::use_case::test_support::QueueAssetTestUnitOfWorkFactory;
    use crate::application::use_case::test_support::asset_unit_of_work;
    use crate::domain::model::upload::ChunkBitmap;
    use crate::domain::model::upload::Upload;
    use crate::domain::port::error::RepositoryError;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_repository::MockFileRepository;
    use crate::domain::port::file_storage::MockFileStorage;
    use crate::domain::port::mime_type_repository::MockMimeTypeRepository;
    use crate::domain::port::upload_repository::MockUploadRepository;

    use super::WriteUploadChunk;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef";
    const CHUNK_MD5: &str = "81dc9bdb52d04dc20036dbd8313ed055";
    const WRONG_CHUNK_MD5: &str = "ffffffffffffffffffffffffffffffff";
    const TTL_SECONDS: u64 = 3600;
    const CHUNK_SIZE: u64 = 4;

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
            DIGEST.to_owned(),
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

    #[tokio::test]
    async fn write_upload_chunk_valid_chunk_persists_bitmap() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        uploads
            .expect_save()
            .times(1)
            .returning(|upload| Box::pin(async move { Ok(upload) }));
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
        let (unit_of_work, committed, rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case =
            WriteUploadChunk::new(Arc::new(factory), Arc::new(file_storage), TTL_SECONDS);
        let command = WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert!(response.received());
        assert!(committed.load(Ordering::SeqCst));
        assert!(!rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_unknown_upload_returns_no_such_upload() -> Result<(), Box<dyn Error>>
    {
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
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(999, 0, b"1234".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::NoSuchUpload)));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_other_user_returns_no_such_upload() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), CHUNK_MD5.to_owned(), 99);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::NoSuchUpload)));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_expired_upload_returns_expired() -> Result<(), Box<dyn Error>> {
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
            DIGEST.to_owned(),
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
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case =
            WriteUploadChunk::new(Arc::new(factory), Arc::new(file_storage), TTL_SECONDS);
        let command = WriteUploadChunkCommand::new(5, 1, b"5678".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::Expired)));
        assert!(committed.load(Ordering::SeqCst));
        assert!(!rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_finished_upload_returns_already_finished()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[0], true)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            TTL_SECONDS,
        );
        let command = WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), CHUNK_MD5.to_owned(), 3);

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
    async fn write_upload_chunk_out_of_range_returns_invalid_chunk() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 4, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            TTL_SECONDS,
        );
        let command = WriteUploadChunkCommand::new(5, 5, b"1234".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::InvalidChunk)));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_wrong_size_returns_invalid_chunk() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            TTL_SECONDS,
        );
        let command = WriteUploadChunkCommand::new(5, 0, b"12".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::InvalidChunk)));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_bad_content_md5_returns_invalid_chunk() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            TTL_SECONDS,
        );
        let command = WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), "oops".to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::InvalidChunk)));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_wrong_digest_returns_invalid_chunk() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut uploads = MockUploadRepository::new();
        expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
        let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
            uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case = WriteUploadChunk::new(
            Arc::new(factory),
            Arc::new(MockFileStorage::new()),
            TTL_SECONDS,
        );
        let command =
            WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), WRONG_CHUNK_MD5.to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::InvalidChunk)));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_storage_failure_returns_unknown() -> Result<(), Box<dyn Error>> {
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
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![unit_of_work]),
        };
        let use_case =
            WriteUploadChunk::new(Arc::new(factory), Arc::new(file_storage), TTL_SECONDS);
        let command = WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::Unknown(_))));
        assert!(rolled_back.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_cas_conflict_retries_and_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut first_uploads = MockUploadRepository::new();
        expect_upload(&mut first_uploads, upload(5, 3, 8, &[], false)?);
        first_uploads
            .expect_save()
            .times(1)
            .returning(|_| Box::pin(async { Err(RepositoryError::ConcurrentModification) }));
        let mut second_uploads = MockUploadRepository::new();
        expect_upload(&mut second_uploads, upload(5, 3, 8, &[], false)?);
        second_uploads
            .expect_save()
            .times(1)
            .returning(|upload| Box::pin(async move { Ok(upload) }));
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_add_chunk()
            .times(2)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        let (first_unit_of_work, first_committed, first_rolled_back) = asset_unit_of_work(
            first_uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let (second_unit_of_work, second_committed, _second_rolled_back) = asset_unit_of_work(
            second_uploads,
            MockFileRepository::new(),
            MockMimeTypeRepository::new(),
        );
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(vec![first_unit_of_work, second_unit_of_work]),
        };
        let use_case =
            WriteUploadChunk::new(Arc::new(factory), Arc::new(file_storage), TTL_SECONDS);
        let command = WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let response = use_case.execute(command).await?;

        // Assert
        assert!(response.received());
        assert!(first_rolled_back.load(Ordering::SeqCst));
        assert!(!first_committed.load(Ordering::SeqCst));
        assert!(second_committed.load(Ordering::SeqCst));
        Ok(())
    }

    #[tokio::test]
    async fn write_upload_chunk_cas_conflict_exhausts_retries_returns_unknown()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut unit_of_works = Vec::new();
        for _ in 0i32..3i32 {
            let mut uploads = MockUploadRepository::new();
            expect_upload(&mut uploads, upload(5, 3, 8, &[], false)?);
            uploads
                .expect_save()
                .times(1)
                .returning(|_| Box::pin(async { Err(RepositoryError::ConcurrentModification) }));
            let (unit_of_work, _committed, _rolled_back) = asset_unit_of_work(
                uploads,
                MockFileRepository::new(),
                MockMimeTypeRepository::new(),
            );
            unit_of_works.push(unit_of_work);
        }
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_add_chunk()
            .times(3)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        let factory = QueueAssetTestUnitOfWorkFactory {
            unit_of_works: Mutex::new(unit_of_works),
        };
        let use_case =
            WriteUploadChunk::new(Arc::new(factory), Arc::new(file_storage), TTL_SECONDS);
        let command = WriteUploadChunkCommand::new(5, 0, b"1234".to_vec(), CHUNK_MD5.to_owned(), 3);

        // Act
        let result = use_case.execute(command).await;

        // Assert
        assert!(matches!(result, Err(WriteUploadChunkError::Unknown(_))));
        Ok(())
    }
}
