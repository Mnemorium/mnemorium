use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::application::port::{
    EnsureStorageDirectoriesCommand, EnsureStorageDirectoriesError,
    EnsureStorageDirectoriesResponse, EnsureStorageDirectoriesUseCase,
};
use crate::application::use_case::{media_root, uploads_path};
use crate::domain::port::FileStorage;

/// Interval between two probes of the library root, in seconds.
const WAIT_POLL_SECONDS: u64 = 1;

/// Use case implementation for ensuring the storage folders exist.
pub(crate) struct EnsureStorageDirectories<S> {
    /// Storage adapter creating the folders.
    file_storage: Arc<S>,
}

impl<S: FileStorage> EnsureStorageDirectories<S> {
    /// Create a new use case.
    #[must_use]
    pub(crate) fn new(file_storage: Arc<S>) -> Self {
        Self { file_storage }
    }
}

impl<S: FileStorage> EnsureStorageDirectoriesUseCase for EnsureStorageDirectories<S> {
    fn execute<'future>(
        &'future self,
        command: EnsureStorageDirectoriesCommand,
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
            let mut media_root_present = file_storage
                .directory_exist(media_root().as_path())
                .await
                .map_err(|error| EnsureStorageDirectoriesError::Unknown(error.into()))?;

            // Only a required root delays startup: a mount that may still
            // appear is waited for, and any other absence is decided at once so
            // readiness never waits for it. The setting is bounded by the
            // domain, so the deadline is representable; `checked_add` keeps the
            // degradation to "decide now" if it ever is not. The wait blocks
            // this worker: the composition root awaits it before the server
            // binds, and `STY-RUST-075` keeps the async clock out of a use case.
            if !media_root_present
                && command.required()
                && let Some(deadline) =
                    Instant::now().checked_add(Duration::from_secs(command.wait_seconds()))
            {
                while !media_root_present && Instant::now() < deadline {
                    thread::sleep(Duration::from_secs(WAIT_POLL_SECONDS));
                    media_root_present = file_storage
                        .directory_exist(media_root().as_path())
                        .await
                        .map_err(|error| EnsureStorageDirectoriesError::Unknown(error.into()))?;
                }
            }
            Ok(EnsureStorageDirectoriesResponse::new(media_root_present))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::port::{MockFileStorage, StorageError};

    use std::error::Error;

    #[tokio::test]
    async fn a_command_carries_its_wait_policy() {
        // Act
        let command = EnsureStorageDirectoriesCommand::new().waiting(7, true);

        // Assert
        assert!(command.required());
        assert_eq!(command.wait_seconds(), 7);
    }

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
    async fn an_absent_root_is_not_waited_for_when_it_is_not_required() -> Result<(), Box<dyn Error>>
    {
        // Arrange: one probe per folder, then the decision is immediate.
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .withf(|path| path == uploads_path())
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        file_storage
            .expect_directory_exist()
            .withf(|path| path == media_root())
            .times(1)
            .returning(|_| Box::pin(async { Ok(false) }));
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage));

        // Act
        let response = use_case
            .execute(EnsureStorageDirectoriesCommand::new().waiting(30, false))
            .await?;

        // Assert
        assert!(!response.media_root_present());
        Ok(())
    }

    #[tokio::test]
    async fn a_required_root_appearing_within_the_window_is_reported_present()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the root is absent once, then present.
        let mut file_storage = MockFileStorage::new();
        file_storage
            .expect_directory_exist()
            .withf(|path| path == uploads_path())
            .times(1)
            .returning(|_| Box::pin(async { Ok(true) }));
        let mut probes = 0u32;
        file_storage
            .expect_directory_exist()
            .withf(|path| path == media_root())
            .times(2)
            .returning(move |_| {
                probes = probes.saturating_add(1);
                let present = probes > 1;
                Box::pin(async move { Ok(present) })
            });
        let use_case = EnsureStorageDirectories::new(Arc::new(file_storage));

        // Act
        let response = use_case
            .execute(EnsureStorageDirectoriesCommand::new().waiting(30, true))
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
