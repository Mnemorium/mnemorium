use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Command to fetch one track of a music album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "`album_id`, `caller_id` and `track_id` are the identifier names the API and SQL sections mandate"
)]
#[non_exhaustive]
pub struct GetAlbumTrackCommand {
    /// Identifier of the album the track belongs to.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the track to fetch.
    track_id: NumericID,
}

impl GetAlbumTrackCommand {
    /// Return the identifier of the album the track belongs to.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-album-track command.
    #[must_use]
    pub fn new(caller_id: NumericID, album_id: NumericID, track_id: NumericID) -> Self {
        Self {
            album_id,
            caller_id,
            track_id,
        }
    }

    /// Return the identifier of the track to fetch.
    #[must_use]
    pub fn track_id(&self) -> NumericID {
        self.track_id
    }
}

/// Response of a successful track lookup: one track of an album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAlbumTrackResponse {
    /// Identifier of the album the track belongs to.
    album_id: NumericID,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Date of the first release of the recording.
    first_release_date: NaiveDate,
    /// ISRC code of the recording, when set.
    isrc_code: Option<String>,
    /// Index of the medium the track is positioned on.
    medium_index: u32,
    /// Name of the recording, when it has one.
    name: Option<String>,
    /// Unique identifier of the track.
    track_id: NumericID,
    /// Position of the track within its medium.
    track_index: i64,
}

impl GetAlbumTrackResponse {
    /// Return the identifier of the album the track belongs to.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the date of the first release of the recording.
    #[must_use]
    pub fn first_release_date(&self) -> NaiveDate {
        self.first_release_date
    }

    /// Return the ISRC code of the recording, if any.
    #[must_use]
    pub fn isrc_code(&self) -> Option<&str> {
        self.isrc_code.as_deref()
    }

    /// Return the index of the medium the track is positioned on.
    #[must_use]
    pub fn medium_index(&self) -> u32 {
        self.medium_index
    }

    /// Return the name of the recording, if it has one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new fetch-album-track response.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the columns of the track read model"
    )]
    #[must_use]
    pub fn new(
        track_id: NumericID,
        album_id: NumericID,
        medium_index: u32,
        track_index: i64,
        file_id: NumericID,
        name: Option<String>,
        isrc_code: Option<String>,
        first_release_date: NaiveDate,
    ) -> Self {
        Self {
            album_id,
            file_id,
            first_release_date,
            isrc_code,
            medium_index,
            name,
            track_id,
            track_index,
        }
    }

    /// Return the unique identifier of the track.
    #[must_use]
    pub fn track_id(&self) -> NumericID {
        self.track_id
    }

    /// Return the position of the track within its medium.
    #[must_use]
    pub fn track_index(&self) -> i64 {
        self.track_index
    }
}

/// Error returned when fetching one track of an album.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetAlbumTrackError {
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No track of the album matches the requested identifier.
    #[error("a track with this identifier does not exist in this album")]
    NoSuchTrack,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching one track of a music album.
#[cfg_attr(test, mockall::automock)]
pub trait GetAlbumTrackUseCase: Send + Sync {
    /// Fetch the requested track, scoped to its album.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetAlbumTrackUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetAlbumTrackCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAlbumTrackResponse, GetAlbumTrackError>> + Send + 'future,
        >,
    >;
}
