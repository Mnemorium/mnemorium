use std::future::Future;
use std::pin::Pin;

/// Command for ensuring the storage folders exist.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct EnsureStorageDirectoriesCommand;

impl EnsureStorageDirectoriesCommand {
    /// Create a new command.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Error returned when ensuring the storage folders.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EnsureStorageDirectoriesError {
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for ensuring the storage folders exist.
#[cfg_attr(test, mockall::automock)]
pub trait EnsureStorageDirectoriesUseCase: Send + Sync {
    /// Ensure the upload and file folders exist, creating them when missing.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned, so the method is object-safe and the use case can be stored as
    /// `Arc<dyn EnsureStorageDirectoriesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: EnsureStorageDirectoriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), EnsureStorageDirectoriesError>> + Send + 'future>>;
}
