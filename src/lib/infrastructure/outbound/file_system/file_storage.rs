use std::future::Future;
use std::io::Error;
use std::io::ErrorKind;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;

use tokio::fs;
use tokio::io::AsyncReadExt as _;
use tokio::io::AsyncSeekExt as _;
use tokio::io::AsyncWriteExt as _;

use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;
use crate::domain::port::content_hasher::ContentHasher;
use crate::domain::port::error::StorageError;
use crate::domain::port::file_storage::FileStorage;

/// Name of the subfolder holding the files being uploaded.
const UPLOADS_FOLDER: &str = "uploads";

/// Name of the subfolder holding the finished files.
const FILES_FOLDER: &str = "files";

/// Size of the buffer used to stream a staged file, in bytes.
const HASH_BUFFER_SIZE: usize = 64 * 1024;

/// File storage storing uploaded content on the local filesystem.
///
/// Files being uploaded live inside the `uploads` subfolder of the root
/// directory, named after the upload identifier. Finished files are moved into
/// the `files` subfolder, named `<upload_id>_<file_name>`.
pub struct FileSystemStorage<H> {
    /// Content hasher computing the whole-file digest.
    hasher: H,
    /// Root directory holding the upload and file folders.
    root: PathBuf,
}

impl<H> FileSystemStorage<H> {
    /// Return the path of the finished files folder.
    fn files_path(&self) -> PathBuf {
        self.root.join(FILES_FOLDER)
    }

    /// Return the relative path of the finished files folder.
    #[expect(
        clippy::single_call_fn,
        reason = "the relative path is a named constant of the adapter layout"
    )]
    fn files_relative() -> PathBuf {
        PathBuf::from(FILES_FOLDER)
    }

    /// Return the path of the finished file of `upload_id`.
    fn final_path(&self, upload_id: NumericID, file_name: &str) -> PathBuf {
        self.files_path().join(format!("{upload_id}_{file_name}"))
    }

    /// Map an I/O error onto a storage error.
    fn map_io_error(err: Error) -> StorageError {
        let kind = err.kind();
        if kind == ErrorKind::NotFound {
            StorageError::Conflict
        } else if kind == ErrorKind::PermissionDenied {
            StorageError::Unavailable
        } else {
            StorageError::Unknown(err.into())
        }
    }

    /// Create a new file system storage rooted at `root`, hashing content with
    /// `hasher`.
    ///
    /// The `uploads` and `files` subfolders are created when they do not exist.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the root or either subfolder cannot be
    /// created.
    pub async fn new(root: PathBuf, hasher: H) -> Result<Self, StorageError> {
        let storage = Self { hasher, root };
        fs::create_dir_all(storage.files_path())
            .await
            .map_err(Self::map_io_error)?;
        fs::create_dir_all(storage.uploads_path())
            .await
            .map_err(Self::map_io_error)?;
        Ok(storage)
    }

    /// Return the path of the staging file of `upload_id`.
    fn staged_path(&self, upload_id: NumericID) -> PathBuf {
        self.uploads_path().join(upload_id.to_string())
    }

    /// Return the path of the uploads staging folder.
    fn uploads_path(&self) -> PathBuf {
        self.root.join(UPLOADS_FOLDER)
    }
}

impl<H: ContentHasher> FileStorage for FileSystemStorage<H> {
    fn add_chunk(
        &self,
        upload_id: NumericID,
        offset: u64,
        chunk: Vec<u8>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let path = self.staged_path(upload_id);

        async move {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .await
                .map_err(Self::map_io_error)?;
            file.seek(SeekFrom::Start(offset))
                .await
                .map_err(Self::map_io_error)?;
            file.write_all(&chunk).await.map_err(Self::map_io_error)?;
            file.flush().await.map_err(Self::map_io_error)?;
            Ok(())
        }
    }

