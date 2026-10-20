use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Command to list the tracks of one medium of one music album.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListAlbumTracksCommand {
    /// Identifier of the album whose medium's tracks are listed.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Album-scoped index of the medium whose tracks are listed.
    medium_index: u8,
}

impl ListAlbumTracksCommand {
    /// Return the identifier of the album whose medium's tracks are listed.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the album-scoped index of the medium whose tracks are listed.
    #[must_use]
    pub fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Create a new list-album-tracks command.
    #[must_use]
    pub fn new(caller_id: NumericID, album_id: NumericID, medium_index: u8) -> Self {
        Self {
            album_id,
            caller_id,
            medium_index,
        }
    }
}

/// One track of an album medium, with the metadata of its backing file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListAlbumTracksItem {
    /// Identifier of the album the track belongs to.
    album_id: NumericID,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Date of the first release of the recording.
    first_release_date: NaiveDate,
    /// ISRC code of the recording, when set.
    isrc_code: Option<String>,
    /// Album-scoped index of the medium the track is placed in.
    medium_index: u8,
    /// Name of the recording, when it has one.
    name: Option<String>,
    /// Identifier of the shared recording the track places.
    recording_id: NumericID,
    /// Position of the track within its medium.
    track_index: u8,
}

impl ListAlbumTracksItem {
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

    /// Return the album-scoped index of the medium the track is placed in.
    #[must_use]
    pub fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Return the name of the recording, if it has one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new listed album track.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the fields the track representation needs"
    )]
    #[must_use]
    pub fn new(
        album_id: NumericID,
        medium_index: u8,
        track_index: u8,
        recording_id: NumericID,
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
            recording_id,
            track_index,
        }
    }

    /// Return the identifier of the shared recording the track places.
    #[must_use]
    pub fn recording_id(&self) -> NumericID {
        self.recording_id
    }

    /// Return the position of the track within its medium.
    #[must_use]
    pub fn track_index(&self) -> u8 {
        self.track_index
    }
}

/// Response of a successful album-track listing: one medium's tracks in order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListAlbumTracksResponse {
    /// The tracks of the medium, ordered by track index.
    tracks: Vec<ListAlbumTracksItem>,
}

impl ListAlbumTracksResponse {
    /// Create a new list-album-tracks response.
    #[must_use]
    pub fn new(tracks: Vec<ListAlbumTracksItem>) -> Self {
        Self { tracks }
    }

    /// Return the tracks of the medium.
    #[must_use]
    pub fn tracks(&self) -> &[ListAlbumTracksItem] {
        &self.tracks
    }
}

/// Error returned when listing the tracks of an album medium.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListAlbumTracksError {
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No medium of the album matches the requested index.
    #[error("a music medium with this index does not exist in this album")]
    NoSuchMedium,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the tracks of one medium of one music album.
#[cfg_attr(test, mockall::automock)]
pub trait ListAlbumTracksUseCase: Send + Sync {
    /// List the tracks of the requested medium.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn ListAlbumTracksUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: ListAlbumTracksCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListAlbumTracksResponse, ListAlbumTracksError>>
                + Send
                + 'future,
        >,
    >;
}
