use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::{MusicAlbum, ProductionType};

/// Command to create a music album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CreateMusicAlbumCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the image holding the album's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Genres the album is associated with; empty asks for `UNKNOWN`.
    genres: Vec<String>,
    /// Identifier of the music group the album belongs to.
    music_group_id: NumericID,
    /// Name of the album.
    name: String,
    /// Production type of the album, when set.
    production_type: Option<ProductionType>,
    /// Date of the release, when known.
    release_date: Option<NaiveDate>,
}

impl CreateMusicAlbumCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the image holding the album's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the genres the album is associated with.
    #[must_use]
    pub(crate) fn genres(&self) -> &[String] {
        &self.genres
    }

    /// Return the identifier of the music group the album belongs to.
    #[must_use]
    pub(crate) fn music_group_id(&self) -> NumericID {
        self.music_group_id
    }

    /// Return the name of the album.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new create-album command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        name: String,
        music_group_id: NumericID,
        release_date: Option<NaiveDate>,
        production_type: Option<ProductionType>,
        genres: Vec<String>,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            caller_id,
            cover_image_id,
            genres,
            music_group_id,
            name,
            production_type,
            release_date,
        }
    }

    /// Return the production type, if any.
    #[must_use]
    pub(crate) fn production_type(&self) -> Option<ProductionType> {
        self.production_type
    }

    /// Return the date of the release, if known.
    #[must_use]
    pub(crate) fn release_date(&self) -> Option<NaiveDate> {
        self.release_date
    }
}

/// Response of a successful album creation: the stored album and whether the
/// write created it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CreateMusicAlbumResponse {
    /// The album that was stored.
    album: MusicAlbum,
    /// Set when the album was created, cleared when it already existed.
    created: bool,
}

impl CreateMusicAlbumResponse {
    /// Return the album that was stored.
    #[must_use]
    pub(crate) fn album(&self) -> &MusicAlbum {
        &self.album
    }

    /// Return whether the write created the album.
    #[must_use]
    pub(crate) fn created(&self) -> bool {
        self.created
    }

    /// Create a new created-album response.
    #[must_use]
    pub(crate) fn new(album: MusicAlbum, created: bool) -> Self {
        Self { album, created }
    }
}

/// Error returned when creating a music album.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CreateMusicAlbumError {
    /// The stored album refuses the write: the cover image belongs to a user.
    #[error("the music album cannot be created")]
    Conflict,
    /// The caller may not manage the library catalogue.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// A genre identifier is empty.
    #[error("a genre identifier must not be empty")]
    InvalidGenre,
    /// The name is empty or made only of whitespace.
    #[error("the name must not be empty")]
    InvalidName,
    /// The name is longer than the name policy allows.
    #[error("the name is too long")]
    NameTooLong,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No music group matches the requested identifier.
    #[error("a music group with this identifier does not exist")]
    NoSuchGroup,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for creating a music album.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait CreateMusicAlbumUseCase: Send + Sync {
    /// Create the album, returning the existing one when its identity is
    /// already stored.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn CreateMusicAlbumUseCase>`. `Box` keeps the future on the heap
    /// at a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: CreateMusicAlbumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreateMusicAlbumResponse, CreateMusicAlbumError>>
                + Send
                + 'future,
        >,
    >;
}