    fn create_upload_file(
        &self,
        upload_id: NumericID,
        file_size: u64,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let path = self.staged_path(upload_id);

        async move {
            // The row was just inserted with an identifier unique among live
            // rows, so a staging file that already exists here is a leftover
            // from an interrupted begin. Reclaim it and retry once instead of
            // failing, so the leftover cannot wedge this identifier.
            let file = match open_new(&path).await {
                Ok(file) => file,
                Err(err) if err.kind() == ErrorKind::AlreadyExists => {
                    fs::remove_file(&path).await.map_err(Self::map_io_error)?;
                    open_new(&path).await.map_err(Self::map_io_error)?
                }
                Err(err) => return Err(Self::map_io_error(err)),
            };
            file.set_len(file_size).await.map_err(Self::map_io_error)?;
            Ok(())
        }
    }

    fn delete_upload_file(
        &self,
        upload_id: NumericID,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let path = self.staged_path(upload_id);

        async move {
            match fs::remove_file(&path).await {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                Err(err) => Err(Self::map_io_error(err)),
            }
        }
    }

    fn integrity_hash(
        &self,
        upload_id: NumericID,
    ) -> impl Future<Output = Result<IntegrityHash<SHA256_HEX_LENGTH>, StorageError>> + Send {
        let path = self.staged_path(upload_id);
        let mut session = self.hasher.hasher();

        async move {
            let mut file = fs::File::open(&path).await.map_err(Self::map_io_error)?;
            let mut buffer = vec![0u8; HASH_BUFFER_SIZE];
            loop {
                let read = file.read(&mut buffer).await.map_err(Self::map_io_error)?;
                if read == 0 {
                    break;
                }
                let chunk = buffer.get(..read).ok_or(StorageError::OperationFailed)?;
                session.update(chunk);
            }
            session
                .finalize()
                .map_err(|error| StorageError::Unknown(error.into()))
        }
    }

    fn promote(
        &self,
        upload_id: NumericID,
        file_name: &str,
    ) -> impl Future<Output = Result<String, StorageError>> + Send {
        let staged = self.staged_path(upload_id);
        let final_path = self.final_path(upload_id, file_name);
        let files_relative = Self::files_relative();
        let final_name = format!("{upload_id}_{file_name}");
        let owned_file_name = file_name.to_owned();

        async move {
            let is_safe_name = is_path_safe(&owned_file_name);
            if !is_safe_name {
                return Err(StorageError::OperationFailed);
            }
            fs::rename(&staged, final_path)
                .await
                .map_err(Self::map_io_error)?;
            let relative = files_relative.join(final_name);
            relative
                .to_str()
                .map(str::to_owned)
                .ok_or(StorageError::OperationFailed)
        }
    }

    fn restore(
        &self,
        upload_id: NumericID,
        file_name: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let staged = self.staged_path(upload_id);
        let final_path = self.final_path(upload_id, file_name);

        async move {
            match fs::rename(&final_path, &staged).await {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                Err(err) => Err(Self::map_io_error(err)),
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

/// Return whether `file_name` is a safe single path segment.
#[expect(
    clippy::single_call_fn,
    reason = "the safety rule is named for readability and reused by the domain model"
)]
fn is_path_safe(file_name: &str) -> bool {
    !file_name.is_empty()
        && !file_name.starts_with('.')
        && !file_name.contains(['/', '\\'])
        && !file_name.contains("..")
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use tempfile::tempdir;
    use tokio::fs;

    use super::FileSystemStorage;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_storage::FileStorage as _;
    use crate::infrastructure::outbound::sha2::content_hasher::Sha2ContentHasher;

    #[tokio::test]
    async fn create_upload_file_creates_preallocated_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;

        // Act
        storage.create_upload_file(42, 12).await?;

        // Assert
        let metadata = fs::metadata(tmp.path().join("uploads/42")).await?;
        assert_eq!(metadata.len(), 12, "the staged file should be preallocated");
        Ok(())
    }

    #[tokio::test]
    async fn create_upload_file_reclaims_stale_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 4).await?;
        storage.add_chunk(42, 0, b"stale".to_vec()).await?;

        // Act
        storage.create_upload_file(42, 12).await?;

