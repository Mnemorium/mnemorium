use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::NumericID;
use crate::domain::model::ProductionType;

/// Command to list the music albums the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ListAlbumsCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Maximum number of albums to return, when set.
    limit: Option<usize>,
    /// Only return albums whose name matches this value, when set.
    name: Option<String>,
    /// Number of matching albums to skip before returning the page.
    offset: Option<usize>,
}

impl ListAlbumsCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the maximum number of albums to return, if any.
    #[must_use]
    pub(crate) fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-albums command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        name: Option<String>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Self {
        Self {
            caller_id,
            limit,
            name,
            offset,
        }
    }

    /// Return the number of matching albums to skip, if any.
    #[must_use]
    pub(crate) fn offset(&self) -> Option<usize> {
        self.offset
    }
}

/// One medium of a listed album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ListAlbumsMediumItem {
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Stored type of the medium (`CD`).
    medium_type: String,
    /// Number of recordings the medium sequences.
    track_count: usize,
}

impl ListAlbumsMediumItem {
    /// Return the album-scoped index of the medium.
    #[must_use]
    pub(crate) fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Return the stored type of the medium.
    #[must_use]
    pub(crate) fn medium_type(&self) -> &str {
        &self.medium_type
    }

    /// Create a new listed album medium.
    #[must_use]
    pub(crate) fn new(medium_index: u8, medium_type: String, track_count: usize) -> Self {
        Self {
            medium_index,
            medium_type,
            track_count,
        }
    }

    /// Return the number of recordings the medium sequences.
    #[must_use]
    pub(crate) fn track_count(&self) -> usize {
        self.track_count
    }
}

/// One album of a successful listing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ListAlbumsItem {
    /// Unique identifier of the album.
    album_id: NumericID,
    /// Identifier of the image holding the album's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Genres of the album, ordered by genre identifier.
    genres: Vec<String>,
    /// Mediums of the album, ordered by medium index.
    mediums: Vec<ListAlbumsMediumItem>,
    /// Identifier of the music group the album belongs to.
    music_group_id: NumericID,
    /// Name of the album.
    name: String,
    /// Production type of the album, when set.
    production_type: Option<ProductionType>,
    /// Date of the release of the album, when the provider reports one.
    release_date: Option<NaiveDate>,
    /// Number of tracks the album contains.
    track_count: usize,
}

impl ListAlbumsItem {
    /// Return the unique identifier of the album.
    #[must_use]
    pub(crate) fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the image holding the album's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the genres of the album, ordered by genre identifier.
    #[must_use]
    pub(crate) fn genres(&self) -> &[String] {
        &self.genres
    }

    /// Return the mediums of the album, ordered by medium index.
    #[must_use]
    pub(crate) fn mediums(&self) -> &[ListAlbumsMediumItem] {
        &self.mediums
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

    /// Create a new listed album.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the item's fields one-to-one"
    )]
    pub(crate) fn new(
        album_id: NumericID,
        music_group_id: NumericID,
        name: String,
        release_date: Option<NaiveDate>,
        production_type: Option<ProductionType>,
        genres: Vec<String>,
        track_count: usize,
        cover_image_id: Option<NumericID>,
        mediums: Vec<ListAlbumsMediumItem>,
    ) -> Self {
        Self {
            album_id,
            cover_image_id,
            genres,
            mediums,
            music_group_id,
            name,
            production_type,
            release_date,
            track_count,
        }
    }

    /// Return the production type, if any.
    #[must_use]
    pub(crate) fn production_type(&self) -> Option<ProductionType> {
        self.production_type
    }

    /// Return the date of the release, when the provider reports one.
    #[must_use]
    pub(crate) fn release_date(&self) -> Option<NaiveDate> {
        self.release_date
    }

    /// Return the number of tracks the album contains.
    #[must_use]
    pub(crate) fn track_count(&self) -> usize {
        self.track_count
    }
}

/// Response of a successful album listing: one page of albums.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ListAlbumsResponse {
    /// The albums of the returned page.
    albums: Vec<ListAlbumsItem>,
    /// Total number of albums the caller may see, before pagination.
    total: usize,
}

impl ListAlbumsResponse {
    /// Return the albums of the returned page.
    #[must_use]
    pub(crate) fn albums(&self) -> &[ListAlbumsItem] {
        &self.albums
    }

    /// Create a new list-albums response.
    #[must_use]
    pub(crate) fn new(albums: Vec<ListAlbumsItem>, total: usize) -> Self {
        Self { albums, total }
    }

    /// Return the total number of albums, before pagination.
    #[must_use]
    pub(crate) fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing music albums.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ListAlbumsError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the music albums the caller may see.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ListAlbumsUseCase: Send + Sync {
    /// List the music albums visible to the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn ListAlbumsUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: ListAlbumsCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListAlbumsResponse, ListAlbumsError>> + Send + 'future>>;
}
