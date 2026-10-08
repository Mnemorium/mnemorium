use std::future::Future;
use std::io::Error;
use std::io::ErrorKind;
use std::io::SeekFrom;
use std::path::Path;

use tokio::fs;
use tokio::io::AsyncReadExt as _;
use tokio::io::AsyncSeekExt as _;
use tokio::io::AsyncWriteExt as _;
use tracing::error;

use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::domain::port::content_hasher::ContentHasher as _;
use crate::domain::port::error::StorageError;
use crate::domain::port::file_storage::FileStorage;
use crate::infrastructure::outbound::sha2::content_hasher::Sha2ContentHasher;

/// Size of the buffer used to stream a file, in bytes.
const HASH_BUFFER_SIZE: usize = 64 * 1024;

/// File storage storing uploaded content on the local filesystem.
///
/// The adapter performs raw filesystem I/O on the paths it is given; it holds no
/// layout knowledge and no long-lived adapter. The media-library layout lives in
/// the application layer.
#[derive(Debug, Default)]
pub struct FileSystemStorage;

impl FileSystemStorage {
    /// Map an I/O error onto a storage error.
    ///
    /// A missing entity is never mapped here: the operations for which absence
    /// is a valid outcome handle `NotFound` themselves and return a value.
    ///
    /// The outbound adapter owns the failure log (`OBS-002`): it emits the
    /// catalogued `port_fault` with the error's stable `kind` and the port
    /// `operation` before mapping, so a caller logs nothing (`OBS-003`,
    /// `OBS-006`). Only the classification is logged, never the `io::Error`
    /// `Debug` or a path.
    fn map_io_error(err: Error, operation: &str) -> StorageError {
        let kind = err.kind();
        error!(
            target: "security",
            event = "port_fault",
            kind = ?kind,
            operation = operation,
            "an outbound dependency failed"
        );
        if kind == ErrorKind::PermissionDenied {
            StorageError::Unavailable
        } else {
            StorageError::Unknown(err.into())
        }
    }

    /// Create a new file system storage.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl FileStorage for FileSystemStorage {
    fn add_chunk(
        &self,
        path: &Path,
        offset: u64,
        chunk: Vec<u8>,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        let owned_path = path.to_path_buf();

        async move {
            let mut file = match fs::OpenOptions::new().write(true).open(&owned_path).await {
                Ok(file) => file,
                Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
                Err(err) => return Err(Self::map_io_error(err, "add_chunk")),
            };
            file.seek(SeekFrom::Start(offset))
                .await
                .map_err(|error| Self::map_io_error(error, "add_chunk"))?;
            file.write_all(&chunk)
                .await
                .map_err(|error| Self::map_io_error(error, "add_chunk"))?;
            file.flush()
                .await
                .map_err(|error| Self::map_io_error(error, "add_chunk"))?;
            Ok(true)
        }
    }

    fn create_directory(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let owned_path = path.to_path_buf();

        async move {
            fs::create_dir_all(&owned_path)
                .await
                .map_err(|error| Self::map_io_error(error, "create_directory"))
        }
    }

