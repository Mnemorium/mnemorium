use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::ProductionType;

/// Command to list the music albums the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListAlbumsCommand {
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
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the maximum number of albums to return, if any.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-albums command.
    #[must_use]
    pub fn new(
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
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }
}

/// One album of a successful listing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListAlbumsItem {
    /// Unique identifier of the album.
    album_id: NumericID,
    /// Genres of the album, ordered by genre identifier.
    genres: Vec<String>,
    /// Name of the album.
    name: String,
    /// Production type of the album, when set.
    production_type: Option<ProductionType>,
    /// Date of the release of the album.
    release_date: NaiveDate,
    /// Number of tracks the album contains.
    track_count: usize,
}

impl ListAlbumsItem {
    /// Return the unique identifier of the album.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the genres of the album, ordered by genre identifier.
    #[must_use]
    pub fn genres(&self) -> &[String] {
        &self.genres
    }

    /// Return the name of the album.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new listed album.
    #[must_use]
    pub fn new(
        album_id: NumericID,
        name: String,
        release_date: NaiveDate,
        production_type: Option<ProductionType>,
        genres: Vec<String>,
        track_count: usize,
    ) -> Self {
        Self {
            album_id,
            genres,
            name,
            production_type,
            release_date,
            track_count,
        }
    }

    /// Return the production type, if any.
    #[must_use]
    pub fn production_type(&self) -> Option<ProductionType> {
        self.production_type
    }

    /// Return the date of the release.
    #[must_use]
    pub fn release_date(&self) -> NaiveDate {
        self.release_date
    }

    /// Return the number of tracks the album contains.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.track_count
    }
}

/// Response of a successful album listing: one page of albums.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListAlbumsResponse {
    /// The albums of the returned page.
    albums: Vec<ListAlbumsItem>,
    /// Total number of albums the caller may see, before pagination.
    total: usize,
}

impl ListAlbumsResponse {
    /// Return the albums of the returned page.
    #[must_use]
    pub fn albums(&self) -> &[ListAlbumsItem] {
        &self.albums
    }

    /// Create a new list-albums response.
    #[must_use]
    pub fn new(albums: Vec<ListAlbumsItem>, total: usize) -> Self {
        Self { albums, total }
    }

    /// Return the total number of albums, before pagination.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing music albums.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListAlbumsError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the music albums the caller may see.
#[cfg_attr(test, mockall::automock)]
pub trait ListAlbumsUseCase: Send + Sync {
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
