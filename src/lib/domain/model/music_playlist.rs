use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::music_recording::MusicRecording;

/// Error returned when initialising or updating a `MusicPlaylist`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MusicPlaylistError {
    /// The `name` is empty.
    #[error("name must not be empty")]
    InvalidName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A music playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicPlaylist {
    /// Date and time of the creation of the playlist.
    created_at: NaiveDateTime,
    /// Unique identifier of the playlist.
    id: NumericID,
    /// Whether the playlist is visible to every user.
    is_public: bool,
    /// Name of the playlist.
    name: String,
    /// Tracks of the playlist, in order.
    tracks: Vec<MusicRecording>,
    /// Identifier of the user owning the playlist.
    user_id: NumericID,
}

impl MusicPlaylist {
    /// Return the date and time of the creation of the playlist.
    #[must_use]
    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return whether the playlist is visible to every user.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the tracks, in order.
    #[must_use]
    pub fn tracks(&self) -> &[MusicRecording] {
        &self.tracks
    }

    /// Initialise a new `MusicPlaylist`, validating `name`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicPlaylistError::InvalidName`] when `name` is empty.
    pub fn try_new(
        id: NumericID,
        name: String,
        created_at: NaiveDateTime,
        user_id: NumericID,
        is_public: bool,
        tracks: Vec<MusicRecording>,
    ) -> Result<Self, MusicPlaylistError> {
        if name.is_empty() {
            return Err(MusicPlaylistError::InvalidName);
        }
        Ok(Self {
            created_at,
            id,
            is_public,
            name,
            tracks,
            user_id,
        })
    }

    /// Return the identifier of the user owning the playlist.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}
