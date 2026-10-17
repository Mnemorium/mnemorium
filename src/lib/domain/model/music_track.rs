use crate::domain::alias::NumericID;

/// Error returned when initialising a `MusicTrack`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MusicTrackError {
    /// The `track_index` is negative.
    #[error("track index must not be negative")]
    InvalidIndex,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// The container a `MusicTrack` is positioned by.
///
/// The shared `music_track` table gives every track exactly one container: a
/// medium of a `MusicAlbum` (`music_medium_id`) or a `MusicPlaylist`
/// (`music_playlist_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicTrackPosition {
    /// The track is positioned by a medium of a `MusicAlbum`.
    Medium(NumericID),
    /// The track is positioned by a `MusicPlaylist`.
    Playlist(NumericID),
}

/// An ordered membership of one recording in a medium or a playlist.
///
/// The same entity models both sides of the shared `music_track` table: an
/// album track is positioned by the `MusicMedium` that holds it, a playlist
/// track by its `MusicPlaylist`. A track carries no container of its own — the
/// container owns its tracks — and a track only ever names one of the two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicTrack {
    /// Unique identifier of the track.
    id: NumericID,
    /// Container the track is positioned by.
    position: MusicTrackPosition,
    /// Identifier of the recording the track plays.
    recording_id: NumericID,
    /// Position of the track within its container.
    track_index: i64,
}

impl MusicTrack {
    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the identifier of the positioning medium, when the track is an
    /// album track.
    #[must_use]
    pub fn medium_id(&self) -> Option<NumericID> {
        match self.position {
            MusicTrackPosition::Medium(medium_id) => Some(medium_id),
            MusicTrackPosition::Playlist(_) => None,
        }
    }

    /// Return the identifier of the positioning playlist, when the track is a
    /// playlist track.
    #[must_use]
    pub fn playlist_id(&self) -> Option<NumericID> {
        match self.position {
            MusicTrackPosition::Playlist(playlist_id) => Some(playlist_id),
            MusicTrackPosition::Medium(_) => None,
        }
    }

    /// Return the container the track is positioned by.
    #[must_use]
    pub fn position(&self) -> MusicTrackPosition {
        self.position
    }

    /// Return the identifier of the recording the track plays.
    #[must_use]
    pub fn recording_id(&self) -> NumericID {
        self.recording_id
    }

    /// Return the position of the track within its container.
    #[must_use]
    pub fn track_index(&self) -> i64 {
        self.track_index
    }

    /// Initialise a new `MusicTrack`, validating `track_index`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicTrackError::InvalidIndex`] when `track_index` is negative.
    pub fn try_new(
        id: NumericID,
        position: MusicTrackPosition,
        recording_id: NumericID,
        track_index: i64,
    ) -> Result<Self, MusicTrackError> {
        if track_index < 0 {
            return Err(MusicTrackError::InvalidIndex);
        }
        Ok(Self {
            id,
            position,
            recording_id,
            track_index,
        })
    }
}
