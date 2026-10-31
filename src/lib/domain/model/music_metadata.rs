use chrono::NaiveDate;

use crate::domain::model::artwork::Artwork;
use crate::domain::model::media_candidate::ExternalRef;
use crate::domain::model::music_album::ProductionType;

/// One name a provider credits for an album, a release or a recording.
///
/// A credit is the artist's name as the provider reports it plus, when the
/// provider joins several artists, the phrase that follows this name. The
/// phrase is metadata, never a separator the system invents.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ArtistCredit {
    /// Phrase the provider places after the name, when it reports one.
    join_phrase: Option<String>,
    /// Name the provider credits.
    name: String,
}

impl ArtistCredit {
    /// Return the phrase that follows this name, when the provider reports one.
    #[must_use]
    pub fn join_phrase(&self) -> Option<&str> {
        self.join_phrase.as_deref()
    }

    /// Return the name the provider credits.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new artist credit.
    #[must_use]
    pub fn new(name: String, join_phrase: Option<String>) -> Self {
        Self { join_phrase, name }
    }
}

/// Format of a medium within an edition.
///
/// Only a compact disc is named: it is the one format the catalogue can
/// materialise today, and the one the ordering rule looks for. Any other
/// reported format is carried verbatim so metadata stays faithful without the
/// catalogue pretending to model it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediumFormat {
    /// A compact disc.
    Cd,
    /// Any other format the provider reports.
    Other(String),
}

impl MediumFormat {
    /// Return whether the medium is a compact disc.
    #[must_use]
    pub fn is_compact_disc(&self) -> bool {
        matches!(self, Self::Cd)
    }
}

/// One medium of an edition, as identification reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MediumSummary {
    /// Format of the medium.
    format: MediumFormat,
    /// Album-scoped position of the medium, starting at one.
    index: u32,
    /// Number of tracks the medium carries.
    track_count: u32,
}

impl MediumSummary {
    /// Return the format of the medium.
    #[must_use]
    pub fn format(&self) -> &MediumFormat {
        &self.format
    }

    /// Return the album-scoped position of the medium, starting at one.
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Create a new medium summary.
    #[must_use]
    pub fn new(format: MediumFormat, index: u32, track_count: u32) -> Self {
        Self {
            format,
            index,
            track_count,
        }
    }

    /// Return the number of tracks the medium carries.
    #[must_use]
    pub fn track_count(&self) -> u32 {
        self.track_count
    }
}

/// Status a provider reports for one release.
///
/// The list is the catalogue's: a provider spelling an equivalent status maps
/// onto one of these values, and a status the list does not contain is absent
/// rather than guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReleaseStatus {
    /// An unofficial release.
    Bootleg,
    /// A release the provider marks as cancelled.
    Cancelled,
    /// The release the artist or label authorised.
    Official,
    /// A promotional release.
    Promotion,
    /// A release that exists only as a legal fiction for licensing.
    PseudoRelease,
    /// A release withdrawn after publication.
    Withdrawn,
}

impl ReleaseStatus {
    /// Map a provider's status name onto the catalogue's list.
    ///
    /// Normalization folds case, trims, and treats spaces, hyphens and
    /// underscores alike, so `Pseudo-Release` and `pseudo_release` both resolve
    /// to [`ReleaseStatus::PseudoRelease`]. A value the list does not contain is
    /// absent, never guessed.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let normalized = name
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_'], " ")
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        match normalized.as_str() {
            "bootleg" => Some(Self::Bootleg),
            "cancelled" | "canceled" => Some(Self::Cancelled),
            "official" => Some(Self::Official),
            "promotion" => Some(Self::Promotion),
            "pseudo release" => Some(Self::PseudoRelease),
            "withdrawn" => Some(Self::Withdrawn),
            _ => None,
        }
    }

    /// Return whether the status is the official one.
    #[must_use]
    pub fn is_official(self) -> bool {
        matches!(self, Self::Official)
    }
}

