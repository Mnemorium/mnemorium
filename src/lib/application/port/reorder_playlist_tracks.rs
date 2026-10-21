use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::music_playlist::MusicPlaylist;

/// Command to reorder the tracks of a playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReorderPlaylistTracksCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Complete permutation of the playlist's current track indices.
    order: Vec<u32>,
    /// Identifier of the playlist to reorder.
    playlist_id: NumericID,
}

impl ReorderPlaylistTracksCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new reorder-playlist-tracks command.
    #[must_use]
    pub fn new(caller_id: NumericID, playlist_id: NumericID, order: Vec<u32>) -> Self {
        Self {
            caller_id,
            order,
            playlist_id,
        }
    }

    /// Return the requested order.
    #[must_use]
    pub fn order(&self) -> &[u32] {
        &self.order
    }

    /// Return the identifier of the playlist to reorder.
    #[must_use]
    pub fn playlist_id(&self) -> NumericID {
        self.playlist_id
    }
}

/// Response of a successful reorder.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReorderPlaylistTracksResponse {
    /// The reordered playlist.
    playlist: MusicPlaylist,
}

impl ReorderPlaylistTracksResponse {
    /// Create a new reorder response.
    #[must_use]
    pub fn new(playlist: MusicPlaylist) -> Self {
        Self { playlist }
    }

    /// Return the reordered playlist.
    #[must_use]
    pub fn playlist(&self) -> &MusicPlaylist {
        &self.playlist
    }
}

/// Error returned when reordering the tracks of a playlist.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReorderPlaylistTracksError {
    /// The requested order is not a complete permutation of the current track
    /// indices.
    #[error("the requested order is not a valid permutation of the playlist tracks")]
    InvalidOrder,
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

/// Use case for reordering the tracks of a playlist.
#[cfg_attr(test, mockall::automock)]
pub trait ReorderPlaylistTracksUseCase: Send + Sync {
    /// Replace the playlist's track order with the requested permutation.
    fn execute<'future>(
        &'future self,
        command: ReorderPlaylistTracksCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ReorderPlaylistTracksResponse, ReorderPlaylistTracksError>>
                + Send
                + 'future,
        >,
    >;
}
