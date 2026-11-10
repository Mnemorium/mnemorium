use crate::domain::model::{ExternalRef, VideoKind};

/// One video search result, as any provider reports it.
///
/// A candidate is deliberately shallow: it holds only what one search call can
/// produce on every provider. It carries no score, popularity or artwork, and
/// the candidates stay in the provider's order because the system adds no
/// ranking of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct Candidate {
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
    pub(crate) fn kind(&self) -> VideoKind {
        self.kind
    }

    /// Create a new candidate.
    #[must_use]
    pub(crate) fn new(
        reference: ExternalRef,
        kind: VideoKind,
        title: String,
        year: Option<u32>,
    ) -> Self {
        Self {
            kind,
            reference,
            title,
            year,
        }
    }

    /// Return the opaque reference the provider issued.
    #[must_use]
    pub(crate) fn reference(&self) -> &ExternalRef {
        &self.reference
    }

    /// Return the title the provider reports.
    #[must_use]
    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    /// Return the release or first-air year, when the provider reports one.
    #[must_use]
    pub(crate) fn year(&self) -> Option<u32> {
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
pub(crate) enum Target {
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
    pub(crate) fn reference(&self) -> &ExternalRef {
        match self {
            Self::Episode { reference, .. } | Self::Movie(reference) | Self::Series(reference) => {
                reference
            }
        }
    }
}