    fn create_upload_file(
        &self,
        path: &Path,
        file_size: u64,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let owned_path = path.to_path_buf();

        async move {
            // The row was just inserted with an identifier unique among live
            // rows, so a file that already exists here is a leftover from an
            // interrupted begin. Reclaim it and retry once instead of failing,
            // so the leftover cannot wedge this identifier.
            let file = match open_new(&owned_path).await {
                Ok(file) => file,
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    fs::remove_file(&owned_path)
                        .await
                        .map_err(|error| Self::map_io_error(error, "create_upload_file"))?;
                    open_new(&owned_path)
                        .await
                        .map_err(|error| Self::map_io_error(error, "create_upload_file"))?
                }
                Err(err) => return Err(Self::map_io_error(err, "create_upload_file")),
            };
            file.set_len(file_size)
                .await
                .map_err(|error| Self::map_io_error(error, "create_upload_file"))?;
            Ok(())
        }
    }

    fn delete_upload_file(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let owned_path = path.to_path_buf();

        async move {
            match fs::remove_file(&owned_path).await {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                Err(err) => Err(Self::map_io_error(err, "delete_upload_file")),
            }
        }
    }

    fn directory_exist(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        let owned_path = path.to_path_buf();

        async move {
            match fs::metadata(&owned_path).await {
                Ok(metadata) => Ok(metadata.is_dir()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
                Err(err) => Err(Self::map_io_error(err, "directory_exist")),
            }
        }
    }

    fn integrity_hash(
        &self,
        path: &Path,
    ) -> impl Future<Output = Result<Option<IntegrityHash<SHA256_HEX_LENGTH>>, StorageError>> + Send
    {
        let owned_path = path.to_path_buf();
        // The stateless hasher is built here, at the point of use, and never
        // stored by the long-lived storage adapter.
        let mut session = Sha2ContentHasher.hasher();

        async move {
            let mut file = match fs::File::open(&owned_path).await {
                Ok(file) => file,
                Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
                Err(err) => return Err(Self::map_io_error(err, "integrity_hash")),
            };
            let mut buffer = vec![0u8; HASH_BUFFER_SIZE];
            loop {
                let read = file
                    .read(&mut buffer)
                    .await
                    .map_err(|error| Self::map_io_error(error, "integrity_hash"))?;
                if read == 0 {
                    break;
                }
                let chunk = buffer.get(..read).ok_or(StorageError::OperationFailed)?;
                session.update(chunk);
            }
            session
                .finalize()
                .map(Some)
                .map_err(|error| StorageError::Unknown(error.into()))
        }
    }

    fn promote(
        &self,
        staged: &Path,
        final_path: &Path,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send {
        let owned_staged = staged.to_path_buf();
        let owned_final = final_path.to_path_buf();

        async move {
            match fs::rename(&owned_staged, &owned_final).await {
                Ok(()) => Ok(true),
                // `rename` reports `NotFound` both when the source is missing
                // and when a component of the destination path is missing.
                // Probe the source to tell the two apart: only a staged file
                // that is genuinely absent is a value, whereas a missing
                // destination directory is a layout failure, not absence.
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    match fs::metadata(&owned_staged).await {
                        Err(metadata_err) if metadata_err.kind() == ErrorKind::NotFound => {
                            Ok(false)
                        }
                        Ok(_) => {
                            error!(
                                target: "security",
                                event = "port_fault",
                                kind = "destination_missing",
                                operation = "promote",
                                "an outbound dependency failed"
                            );
                            Err(StorageError::OperationFailed)
                        }
                        Err(metadata_err) => Err(Self::map_io_error(metadata_err, "promote")),
                    }
                }
                Err(err) => Err(Self::map_io_error(err, "promote")),
            }
        }
    }

    fn restore(
        &self,
        final_path: &Path,
        staged: &Path,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let owned_final = final_path.to_path_buf();
        let owned_staged = staged.to_path_buf();

        async move {
            match fs::rename(&owned_final, &owned_staged).await {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                Err(err) => Err(Self::map_io_error(err, "restore")),
            }
        }
    }
}

