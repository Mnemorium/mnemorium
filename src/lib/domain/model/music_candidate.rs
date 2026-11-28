use crate::domain::model::{ExternalRef, MediumSummary, MusicKind, ReleaseStatus};

/// One music search result, as any provider reports it.
///
/// A candidate is deliberately shallow: it holds only what one search call can
/// produce. A release candidate also names the album it belongs to, its mediums
/// and its status, because those are part of an edition rather than of the
/// album's identity. The fields that only a release carries are absent from the
/// other kinds rather than repeated empty.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum MusicCandidate {
    /// The abstract album, independent of any one edition.
    Album {
        /// Group the provider reports, when it reports one.
        group: Option<String>,
        /// Opaque reference the provider issued for the album.
        reference: ExternalRef,
        /// Title the provider reports.
        title: String,
        /// Release or first-release year, when the provider reports one.
        year: Option<u32>,
    },
    /// One performance, independent of the release it appears on.
    Recording {
        /// Group the provider reports, when it reports one.
        group: Option<String>,
        /// Opaque reference the provider issued for the recording.
        reference: ExternalRef,
        /// Title the provider reports.
        title: String,
        /// Release or first-release year, when the provider reports one.
        year: Option<u32>,
    },
    /// One edition of an album.
    Release {
        /// Reference to the album the release belongs to, when the provider
        /// reports one.
        album: Option<ExternalRef>,
        /// Group the provider reports, when it reports one.
        group: Option<String>,
        /// Mediums of the release, in the provider's order.
        mediums: Vec<MediumSummary>,
        /// Opaque reference the provider issued for the release.
        reference: ExternalRef,
        /// Status the provider reports, when it reports a mappable one.
        status: Option<ReleaseStatus>,
        /// Title the provider reports.
        title: String,
        /// Release year, when the provider reports one.
        year: Option<u32>,
    },
}

/// Item whose rich metadata is resolved through the reference's provider.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum MusicTarget {
    /// One album.
    Album(ExternalRef),
    /// One recording.
    Recording(ExternalRef),
    /// One release.
    Release(ExternalRef),
}

impl MusicTarget {
    /// Return the kind of work the target resolves.
    #[must_use]
    pub(crate) fn kind(&self) -> MusicKind {
        match *self {
            Self::Album(_) => MusicKind::Album,
            Self::Recording(_) => MusicKind::Recording,
            Self::Release(_) => MusicKind::Release,
        }
    }

    /// Return the reference the target resolves through.
    #[must_use]
    pub(crate) fn reference(&self) -> &ExternalRef {
        match self {
            Self::Album(reference) | Self::Recording(reference) | Self::Release(reference) => {
                reference
            }
        }
    }
}
