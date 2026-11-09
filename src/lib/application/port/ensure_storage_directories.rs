use std::future::Future;
use std::pin::Pin;

/// Command for ensuring the storage folders exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub(crate) struct EnsureStorageDirectoriesCommand {
    /// Whether startup must wait for the library root before serving.
    required: bool,
    /// Seconds startup waits for the library root to appear.
    wait_seconds: u64,
}

impl EnsureStorageDirectoriesCommand {
    /// Create a new command deciding immediately.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Return whether startup must wait for the library root.
    #[must_use]
    pub(crate) fn required(&self) -> bool {
        self.required
    }

    /// Return the seconds startup waits for the library root to appear.
    #[must_use]
    pub(crate) fn wait_seconds(&self) -> u64 {
        self.wait_seconds
    }

    /// Wait up to `wait_seconds` for the root when `required` is set.
    ///
    /// The wait delays startup only when the root is required; otherwise the
    /// decision is immediate so readiness never waits for a mount.
    #[must_use]
    pub(crate) fn waiting(mut self, wait_seconds: u64, required: bool) -> Self {
        self.required = required;
        self.wait_seconds = wait_seconds;
        self
    }
}

/// Outcome of ensuring the storage folders exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct EnsureStorageDirectoriesResponse {
    /// Whether the library root exists. It is never created: absence usually
    /// means the volume did not mount, which the server reports and survives.
    media_root_present: bool,
}

impl EnsureStorageDirectoriesResponse {
    /// Return whether the library root exists.
    #[must_use]
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "STY-RUST-019 fixes the getter receiver as `&self`"
    )]
    pub(crate) fn media_root_present(&self) -> bool {
        self.media_root_present
    }

    /// Create a new response.
    #[must_use]
    pub(crate) fn new(media_root_present: bool) -> Self {
        Self { media_root_present }
    }
}

/// Error returned when ensuring the storage folders.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum EnsureStorageDirectoriesError {
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for ensuring the storage folders exist.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait EnsureStorageDirectoriesUseCase: Send + Sync {
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
