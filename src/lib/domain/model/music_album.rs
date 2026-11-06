use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::music_medium::MusicMedium;
use crate::domain::model::permission::{Protected, ResourceKind, ResourceRef};

/// Production type of a music album.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum ProductionType {
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

impl ProductionType {
    /// Render the production type as its stored representation (`STUDIO`).
    ///
    /// The names match the `chk_music_album_production_type` values of the
    /// `music_album` table.
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Compilation => "COMPILATION",
            Self::Demo => "DEMO",
            Self::DjMix => "DJMIX",
            Self::Live => "LIVE",
            Self::Mixtape => "MIXTAPE",
            Self::Remix => "REMIX",
            Self::Soundtrack => "SOUNDTRACK",
            Self::Studio => "STUDIO",
        }
    }

    /// Parse a production type from its stored representation (`STUDIO`).
    ///
    /// Returns `None` when `value` is not one of the tokens the
    /// `chk_music_album_production_type` check constraint admits, so a stored
    /// value outside the discrete list is reported as an absent production type
    /// rather than failing the read.
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "COMPILATION" => Some(Self::Compilation),
            "DEMO" => Some(Self::Demo),
            "DJMIX" => Some(Self::DjMix),
            "LIVE" => Some(Self::Live),
            "MIXTAPE" => Some(Self::Mixtape),
            "REMIX" => Some(Self::Remix),
            "SOUNDTRACK" => Some(Self::Soundtrack),
            "STUDIO" => Some(Self::Studio),
            _ => None,
        }
    }
}

/// Error returned when initialising a `MusicAlbum`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MusicAlbumError {
    /// A genre identifier is empty.
    #[error("a genre identifier must not be empty")]
    InvalidGenre,
    /// The `name` is empty.
    #[error("name must not be empty")]
    InvalidName,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    #[expect(
        dead_code,
        reason = "the error enum keeps the catch-all variant the rulebook's error shape declares; no caller constructs it yet"
    )]
    Unknown(#[source] anyhow::Error),
}

/// A music album.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MusicAlbum {
    /// Identifier of the image holding the album's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Identifiers of the genres the album is associated with.
    genre_ids: Vec<String>,
    /// Unique identifier of the album.
    id: NumericID,
    /// Mediums of the album, in order.
    mediums: Vec<MusicMedium>,
    /// Identifier of the music group the album belongs to.
    music_group_id: NumericID,
    /// Identifiers of the music groups associated with the album, when set.
    music_group_ids: Option<Vec<NumericID>>,
    /// Name of the album.
    name: String,
    /// Production type of the album, when set.
    production_type: Option<ProductionType>,
    /// Date of the release of the album, when the provider reports one.
    release_date: Option<NaiveDate>,
}

impl MusicAlbum {
    /// Return the identifier of the image holding the album's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the identifiers of the genres the album is associated with.
    #[must_use]
    pub(crate) fn genre_ids(&self) -> &[String] {
        &self.genre_ids
    }

    /// Return the unique identifier.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the mediums, in order.
    #[must_use]
    pub(crate) fn mediums(&self) -> &[MusicMedium] {
        &self.mediums
    }

    /// Return the identifier of the music group the album belongs to.
    #[must_use]
    pub(crate) fn music_group_id(&self) -> NumericID {
        self.music_group_id
    }

    /// Return the identifiers of the associated music groups, if any.
    #[must_use]
    pub(crate) fn music_group_ids(&self) -> Option<&[NumericID]> {
        self.music_group_ids.as_deref()
    }

    /// Return the name.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Return the production type, if any.
    #[must_use]
    pub(crate) fn production_type(&self) -> Option<ProductionType> {
        self.production_type
    }

    /// Return the date of the release, when the provider reports one.
    #[must_use]
    pub(crate) fn release_date(&self) -> Option<NaiveDate> {
        self.release_date
    }

    /// Initialise a new `MusicAlbum`, validating `name` and `genre_ids`, and
    /// defaulting `music_group_ids` to `None` when empty.
    ///
    /// # Errors
    ///
    /// Returns [`MusicAlbumError::InvalidName`] when `name` is empty, and
    /// [`MusicAlbumError::InvalidGenre`] when a genre identifier is empty.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the album's fields one-to-one"
    )]
    pub(crate) fn try_new(
        id: NumericID,
        name: String,
        release_date: Option<NaiveDate>,
        production_type: Option<ProductionType>,
        genre_ids: Vec<String>,
        music_group_id: NumericID,
        music_group_ids: Option<Vec<NumericID>>,
        cover_image_id: Option<NumericID>,
        mediums: Vec<MusicMedium>,
    ) -> Result<Self, MusicAlbumError> {
        if name.is_empty() {
            return Err(MusicAlbumError::InvalidName);
        }
        if genre_ids.iter().any(String::is_empty) {
            return Err(MusicAlbumError::InvalidGenre);
        }
        let normalized_music_group_ids = match music_group_ids {
            Some(ids) if ids.is_empty() => None,
            ids => ids,
        };
        Ok(Self {
            cover_image_id,
            genre_ids,
            id,
            mediums,
            music_group_id,
            music_group_ids: normalized_music_group_ids,
            name,
            production_type,
            release_date,
        })
    }
}

impl Protected for MusicAlbum {
    fn is_privileged_account(&self) -> bool {
        false
    }

    fn is_publicly_visible(&self) -> bool {
        true
    }

    fn owner_id(&self) -> Option<NumericID> {
        None
    }

    fn resource_ref(&self) -> Option<ResourceRef> {
        Some(ResourceRef::new(ResourceKind::MusicAlbum, self.id))
    }
}
