use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::ProductionType;

/// Command to fetch one music album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAlbumCommand {
    /// Identifier of the album to fetch.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
}

impl GetAlbumCommand {
    /// Return the identifier of the album to fetch.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-album command.
    #[must_use]
    pub fn new(caller_id: NumericID, album_id: NumericID) -> Self {
        Self {
            album_id,
            caller_id,
        }
    }
}

/// One medium of a fetched album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAlbumMediumItem {
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Stored type of the medium (`CD`).
    medium_type: String,
    /// Number of recordings the medium sequences.
    track_count: usize,
}

impl GetAlbumMediumItem {
    /// Return the album-scoped index of the medium.
    #[must_use]
    pub fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Return the stored type of the medium.
    #[must_use]
    pub fn medium_type(&self) -> &str {
        &self.medium_type
    }

    /// Create a new fetched album medium.
    #[must_use]
    pub fn new(medium_index: u8, medium_type: String, track_count: usize) -> Self {
        Self {
            medium_index,
            medium_type,
            track_count,
        }
    }

    /// Return the number of recordings the medium sequences.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.track_count
    }
}

/// Response of a successful album lookup: one album with its mediums.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAlbumResponse {
    /// Unique identifier of the album.
    album_id: NumericID,
    /// Genres of the album, ordered by genre identifier.
    genres: Vec<String>,
    /// Mediums of the album, ordered by medium index.
    mediums: Vec<GetAlbumMediumItem>,
    /// Name of the album.
    name: String,
    /// Production type of the album, when set.
    production_type: Option<ProductionType>,
    /// Date of the release of the album.
    release_date: NaiveDate,
    /// Number of tracks the album contains.
    track_count: usize,
}

impl GetAlbumResponse {
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

    /// Return the mediums of the album, ordered by medium index.
    #[must_use]
    pub fn mediums(&self) -> &[GetAlbumMediumItem] {
        &self.mediums
    }

    /// Return the name of the album.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-album response.
    #[must_use]
    pub fn new(
        album_id: NumericID,
        name: String,
        release_date: NaiveDate,
        production_type: Option<ProductionType>,
        genres: Vec<String>,
        track_count: usize,
        mediums: Vec<GetAlbumMediumItem>,
    ) -> Self {
        Self {
            album_id,
            genres,
            mediums,
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

/// Error returned when fetching one music album.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetAlbumError {
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

/// Use case for fetching one music album.
#[cfg_attr(test, mockall::automock)]
pub trait GetAlbumUseCase: Send + Sync {
    /// Fetch the requested album with its mediums.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetAlbumUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetAlbumCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetAlbumResponse, GetAlbumError>> + Send + 'future>>;
}
