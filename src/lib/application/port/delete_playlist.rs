use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to delete a playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DeletePlaylistCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the playlist to delete.
    playlist_id: NumericID,
}

impl DeletePlaylistCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new delete-playlist command.
    #[must_use]
    pub fn new(caller_id: NumericID, playlist_id: NumericID) -> Self {
        Self {
            caller_id,
            playlist_id,
        }
    }

    /// Return the identifier of the playlist to delete.
    #[must_use]
    pub fn playlist_id(&self) -> NumericID {
        self.playlist_id
    }
}

/// Error returned when deleting a playlist.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DeletePlaylistError {
    /// The caller may read the playlist but does not own it.
    #[error("only the owner may delete a playlist")]
    Forbidden,
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

/// Use case for deleting a playlist.
#[cfg_attr(test, mockall::automock)]
pub trait DeletePlaylistUseCase: Send + Sync {
    /// Delete the requested playlist and its track placements.
    fn execute<'future>(
        &'future self,
        command: DeletePlaylistCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeletePlaylistError>> + Send + 'future>>;
}