/// One edition of an album.
///
/// An edition carries the values that distinguish one pressing from another. A
/// date, a country or a status the provider does not report is absent rather
/// than replaced by a placeholder or a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Edition {
    /// Country of release as an uppercase ISO 3166-1 alpha-2 code, when the
    /// provider reports a mappable one.
    country: Option<String>,
    /// Mediums of the edition, in the provider's order.
    mediums: Vec<MediumSummary>,
    /// Opaque reference the provider issued for the edition.
    reference: ExternalRef,
    /// Date of release, when the provider reports a complete one.
    release_date: Option<NaiveDate>,
    /// Status the provider reports, when it reports a mappable one.
    status: Option<ReleaseStatus>,
    /// Title the provider reports for the edition.
    title: String,
}

impl Edition {
    /// Return the country of release, when the provider reports a mappable one.
    #[must_use]
    pub fn country(&self) -> Option<&str> {
        self.country.as_deref()
    }

    /// Return the mediums of the edition, in the provider's order.
    #[must_use]
    pub fn mediums(&self) -> &[MediumSummary] {
        &self.mediums
    }

    /// Create a new edition.
    #[must_use]
    pub fn new(
        reference: ExternalRef,
        title: String,
        release_date: Option<NaiveDate>,
        country: Option<String>,
        status: Option<ReleaseStatus>,
        mediums: Vec<MediumSummary>,
    ) -> Self {
        Self {
            country,
            mediums,
            reference,
            release_date,
            status,
            title,
        }
    }

    /// Return the opaque reference the provider issued for the edition.
    #[must_use]
    pub fn reference(&self) -> &ExternalRef {
        &self.reference
    }

    /// Return the date of release, when the provider reports a complete one.
    #[must_use]
    pub fn release_date(&self) -> Option<NaiveDate> {
        self.release_date
    }

    /// Return the status, when the provider reports a mappable one.
    #[must_use]
    pub fn status(&self) -> Option<ReleaseStatus> {
        self.status
    }

    /// Return the title the provider reports for the edition.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }
}

/// Metadata a provider reports for an album, a release or a recording.
///
/// The shape does not depend on the provider that answered. Absence is `None`
/// or an empty list: a partial date, a placeholder country, an unmappable
/// production type or status, and a genre outside the provider's curated
/// vocabulary are absent rather than fabricated. Genres are the vocabulary the
/// provider curates, normalized so equivalent spellings resolve to one name;
/// the film and television genre vocabulary does not apply here.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicMetadata {
    /// One album and the editions the provider lists for it.
    Album {
        /// Artwork the provider reports, when the download succeeds.
        artwork: Option<Artwork>,
        /// Names the provider credits.
        artist_credits: Vec<ArtistCredit>,
        /// Editions the provider lists, ordered by the edition rule.
        editions: Vec<Edition>,
        /// Date of first release, when the provider reports a complete one.
        first_release_date: Option<NaiveDate>,
        /// Genres from the provider's curated vocabulary.
        genres: Vec<String>,
        /// Production type from the catalogue's list, when it maps.
        production_type: Option<ProductionType>,
        /// Title the provider reports.
        title: String,
    },
    /// One recording, independent of any release.
    Recording {
        /// Names the provider credits.
        artist_credits: Vec<ArtistCredit>,
        /// Date of first release, when the provider reports a complete one.
        first_release_date: Option<NaiveDate>,
        /// Genres from the provider's curated vocabulary.
        genres: Vec<String>,
        /// ISRC codes the provider reports.
        isrcs: Vec<String>,
        /// Title the provider reports.
        title: String,
    },
    /// One edition of an album.
    Release {
        /// Reference to the album the release belongs to, when the provider
        /// reports one.
        album: Option<ExternalRef>,
        /// Artwork the provider reports, when the download succeeds.
        artwork: Option<Artwork>,
        /// Names the provider credits.
        artist_credits: Vec<ArtistCredit>,
        /// The edition itself: title, date, country, status and mediums.
        edition: Edition,
        /// Genres from the provider's curated vocabulary.
        genres: Vec<String>,
    },
}
