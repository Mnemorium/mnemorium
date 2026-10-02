use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to complete an upload session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CompleteUploadCommand {
    /// Identifier of the upload session.
    upload_id: NumericID,
    /// Identifier of the user owning the upload.
    user_id: NumericID,
}

impl CompleteUploadCommand {
    /// Create a new complete-upload command.
    #[must_use]
    pub fn new(upload_id: NumericID, user_id: NumericID) -> Self {
        Self { upload_id, user_id }
    }

    /// Return the identifier of the upload session.
    #[must_use]
    pub fn upload_id(&self) -> NumericID {
        self.upload_id
    }

    /// Return the identifier of the owning user.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Response of a successfully completed upload session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CompleteUploadResponse {
    /// Unique identifier of the finished file.
    file_id: NumericID,
    /// Whether the upload session has been finished.
    is_finished: bool,
}

impl CompleteUploadResponse {
    /// Return the unique identifier of the finished file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return whether the upload session has been finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.is_finished
    }

    /// Create a new complete-upload response.
    #[must_use]
    pub fn new(file_id: NumericID, is_finished: bool) -> Self {
        Self {
            file_id,
            is_finished,
        }
    }
}

/// Error returned when completing an upload session.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CompleteUploadError {
    /// The upload session has expired.
    #[error("the upload session has expired")]
    Expired,
    /// The upload session is not complete.
    #[error("the upload session is not complete")]
    Incomplete,
    /// The recomputed integrity hash does not match the hash declared at init.
    #[error("the integrity hash does not match the declared one")]
    IntegrityMismatch,
    /// No upload session matches the requested identifier.
    #[error("no upload session matches this identifier")]
    NoSuchUpload,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for completing an upload session.
#[cfg_attr(test, mockall::automock)]
pub trait CompleteUploadUseCase: Send + Sync {
    /// Complete an upload session, promoting the staged file.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn CompleteUploadUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: CompleteUploadCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CompleteUploadResponse, CompleteUploadError>>
                + Send
                + 'future,
        >,
    >;
}
