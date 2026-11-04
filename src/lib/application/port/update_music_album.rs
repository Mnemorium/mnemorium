use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::field_update::FieldUpdate;
use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;
use crate::domain::model::music_album::ProductionType;

/// Command to update a music album partially.
///
/// An absent field keeps its stored value; a field the request clears drops an
/// optional value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UpdateMusicAlbumCommand {
    /// Identifier of the album to update.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Replacement cover image, when the request names one; `Clear` drops it.
    cover_image_id: FieldUpdate<NumericID>,
    /// Replacement genres, when the request names them.
    genres: Option<Vec<String>>,
    /// Replacement group, when the request moves the album.
    music_group_id: Option<NumericID>,
    /// Replacement name, when the request renames the album.
    name: Option<String>,
    /// Replacement production type, when the request names one; `Clear` drops
    /// it.
    production_type: FieldUpdate<ProductionType>,
    /// Replacement release date, when the request names one; `Clear` drops it.
    release_date: FieldUpdate<NaiveDate>,
}

impl UpdateMusicAlbumCommand {
    /// Return the identifier of the album to update.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the replacement cover image, if the request names one.
    #[must_use]
    pub fn cover_image_id(&self) -> FieldUpdate<NumericID> {
        self.cover_image_id
    }

    /// Return the replacement genres, if the request names them.
    #[must_use]
    pub fn genres(&self) -> Option<&[String]> {
        self.genres.as_deref()
    }

    /// Return the replacement group, if the request moves the album.
    #[must_use]
    pub fn music_group_id(&self) -> Option<NumericID> {
        self.music_group_id
    }

    /// Return the replacement name, if the request renames the album.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new update-album command.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the album's write fields one-to-one"
    )]
    pub fn new(
        caller_id: NumericID,
        album_id: NumericID,
        name: Option<String>,
        music_group_id: Option<NumericID>,
        release_date: FieldUpdate<NaiveDate>,
        production_type: FieldUpdate<ProductionType>,
        genres: Option<Vec<String>>,
        cover_image_id: FieldUpdate<NumericID>,
    ) -> Self {
        Self {
            album_id,
            caller_id,
            cover_image_id,
            genres,
            music_group_id,
            name,
            production_type,
            release_date,
        }
    }

    /// Return the replacement production type, if the request names one.
    #[must_use]
    pub fn production_type(&self) -> FieldUpdate<ProductionType> {
        self.production_type
    }

    /// Return the replacement release date, if the request names one.
    #[must_use]
    pub fn release_date(&self) -> FieldUpdate<NaiveDate> {
        self.release_date
    }
}

/// Response of a successful album update: the stored album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UpdateMusicAlbumResponse {
    /// The album that was stored.
    album: MusicAlbum,
}

impl UpdateMusicAlbumResponse {
    /// Return the album that was stored.
    #[must_use]
    pub fn album(&self) -> &MusicAlbum {
        &self.album
    }

    /// Create a new updated-album response.
    #[must_use]
    pub fn new(album: MusicAlbum) -> Self {
        Self { album }
    }
}

/// Error returned when updating a music album.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UpdateMusicAlbumError {
    /// The write duplicates another album's `(name, group)` identity.
    #[error("a music album with this name and group already exists")]
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
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
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

/// Use case for updating a music album.
#[cfg_attr(test, mockall::automock)]
pub trait UpdateMusicAlbumUseCase: Send + Sync {
    /// Update the requested album.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn UpdateMusicAlbumUseCase>`. `Box` keeps the future on the heap
    /// at a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: UpdateMusicAlbumCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateMusicAlbumResponse, UpdateMusicAlbumError>>
                + Send
                + 'future,
        >,
    >;
}
