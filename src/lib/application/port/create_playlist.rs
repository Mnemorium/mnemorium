use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::music_playlist::MusicPlaylist;

/// Command to create a playlist owned by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CreatePlaylistCommand {
    /// Identifier of the authenticated caller that will own the playlist.
    caller_id: NumericID,
    /// Whether any authenticated user may read and moderate the playlist.
    is_public: bool,
    /// Name of the playlist.
    name: String,
}

impl CreatePlaylistCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return whether the playlist is public.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the name of the playlist.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new create-playlist command.
    #[must_use]
    pub fn new(caller_id: NumericID, name: String, is_public: bool) -> Self {
        Self {
            caller_id,
            is_public,
            name,
        }
    }
}

/// Response of a successful playlist creation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CreatePlaylistResponse {
    /// The created playlist.
    playlist: MusicPlaylist,
}

impl CreatePlaylistResponse {
    /// Create a new create-playlist response.
    #[must_use]
    pub fn new(playlist: MusicPlaylist) -> Self {
        Self { playlist }
    }

    /// Return the created playlist.
    #[must_use]
    pub fn playlist(&self) -> &MusicPlaylist {
        &self.playlist
    }
}

/// Error returned when creating a playlist.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CreatePlaylistError {
    /// The name is empty or made only of whitespace.
    #[error("playlist name must not be empty")]
    InvalidName,
    /// The caller already owns a playlist with the same name.
    #[error("the caller already owns a playlist with this name")]
    NameAlreadyExists,
    /// The name is longer than the maximum allowed length.
    #[error("playlist name must be at most 100 characters long")]
    NameTooLong,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for creating a playlist.
#[cfg_attr(test, mockall::automock)]
pub trait CreatePlaylistUseCase: Send + Sync {
    /// Create a playlist owned by the authenticated caller.
    fn execute<'future>(
        &'future self,
        command: CreatePlaylistCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreatePlaylistResponse, CreatePlaylistError>>
                + Send
                + 'future,
        >,
    >;
}
