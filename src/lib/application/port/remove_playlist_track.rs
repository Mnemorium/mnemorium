use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to remove one track of a playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RemovePlaylistTrackCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the playlist the track belongs to.
    playlist_id: NumericID,
    /// Zero-based index of the track within the playlist.
    track_index: u32,
}

impl RemovePlaylistTrackCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new remove-playlist-track command.
    #[must_use]
    pub fn new(caller_id: NumericID, playlist_id: NumericID, track_index: u32) -> Self {
        Self {
            caller_id,
            playlist_id,
            track_index,
        }
    }

    /// Return the identifier of the playlist the track belongs to.
    #[must_use]
    pub fn playlist_id(&self) -> NumericID {
        self.playlist_id
    }

    /// Return the zero-based index of the track within the playlist.
    #[must_use]
    pub fn track_index(&self) -> u32 {
        self.track_index
    }
}

/// Error returned when removing one track of a playlist.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RemovePlaylistTrackError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No playlist matches the requested identifier, or the caller cannot see
    /// it.
    #[error("a music playlist with this identifier does not exist")]
    NoSuchPlaylist,
    /// No track of the playlist matches the requested index.
    #[error("a playlist track with this index does not exist")]
    NoSuchTrack,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for removing one track of a playlist.
#[cfg_attr(test, mockall::automock)]
pub trait RemovePlaylistTrackUseCase: Send + Sync {
    /// Remove the requested track placement.
    fn execute<'future>(
        &'future self,
        command: RemovePlaylistTrackCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), RemovePlaylistTrackError>> + Send + 'future>>;
}
