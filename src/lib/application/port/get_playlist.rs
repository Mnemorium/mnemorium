use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::music_playlist::MusicPlaylist;

/// Command to fetch one playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetPlaylistCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the playlist to fetch.
    playlist_id: NumericID,
}

impl GetPlaylistCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-playlist command.
    #[must_use]
    pub fn new(caller_id: NumericID, playlist_id: NumericID) -> Self {
        Self {
            caller_id,
            playlist_id,
        }
    }

    /// Return the identifier of the playlist to fetch.
    #[must_use]
    pub fn playlist_id(&self) -> NumericID {
        self.playlist_id
    }
}

/// Response of a successful playlist lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetPlaylistResponse {
    /// The fetched playlist.
    playlist: MusicPlaylist,
}

impl GetPlaylistResponse {
    /// Create a new fetch-playlist response.
    #[must_use]
    pub fn new(playlist: MusicPlaylist) -> Self {
        Self { playlist }
    }

    /// Return the fetched playlist.
    #[must_use]
    pub fn playlist(&self) -> &MusicPlaylist {
        &self.playlist
    }
}

/// Error returned when fetching one playlist.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetPlaylistError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No playlist matches the requested identifier, or the caller cannot see
    /// it.
    #[error("a music playlist with this identifier does not exist")]
    NoSuchPlaylist,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching one playlist.
#[cfg_attr(test, mockall::automock)]
pub trait GetPlaylistUseCase: Send + Sync {
    /// Fetch the requested playlist with its tracks.
    fn execute<'future>(
        &'future self,
        command: GetPlaylistCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetPlaylistResponse, GetPlaylistError>> + Send + 'future>>;
}
