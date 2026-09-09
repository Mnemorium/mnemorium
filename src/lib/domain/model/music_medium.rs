use crate::domain::alias::NumericID;
use crate::domain::model::music_recording::MusicRecording;

/// Default value of `MusicMedium::medium_index` when no index is provided.
pub const DEFAULT_MEDIUM_INDEX: u32 = 1;

/// A medium within a music album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicMedium {
    /// Unique identifier of the medium.
    id: NumericID,
    /// Index of the medium within its album, defaulting to
    /// [`DEFAULT_MEDIUM_INDEX`].
    medium_index: u32,
    /// Type of the medium.
    medium_type: String,
    /// Tracks of the medium, in order.
    tracks: Vec<MusicRecording>,
}

impl MusicMedium {
    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the index of the medium within its album.
    #[must_use]
    pub fn medium_index(&self) -> u32 {
        self.medium_index
    }

    /// Return the type of the medium.
    #[must_use]
    pub fn medium_type(&self) -> &str {
        &self.medium_type
    }

    /// Initialise a new `MusicMedium`, defaulting `medium_index` to
    /// [`DEFAULT_MEDIUM_INDEX`] when absent.
    #[must_use]
    pub fn new(
        id: NumericID,
        medium_index: Option<u32>,
        medium_type: String,
        tracks: Vec<MusicRecording>,
    ) -> Self {
        Self {
            id,
            medium_index: medium_index.unwrap_or(DEFAULT_MEDIUM_INDEX),
            medium_type,
            tracks,
        }
    }

    /// Return the tracks, in order.
    #[must_use]
    pub fn tracks(&self) -> &[MusicRecording] {
        &self.tracks
    }
}
