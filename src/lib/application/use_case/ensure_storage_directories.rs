use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::application::port::ensure_storage_directories::EnsureStorageDirectoriesCommand;
use crate::application::port::ensure_storage_directories::EnsureStorageDirectoriesError;
use crate::application::port::ensure_storage_directories::EnsureStorageDirectoriesResponse;
use crate::application::port::ensure_storage_directories::EnsureStorageDirectoriesUseCase;
use crate::application::use_case::upload_layout::media_root;
use crate::application::use_case::upload_layout::uploads_path;
use crate::domain::port::file_storage::FileStorage;

/// Use case implementation for ensuring the storage folders exist.
pub struct EnsureStorageDirectories<S> {
    /// Storage adapter creating the folders.
    file_storage: Arc<S>,
}

impl<S: FileStorage> EnsureStorageDirectories<S> {
    /// Create a new use case.
    #[must_use]
    pub fn new(file_storage: Arc<S>) -> Self {
        Self { file_storage }
    }
}

impl<S: FileStorage> EnsureStorageDirectoriesUseCase for EnsureStorageDirectories<S> {
    fn execute<'future>(
        &'future self,
        _command: EnsureStorageDirectoriesCommand,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        EnsureStorageDirectoriesResponse,
                        EnsureStorageDirectoriesError,
                    >,
                > + Send
                + 'future,
        >,
    > {
        let file_storage = Arc::clone(&self.file_storage);

        Box::pin(async move {
            // The staging area is server-owned, so it is recreated when absent.
            let staging = uploads_path();
            let exist = file_storage
                .directory_exist(staging.as_path())
                .await
                .map_err(|error| EnsureStorageDirectoriesError::Unknown(error.into()))?;
            if !exist {
                file_storage
                    .create_directory(staging.as_path())
                    .await
                    .map_err(|error| EnsureStorageDirectoriesError::Unknown(error.into()))?;
            }

            // The library root is the operator's mount: it is never created.
            // Absence is reported so the caller can warn once and continue.
            let media_root_present = file_storage
                .directory_exist(media_root().as_path())
                .await
                .map_err(|error| EnsureStorageDirectoriesError::Unknown(error.into()))?;
            Ok(EnsureStorageDirectoriesResponse::new(media_root_present))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use crate::application::use_case::upload_layout::media_root;
    use crate::application::use_case::upload_layout::uploads_path;
    use crate::domain::port::error::StorageError;
    use crate::domain::port::file_storage::MockFileStorage;

    use super::EnsureStorageDirectories;
    use super::EnsureStorageDirectoriesCommand;
    use super::EnsureStorageDirectoriesUseCase as _;

    #[tokio::test]
    async fn ensure_storage_directories_creates_only_the_staging_folder()
    -> Result<(), Box<dyn Error>> {
        // Arrange: neither folder exists.
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .times(2)
            .returning(|_| Box::pin(async { Ok(false) }));
        file_storage
            .expect_create_directory()
            .times(1)
            .withf(|path| path == uploads_path())
            .returning(|_| Box::pin(async { Ok(()) }));
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage));

        // Act
        let response = use_case
            .execute(EnsureStorageDirectoriesCommand::new())
            .await?;

        // Assert: no `create_directory` call addressed the library root.
        assert!(!response.media_root_present());
        Ok(())
    }

    #[tokio::test]
    async fn ensure_storage_directories_reports_a_present_media_root() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .withf(|path| path == media_root())
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        file_storage
            .expect_directory_exist()
            .withf(|path| path == uploads_path())
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        file_storage.expect_create_directory().times(0);
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage));

        // Act
        let response = use_case
            .execute(EnsureStorageDirectoriesCommand::new())
            .await?;

        // Assert
        assert!(response.media_root_present());
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
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage));

        // Act
        let result = use_case
            .execute(EnsureStorageDirectoriesCommand::new())
            .await;

        // Assert
        assert!(result.is_err());
        Ok(())
    }
}
