use crate::domain::model::music_recording::MusicRecording;

/// The album-scoped identity of a medium.
///
/// The variant names the physical or digital medium type; its payload is the
/// index of the medium within its album. Only `CD` is modelled today, so the
/// adapter reports any other stored type as a data-integrity failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediumIndex {
    /// A compact disc at this album-scoped index.
    Cd(u8),
}

impl MediumIndex {
    /// Render the medium's stored type (`CD`).
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Cd(_) => "CD",
        }
    }

    /// Parse a medium index from its stored type and album-scoped index.
    ///
    /// Returns `None` when `medium_type` is not one of the types the schema
    /// admits, so a stored value outside the modelled list is reported as a
    /// data-integrity failure rather than an absent medium.
    #[must_use]
    pub fn from_wire(medium_type: &str, index: u8) -> Option<Self> {
        match medium_type {
            "CD" => Some(Self::Cd(index)),
            _ => None,
        }
    }

    /// Return the album-scoped index of the medium.
    #[must_use]
    pub fn index(self) -> u8 {
        match self {
            Self::Cd(index) => index,
        }
    }
}

/// A medium within a music album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicMedium {
    /// Album-scoped identity and type of the medium.
    index: MediumIndex,
    /// Recordings placed in the medium, in order.
    tracks: Vec<MusicRecording>,
}

impl MusicMedium {
    /// Return the album-scoped identity and type of the medium.
    #[must_use]
    pub fn index(&self) -> MediumIndex {
        self.index
    }

    /// Initialise a new `MusicMedium`.
    #[must_use]
    pub fn new(index: MediumIndex, tracks: Vec<MusicRecording>) -> Self {
        Self { index, tracks }
    }

    /// Return the recordings placed in the medium, in order.
    #[must_use]
    pub fn tracks(&self) -> &[MusicRecording] {
        &self.tracks
    }
}
