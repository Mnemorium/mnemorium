use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::integrity_hash::IntegrityHash;
use crate::domain::model::integrity_hash::SHA256_HEX_LENGTH;

/// Command to write one chunk of an upload session.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteUploadChunkCommand {
    /// Raw bytes of the chunk.
    chunk: Vec<u8>,
    /// Zero-based number of the chunk.
    chunk_number: u64,
    /// Integrity hash of the chunk, supplied by the client.
    content_digest: IntegrityHash<SHA256_HEX_LENGTH>,
    /// Start offset of the chunk declared by the client, from `Content-Range`.
    start: u64,
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

    /// Return the integrity hash of the chunk supplied by the client.
    #[must_use]
    pub fn content_digest(&self) -> &IntegrityHash<SHA256_HEX_LENGTH> {
        &self.content_digest
    }

    /// Create a new write-upload-chunk command.
    #[must_use]
    pub fn new(
        upload_id: NumericID,
        chunk_number: u64,
        start: u64,
        chunk: Vec<u8>,
        content_digest: IntegrityHash<SHA256_HEX_LENGTH>,
        user_id: NumericID,
    ) -> Self {
        Self {
            chunk,
            chunk_number,
            content_digest,
            start,
            upload_id,
            user_id,
        }
    }

    /// Return the start offset of the chunk declared by the client.
    #[must_use]
    pub fn start(&self) -> u64 {
        self.start
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteUploadChunkResponse {
    /// One character per chunk, `1` when received and `0` otherwise.
    bitmap: String,
    /// Date and time at which the upload session expires.
    expires_at: NaiveDateTime,
    /// Unique identifier of the caller's file for the session digest, when one
    /// exists.
    file_id: Option<NumericID>,
    /// Whether the upload session has been finished.
    is_finished: bool,
    /// Total number of chunks the upload is split into.
    total_chunks: usize,
}

impl WriteUploadChunkResponse {
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

    /// Return the unique identifier of the caller's file, if any.
    #[must_use]
    pub fn file_id(&self) -> Option<NumericID> {
        self.file_id
    }

    /// Return whether the upload session has been finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.is_finished
    }

    /// Create a new write-upload-chunk response.
    #[must_use]
    pub fn new(
        bitmap: String,
        expires_at: NaiveDateTime,
        file_id: Option<NumericID>,
        is_finished: bool,
        total_chunks: usize,
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
    /// The declared `Content-Range` start does not match the chunk's position.
    #[error("the chunk range does not match the chunk number")]
    InvalidChunkRange,
    /// The declared `Content-Digest` digest does not match the chunk.
    #[error("the content digest does not match the chunk")]
    InvalidContentDigest,
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
