use std::future::Future;
use std::pin::Pin;

/// Command to reap expired, stale and orphaned upload sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub(crate) struct ReapUploadsCommand;

impl ReapUploadsCommand {
    /// Create a new reap-uploads command.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self
    }
}

/// Response of a reaping run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub(crate) struct ReapUploadsResponse {
    /// Number of expired sessions removed.
    expired: u64,
    /// Number of orphaned staging files removed.
    orphaned: u64,
    /// Number of stale verification seals released.
    unsealed: u64,
}

impl ReapUploadsResponse {
    /// Return the number of expired sessions removed.
    #[must_use]
    pub(crate) fn expired(&self) -> u64 {
        self.expired
    }

    /// Create a new reap-uploads response.
    #[must_use]
    pub(crate) fn new(expired: u64, orphaned: u64, unsealed: u64) -> Self {
        Self {
            expired,
            orphaned,
            unsealed,
        }
    }

    /// Return the number of orphaned staging files removed.
    #[must_use]
    pub(crate) fn orphaned(&self) -> u64 {
        self.orphaned
    }

    /// Return the number of stale verification seals released.
    #[must_use]
    pub(crate) fn unsealed(&self) -> u64 {
        self.unsealed
    }
}

/// Error returned when reaping upload sessions.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ReapUploadsError {
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for reaping expired, stale and orphaned upload sessions.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ReapUploadsUseCase: Send + Sync {
    /// Run one bounded reaping pass.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn ReapUploadsUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: ReapUploadsCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ReapUploadsResponse, ReapUploadsError>> + Send + 'future>>;
}
