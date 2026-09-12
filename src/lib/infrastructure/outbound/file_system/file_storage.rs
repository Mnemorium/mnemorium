use std::future::Future;
use std::io::Error;
use std::io::ErrorKind;
use std::path::PathBuf;

use tokio::fs;
use tokio::io::AsyncWriteExt as _;

use crate::domain::alias::NumericID;
use crate::domain::port::error::StorageError;
use crate::domain::port::file_storage::FileStorage;
use crate::domain::port::file_storage::UploadId;

/// Name of the subfolder holding the files being uploaded.
const STAGING_FOLDER: &str = "staging";

/// Separator delimiting the fields of a staging file name.
const NAME_SEPARATOR: &str = "__";

/// File storage storing uploaded content on the local filesystem.
///
/// Files being uploaded live inside the `staging` subfolder of the root
/// directory. Their name encodes the upload identifier, the owning user, the
/// media type and the original file name:
/// `<id>__<user_id>__<mime_type_id>__<file_name>`, with the `/` of the media
/// type encoded as `+`.
pub struct FileSystemStorage {
    /// Root directory holding the staging folder.
    root: PathBuf,
}

impl FileSystemStorage {
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

    /// Create a new file system storage rooted at `root`.
    ///
    /// The `staging` subfolder is created when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the root or the staging folder cannot be
    /// created.
    pub async fn new(root: PathBuf) -> Result<Self, StorageError> {
        let storage = Self { root };
        fs::create_dir_all(storage.staging_path())
            .await
            .map_err(Self::map_io_error)?;
        Ok(storage)
    }

    /// Return the path of the staging folder.
    fn staging_path(&self) -> PathBuf {
        self.root.join(STAGING_FOLDER)
    }
}

impl FileStorage for FileSystemStorage {
    fn append_chunk(
        &self,
        id: &UploadId,
        chunk: Vec<u8>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let path = self.staging_path();
        let prefix = format!("{}{NAME_SEPARATOR}", id.value());

        async move {
            let mut entries = fs::read_dir(&path).await.map_err(Self::map_io_error)?;
            let mut match_found = None;
            while let Some(entry) = entries.next_entry().await.map_err(Self::map_io_error)? {
                if entry.file_name().to_string_lossy().starts_with(&prefix) {
                    match_found = Some(entry.path());
                    break;
                }
            }
            let staging = match_found.ok_or(StorageError::Conflict)?;
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&staging)
                .await
                .map_err(Self::map_io_error)?;
            file.write_all(&chunk).await.map_err(Self::map_io_error)?;
            Ok(())
        }
    }

    fn begin(
        &self,
        id: &UploadId,
        file_name: &str,
        user_id: NumericID,
        mime_type_id: &str,
    ) -> impl Future<Output = Result<(), StorageError>> + Send {
        let staging_path = self.staging_path();
        let upload_id = id.value().to_owned();

        async move {
            let is_safe_name = !file_name.is_empty()
                && !file_name.starts_with('.')
                && !file_name.contains(['/', '\\'])
                && !file_name.contains("..");
            if !is_safe_name {
                return Err(StorageError::OperationFailed);
            }
            fs::create_dir_all(&staging_path)
                .await
                .map_err(Self::map_io_error)?;
            let id_prefix = format!("{upload_id}{NAME_SEPARATOR}");
            let mut entries = fs::read_dir(&staging_path)
                .await
                .map_err(Self::map_io_error)?;
            while let Some(entry) = entries.next_entry().await.map_err(Self::map_io_error)? {
                if entry.file_name().to_string_lossy().starts_with(&id_prefix) {
                    return Err(StorageError::Conflict);
                }
            }
            let encoded_mime = mime_type_id.replace('/', "+");
            let staging = staging_path.join(format!(
                "{upload_id}{NAME_SEPARATOR}{user_id}{NAME_SEPARATOR}{encoded_mime}{NAME_SEPARATOR}{file_name}",
            ));
            fs::File::create(&staging)
                .await
                .map_err(Self::map_io_error)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use tempfile::tempdir;
    use tokio::fs;

    use super::FileSystemStorage;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_storage::FileStorage as _;
    use crate::domain::port::file_storage::UploadId;

    #[tokio::test]
    async fn begin_missing_staging_folder_creates_it() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf()).await?;
        fs::remove_dir(tmp.path().join("staging")).await?;
        let id = UploadId::new("42".to_owned());

        // Act
        storage.begin(&id, "clip.mp4", 7, "video/mp4").await?;

        // Assert
        assert!(
            tmp.path()
                .join("staging/42__7__video+mp4__clip.mp4")
                .exists(),
            "the staging folder and file should exist after begin"
        );
        Ok(())
    }

    #[tokio::test]
    async fn begin_valid_file_creates_encoded_staging_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf()).await?;
        let id = UploadId::new("42".to_owned());

        // Act
        storage.begin(&id, "clip.mp4", 7, "video/mp4").await?;

        // Assert
        assert!(
            tmp.path()
                .join("staging/42__7__video+mp4__clip.mp4")
                .exists(),
            "the staging file should carry the encoded mime type and file name"
        );
        Ok(())
    }

    #[tokio::test]
    async fn begin_existing_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf()).await?;
        let id = UploadId::new("42".to_owned());
        storage.begin(&id, "clip.mp4", 7, "video/mp4").await?;

        // Act
        let result = storage.begin(&id, "other.mp4", 7, "video/mp4").await;

        // Assert
        assert!(matches!(result, Err(StorageError::Conflict)));
        Ok(())
    }

    #[tokio::test]
    async fn begin_path_traversal_file_name_returns_error() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf()).await?;
        let id = UploadId::new("42".to_owned());

        // Act
        let result = storage.begin(&id, "../escape.mp4", 7, "video/mp4").await;

        // Assert
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn append_chunk_writes_bytes_in_order() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf()).await?;
        let id = UploadId::new("42".to_owned());
        storage.begin(&id, "clip.mp4", 7, "video/mp4").await?;

        // Act
        storage.append_chunk(&id, b"chunk-one-".to_vec()).await?;
        storage.append_chunk(&id, b"chunk-two".to_vec()).await?;

        // Assert
        let content = fs::read(tmp.path().join("staging/42__7__video+mp4__clip.mp4")).await?;
        assert_eq!(content, b"chunk-one-chunk-two");
        Ok(())
    }

    #[tokio::test]
    async fn append_chunk_unknown_upload_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let tmp = tempdir()?;
        let storage = FileSystemStorage::new(tmp.path().to_path_buf()).await?;
        let id = UploadId::new("42".to_owned());

        // Act
        let result = storage.append_chunk(&id, b"bytes".to_vec()).await;

        // Assert
        assert!(matches!(result, Err(StorageError::Conflict)));
        Ok(())
    }
}
