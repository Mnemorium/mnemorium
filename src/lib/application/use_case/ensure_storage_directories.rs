use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::ensure_storage_directories::EnsureStorageDirectoriesCommand;
use crate::application::port::ensure_storage_directories::EnsureStorageDirectoriesError;
use crate::application::port::ensure_storage_directories::EnsureStorageDirectoriesUseCase;
use crate::application::use_case::upload_layout::files_path;
use crate::application::use_case::upload_layout::uploads_path;
use crate::domain::port::file_storage::FileStorage;

/// Use case implementation for ensuring the storage folders exist.
pub struct EnsureStorageDirectories<S> {
    /// Storage adapter creating the folders.
    file_storage: Arc<S>,
    /// Root directory holding the upload and file folders.
    root: PathBuf,
}

impl<S: FileStorage> EnsureStorageDirectories<S> {
    /// Create a new use case.
    #[must_use]
    pub fn new(file_storage: Arc<S>, root: PathBuf) -> Self {
        Self { file_storage, root }
    }
}

impl<S: FileStorage> EnsureStorageDirectoriesUseCase for EnsureStorageDirectories<S> {
    fn execute<'future>(
        &'future self,
        _command: EnsureStorageDirectoriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnsureStorageDirectoriesError>> + Send + 'future>>
    {
        let root = self.root.clone();
        let file_storage = Arc::clone(&self.file_storage);

        Box::pin(async move {
            for path in [uploads_path(&root), files_path(&root)] {
                let exist = file_storage
                    .directory_exist(path.as_path())
                    .await
                    .map_err(|error| EnsureStorageDirectoriesError::Unknown(error.into()))?;
                if !exist {
                    file_storage
                        .create_directory(path.as_path())
                        .await
                        .map_err(|error| EnsureStorageDirectoriesError::Unknown(error.into()))?;
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_storage::MockFileStorage;

    use super::EnsureStorageDirectories;
    use super::EnsureStorageDirectoriesCommand;
    use super::EnsureStorageDirectoriesUseCase as _;

    /// A root the fake storage reports its paths against.
    const ROOT: &str = "/storage";

    #[tokio::test]
    async fn ensure_storage_directories_missing_creates_both() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .times(2)
            .returning(|_| Box::pin(async { Ok(false) }));
        file_storage
            .expect_create_directory()
            .times(2)
            .returning(|_| Box::pin(async { Ok(()) }));
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage), PathBuf::from(ROOT));

        // Act
        use_case
            .execute(EnsureStorageDirectoriesCommand::new())
            .await?;

        // Assert
        Ok(())
    }

    #[tokio::test]
    async fn ensure_storage_directories_existing_creates_none() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .times(2)
            .returning(|_| Box::pin(async { Ok(true) }));
        file_storage.expect_create_directory().times(0);
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage), PathBuf::from(ROOT));

        // Act
        use_case
            .execute(EnsureStorageDirectoriesCommand::new())
            .await?;

        // Assert
        Ok(())
    }

    #[tokio::test]
    async fn ensure_storage_directories_probe_failure_returns_unknown() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .returning(|_| Box::pin(async { Err(StorageError::Unavailable) }));
        file_storage.expect_create_directory().times(0);
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage), PathBuf::from(ROOT));

        // Act
        let result = use_case
            .execute(EnsureStorageDirectoriesCommand::new())
            .await;

        // Assert
        assert!(result.is_err());
        Ok(())
    }
}
