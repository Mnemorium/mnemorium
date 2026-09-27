use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to check whether a file with a given MD5 digest already exists.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CheckUploadCommand {
    /// MD5 digest of the complete file.
    md5: String,
    /// Identifier of the user owning the file.
    user_id: NumericID,
}

impl CheckUploadCommand {
    /// Return the MD5 digest of the complete file.
    #[must_use]
    pub fn md5(&self) -> &str {
        &self.md5
    }

    /// Create a new check-upload command.
    #[must_use]
    pub fn new(md5: String, user_id: NumericID) -> Self {
        Self { md5, user_id }
    }

    /// Return the identifier of the owning user.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Response of a check-upload lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CheckUploadResponse {
    /// Unique identifier of the matching file, when one exists.
    file_id: Option<NumericID>,
}

impl CheckUploadResponse {
    /// Return the unique identifier of the matching file, if any.
    #[must_use]
    pub fn file_id(&self) -> Option<NumericID> {
        self.file_id
    }

    /// Create a new check-upload response.
    #[must_use]
    pub fn new(file_id: Option<NumericID>) -> Self {
        Self { file_id }
    }
}

/// Error returned when checking for an existing file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CheckUploadError {
    /// The MD5 digest is not a 32-character hexadecimal string.
    #[error("the md5 digest is invalid")]
    InvalidMd5,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for checking whether a file with a given MD5 digest already exists.
#[cfg_attr(test, mockall::automock)]
pub trait CheckUploadUseCase: Send + Sync {
    /// Check whether the caller already owns a file with the given MD5 digest.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn CheckUploadUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: CheckUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<CheckUploadResponse, CheckUploadError>> + Send + 'future>>;
}
