use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::music_playlist::MusicPlaylist;

/// Command to update a playlist's metadata and/or append a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UpdatePlaylistCommand {
    /// Identifier of the recording to append, when requested.
    add_track: Option<NumericID>,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// New visibility, when requested.
    is_public: Option<bool>,
    /// New name, when requested.
    name: Option<String>,
    /// Identifier of the playlist to update.
    playlist_id: NumericID,
}

impl UpdatePlaylistCommand {
    /// Return the identifier of the recording to append, if any.
    #[must_use]
    pub fn add_track(&self) -> Option<NumericID> {
        self.add_track
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the new visibility, if requested.
    #[must_use]
    pub fn is_public(&self) -> Option<bool> {
        self.is_public
    }

    /// Return the new name, if requested.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new update-playlist command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        playlist_id: NumericID,
        name: Option<String>,
        is_public: Option<bool>,
        add_track: Option<NumericID>,
    ) -> Self {
        Self {
            add_track,
            caller_id,
            is_public,
            name,
            playlist_id,
        }
    }

    /// Return the identifier of the playlist to update.
    #[must_use]
    pub fn playlist_id(&self) -> NumericID {
        self.playlist_id
    }
}

/// Response of a successful playlist update.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UpdatePlaylistResponse {
    /// The updated playlist.
    playlist: MusicPlaylist,
}

impl UpdatePlaylistResponse {
    /// Create a new update-playlist response.
    #[must_use]
    pub fn new(playlist: MusicPlaylist) -> Self {
        Self { playlist }
    }

    /// Return the updated playlist.
    #[must_use]
    pub fn playlist(&self) -> &MusicPlaylist {
        &self.playlist
    }
}

/// Error returned when updating a playlist.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UpdatePlaylistError {
    /// A concurrent change conflicts with the requested one.
    #[error("the playlist update conflicts with its current state")]
    Conflict,
    /// The caller may read the playlist but does not own it.
    #[error("only the owner may change a playlist's metadata")]
    Forbidden,
    /// The name is empty or made only of whitespace.
    #[error("playlist name must not be empty")]
    InvalidName,
    /// The caller already owns another playlist with the same name.
    #[error("the caller already owns a playlist with this name")]
    NameAlreadyExists,
    /// The name is longer than the maximum allowed length.
    #[error("playlist name must be at most 100 characters long")]
    NameTooLong,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No playlist matches the requested identifier, or the caller cannot see
    /// it.
    #[error("a music playlist with this identifier does not exist")]
    NoSuchPlaylist,
    /// The recording to append does not exist.
    #[error("a music recording with this identifier does not exist")]
    NoSuchRecording,
    /// The recording is already present in the playlist.
    #[error("the recording is already present in the playlist")]
    RecordingAlreadyPresent,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for updating a playlist.
#[cfg_attr(test, mockall::automock)]
pub trait UpdatePlaylistUseCase: Send + Sync {
    /// Apply the requested metadata and/or track change.
    fn execute<'future>(
        &'future self,
        command: UpdatePlaylistCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdatePlaylistResponse, UpdatePlaylistError>>
                + Send
                + 'future,
        >,
    >;
}
