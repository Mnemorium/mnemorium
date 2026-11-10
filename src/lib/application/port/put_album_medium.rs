use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::model::MusicAlbum;

/// Command to create or retype one medium of a music album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PutAlbumMediumCommand {
    /// Identifier of the album that holds the medium.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Requested medium type (`CD` or `DIGITAL`).
    medium_type: String,
}

impl PutAlbumMediumCommand {
    /// Return the identifier of the album that holds the medium.
    #[must_use]
    pub(crate) fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the album-scoped index of the medium.
    #[must_use]
    pub(crate) fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Return the requested medium type.
    #[must_use]
    pub(crate) fn medium_type(&self) -> &str {
        &self.medium_type
    }

    /// Create a new put-medium command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        album_id: NumericID,
        medium_index: u8,
        medium_type: String,
    ) -> Self {
        Self {
            album_id,
            caller_id,
            medium_index,
            medium_type,
        }
    }
}

/// Response of a successful medium write: the stored album and the medium it
/// now holds at the requested index.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PutAlbumMediumResponse {
    /// The stored album with its mediums.
    album: MusicAlbum,
    /// Set when the write created the medium, cleared when it retyped one.
    created: bool,
    /// Album-scoped index of the written medium.
    medium_index: u8,
}

impl PutAlbumMediumResponse {
    /// Return the stored album.
    #[must_use]
    pub(crate) fn album(&self) -> &MusicAlbum {
        &self.album
    }

    /// Return whether the write created the medium.
    #[must_use]
    pub(crate) fn created(&self) -> bool {
        self.created
    }

    /// Return the album-scoped index of the written medium.
    #[must_use]
    pub(crate) fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Create a new written-medium response.
    #[must_use]
    pub(crate) fn new(album: MusicAlbum, medium_index: u8, created: bool) -> Self {
        Self {
            album,
            created,
            medium_index,
        }
    }
}

/// Error returned when writing a music medium.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum PutAlbumMediumError {
    /// The write conflicts with the stored album's protected state.
    #[error("the music medium cannot be written")]
    Conflict,
    /// The default album is immutable.
    #[error("the default music album cannot be written")]
    DefaultAlbum,
    /// The caller may not manage the library catalogue.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The requested type is not `CD` or `DIGITAL`.
    #[error("the medium type must be CD or DIGITAL")]
    InvalidMediumType,
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for creating or retyping a music medium.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait PutAlbumMediumUseCase: Send + Sync {
    /// Create or retype the medium at the requested index.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn PutAlbumMediumUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: PutAlbumMediumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutAlbumMediumResponse, PutAlbumMediumError>>
                + Send
                + 'future,
        >,
    >;
}
