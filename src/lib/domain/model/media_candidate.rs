use crate::domain::model::media_query::VideoKind;

/// Identifier of an external metadata provider.
///
/// The identifier is provider-tagged rather than positional: a reference one
/// provider issues means nothing to another, so every reference and every
/// configured link carries the provider that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ProviderId {
    /// The Open Movie Database.
    Omdb,
    /// The Movie Database.
    Tmdb,
}

impl ProviderId {
    /// Return the provider name as the configuration declares it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Omdb => "omdb",
            Self::Tmdb => "tmdb",
        }
    }

    /// Parse the provider name a configuration entry declares.
    ///
    /// The comparison is case-insensitive and ignores surrounding whitespace, so
    /// `TMDB` and ` tmdb ` both name the same provider.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "omdb" => Some(Self::Omdb),
            "tmdb" => Some(Self::Tmdb),
            _ => None,
        }
    }
}

/// Opaque reference to one item at a provider.
///
/// The identifier is opaque: no layer outside the provider's adapter interprets
/// it. The reference stays provider-tagged because the future manual flow sends
/// a selection back to the provider that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExternalRef {
    /// Identifier the provider issued for the item.
    id: String,
    /// Provider that issued the identifier.
    provider: ProviderId,
}

impl ExternalRef {
    /// Return the identifier the provider issued.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Create a new external reference.
    #[must_use]
    pub fn new(provider: ProviderId, id: String) -> Self {
        Self { id, provider }
    }

    /// Return the provider that issued the identifier.
    #[must_use]
    pub fn provider(&self) -> ProviderId {
        self.provider
    }
}

/// One search result, as any provider reports it.
///
/// A candidate is deliberately shallow: it holds only what one search call can
/// produce on every provider. It carries no score, popularity or artwork, and
/// the candidates stay in the provider's order because the system adds no
/// ranking of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Candidate {
    /// Kind of work the candidate denotes.
    kind: VideoKind,
    /// Opaque reference the provider issued.
    reference: ExternalRef,
    /// Title the provider reports.
    title: String,
    /// Release or first-air year, when the provider reports one.
    year: Option<u32>,
}

impl Candidate {
    /// Return the kind of work the candidate denotes.
    #[must_use]
    pub fn kind(&self) -> VideoKind {
        self.kind
    }

    /// Create a new candidate.
    #[must_use]
    pub fn new(reference: ExternalRef, kind: VideoKind, title: String, year: Option<u32>) -> Self {
        Self {
            kind,
            reference,
            title,
            year,
        }
    }

    /// Return the opaque reference the provider issued.
    #[must_use]
    pub fn reference(&self) -> &ExternalRef {
        &self.reference
    }

    /// Return the title the provider reports.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return the release or first-air year, when the provider reports one.
    #[must_use]
    pub fn year(&self) -> Option<u32> {
        self.year
    }
}

/// Item whose rich metadata is resolved through the reference's provider.
///
/// An episode names the series reference it belongs to plus the season and
/// episode position, so a provider that charges one request per episode costs
/// one request here rather than a full season tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Target {
    /// One episode of the show the reference names.
    Episode {
        /// Index of the episode within its season.
        episode: u32,
        /// Reference to the show, issued by its provider.
        reference: ExternalRef,
        /// Index of the season within the show.
        season: u32,
    },
    /// One film.
    Movie(ExternalRef),
    /// One show.
    Series(ExternalRef),
}

impl Target {
    /// Return the reference the target resolves through.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms borrow the reference out of a non-Copy enum matched by reference"
    )]
    pub fn reference(&self) -> &ExternalRef {
        match self {
            Self::Episode { reference, .. } | Self::Movie(reference) | Self::Series(reference) => {
                reference
            }
        }
    }
}
