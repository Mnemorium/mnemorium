use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;
use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::file::Origin;

/// Command to fetch one stored file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetFileCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the file to fetch.
    file_id: NumericID,
}

impl GetFileCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the file to fetch.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Create a new fetch-file command.
    #[must_use]
    pub fn new(caller_id: NumericID, file_id: NumericID) -> Self {
        Self { caller_id, file_id }
    }
}

/// Response of a successful file lookup: one file and the media it backs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetFileResponse {
    /// Identifier of the audio row backing the file, when one exists.
    audio_id: Option<NumericID>,
    /// Identifier of the file.
    id: NumericID,
    /// Identifier of the image row backing the file, when one exists.
    image_id: Option<NumericID>,
    /// Whether the file is marked public.
    is_public: bool,
    /// Identifier of the mime type of the file.
    mime_type_id: String,
    /// Instant at which the content was last modified.
    modified_at: NaiveDateTime,
    /// Origin of the file's bytes.
    origin: Origin,
    /// Identifier of the user who owns the file.
    owner_id: NumericID,
    /// Path of the file relative to the library root.
    path: String,
    /// Size of the file content, in bytes.
    size_bytes: i64,
    /// Date at which the file was uploaded.
    uploaded_at: NaiveDate,
    /// Identifier of the video row backing the file, when one exists.
    video_id: Option<NumericID>,
}

impl GetFileResponse {
    /// Return the identifier of the audio backing the file, if any.
    #[must_use]
    pub fn audio_id(&self) -> Option<NumericID> {
        self.audio_id
    }

    /// Return the identifier of the file.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the identifier of the image backing the file, if any.
    #[must_use]
    pub fn image_id(&self) -> Option<NumericID> {
        self.image_id
    }

    /// Return whether the file is marked public.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the identifier of the mime type of the file.
    #[must_use]
    pub fn mime_type_id(&self) -> &str {
        &self.mime_type_id
    }

    /// Return the instant at which the content was last modified.
    #[must_use]
    pub fn modified_at(&self) -> NaiveDateTime {
        self.modified_at
    }

    /// Create a new fetch-file response.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the response's fields one-to-one"
    )]
    pub fn new(
        id: NumericID,
        path: String,
        mime_type_id: String,
        size_bytes: i64,
        uploaded_at: NaiveDate,
        modified_at: NaiveDateTime,
        origin: Origin,
        is_public: bool,
        owner_id: NumericID,
        image_id: Option<NumericID>,
        audio_id: Option<NumericID>,
        video_id: Option<NumericID>,
    ) -> Self {
        Self {
            audio_id,
            id,
            image_id,
            is_public,
            mime_type_id,
            modified_at,
            origin,
            owner_id,
            path,
            size_bytes,
            uploaded_at,
            video_id,
        }
    }

    /// Return the origin of the file's bytes.
    #[must_use]
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// Return the identifier of the user owning the file.
    #[must_use]
    pub fn owner_id(&self) -> NumericID {
        self.owner_id
    }

    /// Return the path of the file relative to the library root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Return the size of the content, in bytes.
    #[must_use]
    pub fn size_bytes(&self) -> i64 {
        self.size_bytes
    }

    /// Return the date at which the file was uploaded.
    #[must_use]
    pub fn uploaded_at(&self) -> NaiveDate {
        self.uploaded_at
    }

    /// Return the identifier of the video backing the file, if any.
    #[must_use]
    pub fn video_id(&self) -> Option<NumericID> {
        self.video_id
    }
}

/// Error returned when fetching a stored file fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetFileError {
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// The file does not exist, or the caller may not read it.
    ///
    /// Both outcomes are reported the same way so a caller cannot probe for
    /// files it may not read.
    #[error("the file does not exist")]
    NoSuchFile,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case fetching one stored file.
#[cfg_attr(test, mockall::automock)]
pub trait GetFileUseCase: Send + Sync {
    /// Fetch the requested file.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetFileUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetFileCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetFileResponse, GetFileError>> + Send + 'future>>;
}