/// Open `path` as a new write-only file, failing when it already exists.
async fn open_new(path: &Path) -> Result<fs::File, Error> {
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use tempfile::tempdir;
    use tokio::fs;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::FileSystemStorage;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_storage::FileStorage as _;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the raw `write` and `flush` are meaningful for an in-memory capture buffer"
    )]
    impl stdio::Write for CaptureWriter {
        fn flush(&mut self) -> stdio::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> stdio::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// Hand every formatted event to a fresh clone of the shared buffer.
    #[expect(
        clippy::missing_trait_methods,
        reason = "the default `make_writer_for` already routes through `make_writer`"
    )]
    impl<'writer> fmt::MakeWriter<'writer> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    #[expect(
        clippy::single_call_fn,
        reason = "the module-local capture helper mirrors the pattern the other adapter suites use"
    )]
    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let guard = set_default(subscriber);
        (buffer, guard)
    }

    /// Read the captured bytes back as a lossy UTF-8 string.
    #[expect(
        clippy::single_call_fn,
        reason = "the module-local read-back helper mirrors the pattern the other adapter suites use"
    )]
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

    #[tokio::test]
    async fn create_upload_file_creates_preallocated_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");
        let uploads = tmp.path().join("uploads");
        storage.create_directory(&uploads).await?;

        // Act
        storage.create_upload_file(&path, 12).await?;

        // Assert
        let metadata = fs::metadata(&path).await?;
        assert_eq!(metadata.len(), 12, "the staged file should be preallocated");
        Ok(())
    }

    #[tokio::test]
    async fn create_upload_file_reclaims_stale_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");
        let uploads = tmp.path().join("uploads");
        storage.create_directory(&uploads).await?;
        storage.create_upload_file(&path, 4).await?;
        storage.add_chunk(&path, 0, b"stale".to_vec()).await?;

        // Act
        storage.create_upload_file(&path, 12).await?;

        // Assert
        let metadata = fs::metadata(&path).await?;
        assert_eq!(
            metadata.len(),
            12,
            "reclaiming the stale file should reset it to the requested size"
        );
        Ok(())
    }

    #[tokio::test]
    async fn add_chunk_writes_bytes_at_their_offset() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");
        let uploads = tmp.path().join("uploads");
        storage.create_directory(&uploads).await?;
        storage.create_upload_file(&path, 8).await?;

        // Act
        storage.add_chunk(&path, 4, b"5678".to_vec()).await?;
        storage.add_chunk(&path, 0, b"1234".to_vec()).await?;

        // Assert
        let content = fs::read(&path).await?;
        assert_eq!(content, b"12345678");
        Ok(())
    }

    #[tokio::test]
    async fn add_chunk_rewriting_same_offset_is_idempotent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");
        let uploads = tmp.path().join("uploads");
        storage.create_directory(&uploads).await?;
        storage.create_upload_file(&path, 4).await?;

        // Act
        storage.add_chunk(&path, 0, b"1234".to_vec()).await?;
        storage.add_chunk(&path, 0, b"1234".to_vec()).await?;

        // Assert
        let content = fs::read(&path).await?;
        assert_eq!(content, b"1234");
        Ok(())
    }

    #[tokio::test]
    async fn add_chunk_missing_file_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");

        // Act
        let result = storage.add_chunk(&path, 0, b"bytes".to_vec()).await;

        // Assert
        assert!(!result?);
        Ok(())
    }

    #[tokio::test]
    async fn integrity_hash_streams_staged_file_and_returns_hex() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");
        let uploads = tmp.path().join("uploads");
        storage.create_directory(&uploads).await?;
        storage.create_upload_file(&path, 4).await?;
        storage.add_chunk(&path, 0, b"1234".to_vec()).await?;

        // Act
        let digest = storage.integrity_hash(&path).await?;

        // Assert
        assert_eq!(
            digest.ok_or("the staged file should exist")?.as_str(),
            "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4"
        );
        Ok(())
    }

    #[tokio::test]
    async fn integrity_hash_empty_staged_file_returns_empty_digest() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");
        let uploads = tmp.path().join("uploads");
        storage.create_directory(&uploads).await?;
        storage.create_upload_file(&path, 0).await?;

        // Act
        let digest = storage.integrity_hash(&path).await?;

        // Assert
        assert_eq!(
            digest.ok_or("the staged file should exist")?.as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        Ok(())
    }

    #[tokio::test]
    async fn integrity_hash_missing_file_returns_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");

        // Act
        let result = storage.integrity_hash(&path).await;

        // Assert
        assert!(result?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn delete_upload_file_removes_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");
        let uploads = tmp.path().join("uploads");
        storage.create_directory(&uploads).await?;
        storage.create_upload_file(&path, 4).await?;

        // Act
        storage.delete_upload_file(&path).await?;

        // Assert
        assert!(!path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn delete_upload_file_missing_file_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads").join("42");

        // Act
        let result = storage.delete_upload_file(&path).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn promote_moves_staged_file_to_final_path() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let staged = tmp.path().join("uploads").join("42");
        let final_path = tmp.path().join("files").join("42_clip.mp4");
        storage
            .create_directory(&tmp.path().join("uploads"))
            .await?;
        storage.create_directory(&tmp.path().join("files")).await?;
        storage.create_upload_file(&staged, 4).await?;
        storage.add_chunk(&staged, 0, b"1234".to_vec()).await?;

        // Act
        storage.promote(&staged, &final_path).await?;

        // Assert
        assert!(final_path.is_file());
        assert!(!staged.exists());
        Ok(())
    }

    #[tokio::test]
    async fn promote_missing_staged_file_returns_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let staged = tmp.path().join("uploads").join("42");
        let final_path = tmp.path().join("files").join("42_clip.mp4");
        // The destination directory exists, so the only missing component is
        // the staged file itself.
        storage.create_directory(&tmp.path().join("files")).await?;

        // Act
        let result = storage.promote(&staged, &final_path).await;

        // Assert
        assert!(!result?);
        Ok(())
    }

    #[tokio::test]
    async fn promote_staged_file_missing_destination_directory_returns_operation_failed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let (buffer, _capture) = capture_logs();
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let staged = tmp.path().join("uploads").join("42");
        let final_path = tmp.path().join("files").join("42_clip.mp4");
        storage
            .create_directory(&tmp.path().join("uploads"))
            .await?;
        storage.create_upload_file(&staged, 4).await?;
        storage.add_chunk(&staged, 0, b"1234".to_vec()).await?;

        // Act
        let result = storage.promote(&staged, &final_path).await;

        // Assert
        assert!(matches!(result, Err(StorageError::OperationFailed)));
        assert!(
            staged.is_file(),
            "a failed promote must leave the staged file in place"
        );
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("ERROR").count(),
            1,
            "a promote fault must emit exactly one error event: {logs}"
        );
        assert!(
            logs.contains("event=\"port_fault\""),
            "the event must be port_fault: {logs}"
        );
        assert!(
            logs.contains("kind=\"destination_missing\""),
            "the event must carry the stable kind: {logs}"
        );
        assert!(
            logs.contains("operation=\"promote\""),
            "the event must name the owning operation: {logs}"
        );
        let staged_text = staged.to_string_lossy();
        let final_text = final_path.to_string_lossy();
        assert!(
            !logs.contains(staged_text.as_ref()),
            "the sink must never carry the staged path: {logs}"
        );
        assert!(
            !logs.contains(final_text.as_ref()),
            "the sink must never carry the final path: {logs}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn restore_moves_final_file_back_to_staging() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let staged = tmp.path().join("uploads").join("42");
        let final_path = tmp.path().join("files").join("42_clip.mp4");
        storage
            .create_directory(&tmp.path().join("uploads"))
            .await?;
        storage.create_directory(&tmp.path().join("files")).await?;
        storage.create_upload_file(&staged, 4).await?;
        storage.add_chunk(&staged, 0, b"1234".to_vec()).await?;
        storage.promote(&staged, &final_path).await?;

        // Act
        storage.restore(&final_path, &staged).await?;

        // Assert
        assert_eq!(fs::read(&staged).await?, b"1234");
        assert!(!final_path.exists());
        Ok(())
    }

    #[tokio::test]
    async fn restore_missing_final_file_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let staged = tmp.path().join("uploads").join("42");
        let final_path = tmp.path().join("files").join("42_clip.mp4");

        // Act
        let result = storage.restore(&final_path, &staged).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn directory_exist_reports_missing_then_existing() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("uploads");

        // Act
        let before = storage.directory_exist(&path).await?;
        storage.create_directory(&path).await?;
        let after = storage.directory_exist(&path).await?;

        // Assert
        assert!(!before);
        assert!(after);
        Ok(())
    }

    #[tokio::test]
    async fn create_directory_creates_nested_and_is_idempotent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let path = tmp.path().join("files").join("nested");

        // Act
        storage.create_directory(&path).await?;
        storage.create_directory(&path).await?;

        // Assert
        assert!(path.is_dir());
        Ok(())
    }
}
