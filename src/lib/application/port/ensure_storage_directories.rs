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

/// Outcome of ensuring the storage folders exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EnsureStorageDirectoriesResponse {
    /// Whether the library root exists. It is never created: absence usually
    /// means the volume did not mount, which the server reports and survives.
    media_root_present: bool,
}

impl EnsureStorageDirectoriesResponse {
    /// Return whether the library root exists.
    #[must_use]
    pub fn media_root_present(&self) -> bool {
        self.media_root_present
    }

    /// Create a new response.
    #[must_use]
    pub fn new(media_root_present: bool) -> Self {
        Self { media_root_present }
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
    /// Ensure the upload staging folder exists, creating it when missing, and
    /// report whether the library root is present.
    ///
    /// The library root is never created.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned, so the method is object-safe and the use case can be stored as
    /// `Arc<dyn EnsureStorageDirectoriesUseCase>`.
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
    >;
}
