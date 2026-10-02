use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Command to fetch the state of an upload session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetUploadCommand {
    /// Identifier of the upload session.
    upload_id: NumericID,
    /// Identifier of the user owning the upload.
    user_id: NumericID,
}

impl GetUploadCommand {
    /// Create a new get-upload command.
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

/// Response describing the state of an upload session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetUploadResponse {
    /// One character per chunk, `1` when received and `0` otherwise.
    bitmap: String,
    /// Date and time at which the upload session expires.
    expires_at: NaiveDateTime,
    /// Unique identifier of the finished file, when the upload is finished.
    file_id: Option<NumericID>,
    /// Whether the upload session has been finished.
    is_finished: bool,
    /// Total number of chunks the upload is split into.
    total_chunks: usize,
}

impl GetUploadResponse {
    /// Return the per-chunk received bitmap.
    #[must_use]
    pub fn bitmap(&self) -> &str {
        &self.bitmap
    }

    /// Return the date and time at which the upload session expires.
    #[must_use]
    pub fn expires_at(&self) -> NaiveDateTime {
        self.expires_at
    }

    /// Return the unique identifier of the finished file, if any.
    #[must_use]
    pub fn file_id(&self) -> Option<NumericID> {
        self.file_id
    }

    /// Return whether the upload session has been finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.is_finished
    }

    /// Create a new get-upload response.
    #[must_use]
    pub fn new(
        bitmap: String,
        total_chunks: usize,
        expires_at: NaiveDateTime,
        is_finished: bool,
        file_id: Option<NumericID>,
    ) -> Self {
        Self {
            bitmap,
            expires_at,
            file_id,
            is_finished,
            total_chunks,
        }
    }

    /// Return the total number of chunks the upload is split into.
    #[must_use]
    pub fn total_chunks(&self) -> usize {
        self.total_chunks
    }
}

/// Error returned when fetching the state of an upload session.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetUploadError {
    /// No upload session matches the requested identifier. An expired session
    /// is reported the same way, so a caller cannot tell the two apart.
    #[error("no upload session matches this identifier")]
    NoSuchUpload,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching the state of an upload session.
#[cfg_attr(test, mockall::automock)]
pub trait GetUploadUseCase: Send + Sync {
    /// Fetch the state of an upload session.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetUploadUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetUploadResponse, GetUploadError>> + Send + 'future>>;
}