        // Assert
        let metadata = fs::metadata(tmp.path().join("uploads/42")).await?;
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
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 8).await?;

        // Act
        storage.add_chunk(42, 4, b"5678".to_vec()).await?;
        storage.add_chunk(42, 0, b"1234".to_vec()).await?;

        // Assert
        let content = fs::read(tmp.path().join("uploads/42")).await?;
        assert_eq!(content, b"12345678");
        Ok(())
    }

    #[tokio::test]
    async fn add_chunk_rewriting_same_offset_is_idempotent() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 4).await?;

        // Act
        storage.add_chunk(42, 0, b"1234".to_vec()).await?;
        storage.add_chunk(42, 0, b"1234".to_vec()).await?;

        // Assert
        let content = fs::read(tmp.path().join("uploads/42")).await?;
        assert_eq!(content, b"1234");
        Ok(())
    }

    #[tokio::test]
    async fn add_chunk_unknown_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;

        // Act
        let result = storage.add_chunk(42, 0, b"bytes".to_vec()).await;

        // Assert
        assert!(matches!(result, Err(StorageError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn integrity_hash_streams_staged_file_and_returns_hex() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 4).await?;
        storage.add_chunk(42, 0, b"1234".to_vec()).await?;

        // Act
        let digest = storage.integrity_hash(42).await?;

        // Assert
        assert_eq!(
            digest.as_str(),
            "03ac674216f3e15c761ee1a5e255f067953623c8b388b4459e13f978d7c846f4"
        );
        Ok(())
    }

    #[tokio::test]
    async fn integrity_hash_empty_staged_file_returns_empty_digest() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 0).await?;

        // Act
        let digest = storage.integrity_hash(42).await?;

        // Assert
        assert_eq!(
            digest.as_str(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        Ok(())
    }

    #[tokio::test]
    async fn integrity_hash_missing_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;

        // Act
        let result = storage.integrity_hash(42).await;

        // Assert
        assert!(matches!(result, Err(StorageError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn delete_upload_file_removes_staged_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 4).await?;

        // Act
        storage.delete_upload_file(42).await?;

        // Assert
        assert!(!tmp.path().join("uploads/42").exists());
        Ok(())
    }

    #[tokio::test]
    async fn delete_upload_file_missing_file_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;

        // Act
        let result = storage.delete_upload_file(42).await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn promote_moves_staged_file_and_returns_relative_path() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 4).await?;
        storage.add_chunk(42, 0, b"1234".to_vec()).await?;

        // Act
        let relative = storage.promote(42, "clip.mp4").await?;

        // Assert
        assert_eq!(relative, "files/42_clip.mp4");
        assert!(tmp.path().join("files/42_clip.mp4").is_file());
        assert!(!tmp.path().join("uploads/42").exists());
        Ok(())
    }

    #[tokio::test]
    async fn promote_path_traversal_file_name_returns_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 4).await?;

        // Act
        let result = storage.promote(42, "../escape.mp4").await;

        // Assert
        assert!(matches!(result, Err(StorageError::OperationFailed)));
        Ok(())
    }

    #[tokio::test]
    async fn promote_unknown_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;

        // Act
        let result = storage.promote(42, "clip.mp4").await;

        // Assert
        assert!(matches!(result, Err(StorageError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn restore_moves_final_file_back_to_staging() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;
        storage.create_upload_file(42, 4).await?;
        storage.add_chunk(42, 0, b"1234".to_vec()).await?;
        let relative = storage.promote(42, "clip.mp4").await?;
        assert_eq!(relative, "files/42_clip.mp4");

        // Act
        storage.restore(42, "clip.mp4").await?;

        // Assert
        assert_eq!(fs::read(tmp.path().join("uploads/42")).await?, b"1234");
        assert!(!tmp.path().join("files/42_clip.mp4").exists());
        Ok(())
    }

    #[tokio::test]
    async fn restore_missing_final_file_succeeds() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf(), Sha2ContentHasher).await?;

        // Act
        let result = storage.restore(42, "clip.mp4").await;

        // Assert
        assert!(result.is_ok());
        Ok(())
    }
}
