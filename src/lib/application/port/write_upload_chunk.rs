use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to write one chunk of an upload session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteUploadChunkCommand {
    /// Raw bytes of the chunk.
    chunk: Vec<u8>,
    /// Zero-based number of the chunk.
    chunk_number: u64,
    /// MD5 digest of the chunk, supplied by the client.
    content_md5: String,
    /// Identifier of the upload session.
    upload_id: NumericID,
    /// Identifier of the user owning the upload.
    user_id: NumericID,
}

impl WriteUploadChunkCommand {
    /// Return the raw bytes of the chunk.
    #[must_use]
    pub fn chunk(&self) -> &[u8] {
        &self.chunk
    }

    /// Return the zero-based number of the chunk.
    #[must_use]
    pub fn chunk_number(&self) -> u64 {
        self.chunk_number
    }

    /// Return the MD5 digest of the chunk supplied by the client.
    #[must_use]
    pub fn content_md5(&self) -> &str {
        &self.content_md5
    }

    /// Create a new write-upload-chunk command.
    #[must_use]
    pub fn new(
        upload_id: NumericID,
        chunk_number: u64,
        chunk: Vec<u8>,
        content_md5: String,
        user_id: NumericID,
    ) -> Self {
        Self {
            chunk,
            chunk_number,
            content_md5,
            upload_id,
            user_id,
        }
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

/// Response of a successfully written chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteUploadChunkResponse {
    /// Whether the chunk has been received.
    received: bool,
}

impl WriteUploadChunkResponse {
    /// Create a new write-upload-chunk response.
    #[must_use]
    pub fn new(received: bool) -> Self {
        Self { received }
    }

    /// Return whether the chunk has been received.
    #[must_use]
    pub fn received(&self) -> bool {
        self.received
    }
}

/// Error returned when writing one chunk of an upload session.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteUploadChunkError {
    /// The upload session has already been finished.
    #[error("the upload session has already been finished")]
    AlreadyFinished,
    /// The upload session has expired.
    #[error("the upload session has expired")]
    Expired,
    /// The chunk length does not match the expected size.
    #[error("the chunk is invalid")]
    InvalidChunk,
    /// The chunk number is outside the upload's range.
    #[error("the chunk number is out of range")]
    InvalidChunkNumber,
    /// The declared `Content-MD5` digest is malformed or does not match the chunk.
    #[error("the content md5 digest is invalid")]
    InvalidMd5,
    /// No upload session matches the requested identifier.
    #[error("no upload session matches this identifier")]
    NoSuchUpload,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for writing one chunk of an upload session.
#[cfg_attr(test, mockall::automock)]
pub trait WriteUploadChunkUseCase: Send + Sync {
    /// Write one chunk of an upload session.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn WriteUploadChunkUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: WriteUploadChunkCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<WriteUploadChunkResponse, WriteUploadChunkError>>
                + Send
                + 'future,
        >,
    >;
}
