use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;

/// Command to place or move a recording at a medium track address.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PutAlbumTrackCommand {
    /// Identifier of the album that holds the medium.
    album_id: NumericID,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Identifier of the recording to place.
    recording_id: NumericID,
    /// Index of the track within the medium.
    track_index: u8,
}

impl PutAlbumTrackCommand {
    /// Return the identifier of the album that holds the medium.
    #[must_use]
    pub fn album_id(&self) -> NumericID {
        self.album_id
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the album-scoped index of the medium.
    #[must_use]
    pub fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Create a new put-track command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        album_id: NumericID,
        medium_index: u8,
        track_index: u8,
        recording_id: NumericID,
    ) -> Self {
        Self {
            album_id,
            caller_id,
            medium_index,
            recording_id,
            track_index,
        }
    }

    /// Return the identifier of the recording to place.
    #[must_use]
    pub fn recording_id(&self) -> NumericID {
        self.recording_id
    }

    /// Return the index of the track within the medium.
    #[must_use]
    pub fn track_index(&self) -> u8 {
        self.track_index
    }
}

/// Response of a successful placement: the stored album and the destination
/// address.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PutAlbumTrackResponse {
    /// The stored album with its mediums.
    album: MusicAlbum,
    /// Set when the placement created a new medium placement.
    created: bool,
    /// Album-scoped index of the medium.
    medium_index: u8,
    /// Index of the track within the medium.
    track_index: u8,
}

impl PutAlbumTrackResponse {
    /// Return the stored album.
    #[must_use]
    pub fn album(&self) -> &MusicAlbum {
        &self.album
    }

    /// Return whether the placement created a new medium placement.
    #[must_use]
    pub fn created(&self) -> bool {
        self.created
    }

    /// Return the album-scoped index of the medium.
    #[must_use]
    pub fn medium_index(&self) -> u8 {
        self.medium_index
    }

    /// Create a new placement response.
    #[must_use]
    pub fn new(album: MusicAlbum, medium_index: u8, track_index: u8, created: bool) -> Self {
        Self {
            album,
            created,
            medium_index,
            track_index,
        }
    }

    /// Return the index of the track within the medium.
    #[must_use]
    pub fn track_index(&self) -> u8 {
        self.track_index
    }
}

/// Error returned when placing a recording.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PutAlbumTrackError {
    /// The destination track already holds another recording.
    #[error("the destination track already holds another recording")]
    Conflict,
    /// The default album is immutable.
    #[error("the default music album cannot be written")]
    DefaultAlbum,
    /// The caller may not manage the library catalogue.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// No album matches the requested identifier.
    #[error("a music album with this identifier does not exist")]
    NoSuchAlbum,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No medium matches the requested album and index.
    #[error("a music medium with this index does not exist")]
    NoSuchMedium,
    /// No recording matches the requested identifier.
    #[error("a music recording with this identifier does not exist")]
    NoSuchRecording,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for placing a recording at a medium track address.
#[cfg_attr(test, mockall::automock)]
pub trait PutAlbumTrackUseCase: Send + Sync {
    /// Place the recording, moving it from its previous medium placement.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn PutAlbumTrackUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: PutAlbumTrackCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutAlbumTrackResponse, PutAlbumTrackError>> + Send + 'future,
        >,
    >;
}
