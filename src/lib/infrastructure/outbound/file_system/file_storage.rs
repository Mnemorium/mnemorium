use std::future::Future;
use std::io::Error;
use std::io::ErrorKind;
use std::io::SeekFrom;
use std::path::Path;

use chrono::DateTime;
use chrono::Utc;
use tokio::fs;
use tokio::io::AsyncReadExt as _;
use tokio::io::AsyncSeekExt as _;
use tokio::io::AsyncWriteExt as _;
use tracing::error;

use crate::domain::model::integrity_hash::DIGEST_HEX_LENGTH;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::port::content_hasher::ContentHasher as _;
use crate::domain::port::error::StorageError;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::file_storage::PromotedFile;
use crate::infrastructure::outbound::sha2::content_hasher::Sha2ContentHasher;

/// Size of the buffer used to stream a file, in bytes.
const HASH_BUFFER_BYTES: usize = 64 * 1024;

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
    /// Classify a promotion whose source or destination component is missing.
    async fn classify_missing_promotion(
        &self,
        staged: &Path,
    ) -> Result<Option<PromotedFile>, StorageError> {
        match fs::metadata(staged).await {
            Err(metadata_err) if metadata_err.kind() == ErrorKind::NotFound => Ok(None),
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

    /// Read the promoted content's size and modification instant.
    async fn promoted_metadata(path: &Path) -> Result<PromotedFile, StorageError> {
        let metadata = fs::metadata(path)
            .await
            .map_err(|error| Self::map_io_error(error, "promote"))?;
        let size_bytes =
            i64::try_from(metadata.len()).map_err(|_error| StorageError::OperationFailed)?;
        let modified_at = metadata
            .modified()
            .map(|instant| DateTime::<Utc>::from(instant).naive_utc())
            .map_err(|error| Self::map_io_error(error, "promote"))?;
        Ok(PromotedFile {
            modified_at,
            size_bytes,
        })
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
    ) -> impl Future<Output = Result<Option<IntegrityHash<DIGEST_HEX_LENGTH>>, StorageError>> + Send
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
            let mut buffer = vec![0u8; HASH_BUFFER_BYTES];
            loop {
                let read = file
                    .read(&mut buffer)
                    .await
                    .map_err(|error| Self::map_io_error(error, "integrity_hash"))?;
                if read == 0 {
                    break;
                }
                // The read length can never exceed the buffer, so this guard is
                // unreachable; if it ever fires it is an adapter fault the
                // outbound layer owns, logged once as the catalogued
                // `port_fault` with only its stable `kind` (`OBS-002`,
                // `OBS-003`, `OBS-006`).
                let chunk = buffer.get(..read).ok_or_else(|| {
                    error!(
                        target: "security",
                        event = "port_fault",
                        kind = "buffer_slice",
                        operation = "integrity_hash",
                        "an outbound dependency failed"
                    );
                    StorageError::OperationFailed
                })?;
                session.update(chunk);
            }
            session.finalize().map(Some).map_err(|error| {
                error!(
                    target: "security",
                    event = "port_fault",
                    kind = "hash_finalize",
                    operation = "integrity_hash",
                    "an outbound dependency failed"
                );
                StorageError::Unknown(error.into())
            })
        }
    }

    fn promote(
        &self,
        staged: &Path,
        final_path: &Path,
    ) -> impl Future<Output = Result<Option<PromotedFile>, StorageError>> + Send {
        let owned_staged = staged.to_path_buf();
        let owned_final = final_path.to_path_buf();

        async move {
            match fs::rename(&owned_staged, &owned_final).await {
                Ok(()) => Self::promoted_metadata(&owned_final).await.map(Some),
                // The staging area and the library root can live on different
                // filesystems (the root is the operator's mount, the staging
                // area is the container's writable layer), so a rename cannot
                // cross the boundary. Copy the bytes and drop the staging copy
                // instead; a failed cleanup leaves a redundant staged file, not
                // a lost promotion.
                Err(err) if err.kind() == ErrorKind::CrossesDevices => {
                    match fs::copy(&owned_staged, &owned_final).await {
                        Ok(_bytes) => {
                            // The bytes are promoted; a staging copy that cannot
                            // be removed is left for the reaper.
                            let _removed = fs::remove_file(&owned_staged).await;
                            Self::promoted_metadata(&owned_final).await.map(Some)
                        }
                        Err(copy_err) if copy_err.kind() == ErrorKind::NotFound => {
                            self.classify_missing_promotion(&owned_staged).await
                        }
                        Err(copy_err) => Err(Self::map_io_error(copy_err, "promote")),
                    }
                }
                // `rename` reports `NotFound` both when the source is missing
                // and when a component of the destination path is missing.
                // Probe the source to tell the two apart: only a staged file
                // that is genuinely absent is a value, whereas a missing
                // destination directory is a layout failure, not absence.
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    self.classify_missing_promotion(&owned_staged).await
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

    use tempfile::Builder;
    use tempfile::tempdir;
    use tokio::fs;

    use super::FileSystemStorage;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_storage::FileStorage as _;
    use tracing_test::traced_test;

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
    #[traced_test]
    async fn delete_upload_file_failure_logs_a_port_fault() -> Result<(), Box<dyn Error>> {
        // Arrange: a directory cannot be removed by `remove_file`, so the failure
        // branch maps the I/O error and emits the catalogued fault.
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new();
        let directory = tmp.path().join("files");
        storage.create_directory(&directory).await?;

        // Act
        let result = storage.delete_upload_file(&directory).await;

        // Assert
        assert!(matches!(result, Err(StorageError::Unknown(_))));
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("ERROR")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("a delete fault must emit exactly one error event (found {count})")
            })
        });
        assert!(
            logs_contain("event=\"port_fault\""),
            "the event must be port_fault"
        );
        assert!(
            logs_contain("kind=IsADirectory"),
            "the event must carry the stable kind"
        );
        assert!(
            logs_contain("operation=\"delete_upload_file\""),
            "the event must name the owning operation"
        );
        let directory_text = directory.to_string_lossy();
        assert!(
            !logs_contain(directory_text.as_ref()),
            "the sink must never carry the path"
        );
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn promote_copies_across_filesystems() -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::MetadataExt as _;

        // Arrange: the staging area and the library root on different devices,
        // as they are when the root is a bind mount.
        let staged_dir = tempdir()?;
        let final_dir = Builder::new().tempdir_in("/dev/shm")?;
        // On a host where both live on one device the rename path runs instead;
        // the promotion still succeeds, and the assertions hold either way.
        let _devices = (
            staged_dir.path().metadata()?.dev(),
            final_dir.path().metadata()?.dev(),
        );
        let storage = FileSystemStorage::new();
        let staged = staged_dir.path().join("42");
        let final_path = final_dir.path().join("42_clip.mp4");
        storage.create_upload_file(&staged, 4).await?;
        storage.add_chunk(&staged, 0, b"1234".to_vec()).await?;

        // Act
        let promoted = storage
            .promote(&staged, &final_path)
            .await?
            .ok_or("the file should have been promoted")?;

        // Assert: the bytes crossed the boundary, the staging copy is gone,
        // and the promoted metadata describes the final file.
        assert_eq!(promoted.size_bytes, 4);
        assert_eq!(fs::read(&final_path).await?, b"1234");
        assert!(!staged.exists());
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
        let promoted = storage
            .promote(&staged, &final_path)
            .await?
            .ok_or("the file should have been promoted")?;

        // Assert
        assert_eq!(promoted.size_bytes, 4);
        assert!(final_path.is_file());
        assert!(!staged.exists());
        Ok(())
    }

    #[tokio::test]
    async fn promote_missing_staged_file_returns_none() -> Result<(), Box<dyn Error>> {
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
        assert!(result?.is_none());
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn promote_staged_file_missing_destination_directory_returns_operation_failed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
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
        logs_assert(|lines| {
            let count = lines.iter().filter(|line| line.contains("ERROR")).count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!("a promote fault must emit exactly one error event (found {count})")
            })
        });
        assert!(
            logs_contain("event=\"port_fault\""),
            "the event must be port_fault"
        );
        assert!(
            logs_contain("kind=\"destination_missing\""),
            "the event must carry the stable kind"
        );
        assert!(
            logs_contain("operation=\"promote\""),
            "the event must name the owning operation"
        );
        let staged_text = staged.to_string_lossy();
        let final_text = final_path.to_string_lossy();
        assert!(
            !logs_contain(staged_text.as_ref()),
            "the sink must never carry the staged path"
        );
        assert!(
            !logs_contain(final_text.as_ref()),
            "the sink must never carry the final path"
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
