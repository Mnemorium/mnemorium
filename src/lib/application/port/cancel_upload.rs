use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;

/// Command to cancel an upload session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CancelUploadCommand {
    /// Identifier of the upload session.
    upload_id: NumericID,
    /// Identifier of the user owning the upload.
    user_id: NumericID,
}

impl CancelUploadCommand {
    /// Create a new cancel-upload command.
    #[must_use]
    pub(crate) fn new(upload_id: NumericID, user_id: NumericID) -> Self {
        Self { upload_id, user_id }
    }

    /// Return the identifier of the upload session.
    #[must_use]
    pub(crate) fn upload_id(&self) -> NumericID {
        self.upload_id
    }

    /// Return the identifier of the owning user.
    #[must_use]
    pub(crate) fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Error returned when cancelling an upload session.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CancelUploadError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No upload session matches the requested identifier.
    #[error("no upload session matches this identifier")]
    NoSuchUpload,
    /// The session is finished or being verified, so it cannot be cancelled.
    #[error("the upload session cannot be cancelled")]
    NotCancellable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for cancelling an upload session.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait CancelUploadUseCase: Send + Sync {
    /// Cancel a pending upload session, deleting its staged content.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn CancelUploadUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: CancelUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), CancelUploadError>> + Send + 'future>>;
}
