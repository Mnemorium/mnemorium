use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;

/// Command to begin a chunked upload session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BeginUploadCommand {
    /// Declared media type of the file content.
    content_type: String,
    /// Original name of the file being uploaded.
    file_name: String,
    /// Total size of the file being uploaded, in bytes.
    file_size: u64,
    /// Integrity hash of the complete file, supplied by the client.
    integrity_hash: IntegrityHash<SHA256_HEX_LENGTH>,
    /// Identifier of the user owning the upload.
    user_id: NumericID,
}

impl BeginUploadCommand {
    /// Return the declared media type.
    #[must_use]
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// Return the original file name.
    #[must_use]
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// Return the total file size, in bytes.
    #[must_use]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Return the integrity hash supplied by the client.
    #[must_use]
    pub fn integrity_hash(&self) -> &IntegrityHash<SHA256_HEX_LENGTH> {
        &self.integrity_hash
    }

    /// Create a new begin-upload command.
    #[must_use]
    pub fn new(
        file_name: String,
        file_size: u64,
        content_type: String,
        integrity_hash: IntegrityHash<SHA256_HEX_LENGTH>,
        user_id: NumericID,
    ) -> Self {
        Self {
            content_type,
            file_name,
            file_size,
            integrity_hash,
            user_id,
        }
    }

    /// Return the identifier of the owning user.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Response of a successfully begun upload session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BeginUploadResponse {
    /// Fixed size of every chunk except the last, in bytes.
    chunk_size: u64,
    /// Date and time at which the upload session expires.
    expires_at: NaiveDateTime,
    /// Unique identifier of the upload session.
    upload_id: NumericID,
}

impl BeginUploadResponse {
    /// Return the fixed chunk size, in bytes.
    #[must_use]
    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }

    /// Return the date and time at which the upload session expires.
    #[must_use]
    pub fn expires_at(&self) -> NaiveDateTime {
        self.expires_at
    }

    /// Create a new begin-upload response.
    #[must_use]
    pub fn new(upload_id: NumericID, chunk_size: u64, expires_at: NaiveDateTime) -> Self {
        Self {
            chunk_size,
            expires_at,
            upload_id,
        }
    }

    /// Return the unique identifier of the upload session.
    #[must_use]
    pub fn upload_id(&self) -> NumericID {
        self.upload_id
    }
}

/// Error returned when beginning an upload session.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BeginUploadError {
    /// The file size exceeds the maximum allowed size.
    #[error("the file size exceeds the maximum allowed size")]
    FileTooLarge,
    /// The file name is unsafe.
    #[error("the file name is invalid")]
    InvalidFileName,
    /// The file size is zero.
    #[error("the file size must be greater than zero")]
    InvalidFileSize,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The declared content type is not a supported media type.
    #[error("the declared content type is not a supported media type")]
    UnsupportedMediaType,
}

/// Use case for beginning an upload session.
#[cfg_attr(test, mockall::automock)]
pub trait BeginUploadUseCase: Send + Sync {
    /// Begin a new chunked upload session.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn BeginUploadUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: BeginUploadCommand,
    ) -> Pin<Box<dyn Future<Output = Result<BeginUploadResponse, BeginUploadError>> + Send + 'future>>;
}
