use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::music_medium::MusicMedium;

/// Production type of a music album.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProductionType {
    /// Compilation of tracks from various sources.
    Compilation,
    /// Demo recording.
    Demo,
    /// Continuous DJ mix.
    DjMix,
    /// Live recording.
    Live,
    /// Mixtape.
    Mixtape,
    /// Remix.
    Remix,
    /// Soundtrack.
    Soundtrack,
    /// Studio recording.
    Studio,
}

/// Error returned when initialising a `MusicAlbum`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MusicAlbumError {
    /// The `genre`, when set, is empty.
    #[error("genre must not be empty when set")]
    InvalidGenre,
    /// The `name` is empty.
    #[error("name must not be empty")]
    InvalidName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// A music album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicAlbum {
    /// Genre of the album, when set.
    genre: Option<String>,
    /// Unique identifier of the album.
    id: NumericID,
    /// Mediums of the album, in order.
    mediums: Vec<MusicMedium>,
    /// Identifiers of the music groups associated with the album, when set.
    music_group_ids: Option<Vec<NumericID>>,
    /// Name of the album.
    name: String,
    /// Production type of the album, when set.
    production_type: Option<ProductionType>,
    /// Date of the release of the album.
    release_date: NaiveDate,
}

impl MusicAlbum {
    /// Return the genre, if any.
    #[must_use]
    pub fn genre(&self) -> Option<&str> {
        self.genre.as_deref()
    }

    /// Return the unique identifier.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the mediums, in order.
    #[must_use]
    pub fn mediums(&self) -> &[MusicMedium] {
        &self.mediums
    }

    /// Return the identifiers of the associated music groups, if any.
    #[must_use]
    pub fn music_group_ids(&self) -> Option<&[NumericID]> {
        self.music_group_ids.as_deref()
    }

    /// Return the name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the production type, if any.
    #[must_use]
    pub fn production_type(&self) -> Option<ProductionType> {
        self.production_type
    }

    /// Return the date of the release.
    #[must_use]
    pub fn release_date(&self) -> NaiveDate {
        self.release_date
    }

    /// Initialise a new `MusicAlbum`, validating `name` and `genre`, and
    /// defaulting `music_group_ids` to `None` when empty.
    ///
    /// # Errors
    ///
    /// Returns [`MusicAlbumError::InvalidName`] when `name` is empty, and
    /// [`MusicAlbumError::InvalidGenre`] when `genre` is set and empty.
    pub fn try_new(
        id: NumericID,
        name: String,
        release_date: NaiveDate,
        production_type: Option<ProductionType>,
        genre: Option<String>,
        music_group_ids: Option<Vec<NumericID>>,
        mediums: Vec<MusicMedium>,
    ) -> Result<Self, MusicAlbumError> {
        if name.is_empty() {
            return Err(MusicAlbumError::InvalidName);
        }
        if genre.as_ref().is_some_and(String::is_empty) {
            return Err(MusicAlbumError::InvalidGenre);
        }
        let normalized_music_group_ids = match music_group_ids {
            Some(ids) if ids.is_empty() => None,
            ids => ids,
        };
        Ok(Self {
            genre,
            id,
            mediums,
            music_group_ids: normalized_music_group_ids,
            name,
            production_type,
            release_date,
        })
    }
}
