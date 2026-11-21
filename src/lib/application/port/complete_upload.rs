use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;

/// Command to complete an upload session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CompleteUploadCommand {
    /// Identifier of the upload session.
    upload_id: NumericID,
    /// Identifier of the user owning the upload.
    user_id: NumericID,
}

impl CompleteUploadCommand {
    /// Create a new complete-upload command.
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

/// Response of a successfully completed upload session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CompleteUploadResponse {
    /// Unique identifier of the finished file.
    file_id: NumericID,
    /// Whether the upload session has been finished.
    is_finished: bool,
}

impl CompleteUploadResponse {
    /// Return the unique identifier of the finished file.
    #[must_use]
    pub(crate) fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return whether the upload session has been finished.
    #[must_use]
    pub(crate) fn is_finished(&self) -> bool {
        self.is_finished
    }

    /// Create a new complete-upload response.
    #[must_use]
    pub(crate) fn new(file_id: NumericID, is_finished: bool) -> Self {
        Self {
            file_id,
            is_finished,
        }
    }
}

/// Error returned when completing an upload session.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CompleteUploadError {
    /// Another completion of the upload session holds, or has released, its
    /// verification seal.
    #[error("another completion of the upload session is in progress")]
    CompletionInProgress,
    /// The upload session has expired.
    #[error("the upload session has expired")]
    Expired,
    /// The upload session is not complete.
    #[error("the upload session is not complete")]
    Incomplete,
    /// The recomputed integrity hash does not match the hash declared at init.
    #[error("the integrity hash does not match the declared one")]
    IntegrityMismatch,
    /// The library root is absent, so the server cannot store the file.
    #[error("the media library is unavailable")]
    LibraryUnavailable,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No upload session matches the requested identifier.
    #[error("no upload session matches this identifier")]
    NoSuchUpload,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The completed upload is not a supported media file.
    #[error("the completed upload is not a supported media file")]
    UnsupportedMedia,
}

/// Use case for completing an upload session.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait CompleteUploadUseCase: Send + Sync {
    /// Complete an upload session, promoting the staged file.
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
