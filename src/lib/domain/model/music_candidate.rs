use crate::domain::model::external_ref::ExternalRef;
use crate::domain::model::music_metadata::{MediumSummary, ReleaseStatus};
use crate::domain::model::music_query::MusicKind;

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

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the music candidate carries provider detail the identification path does not read yet"
    )
)]
impl MusicCandidate {
    /// Return the reference to the album the release belongs to, when the
    /// candidate is a release that names one.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn album(&self) -> Option<&ExternalRef> {
        match self {
            Self::Album { .. } | Self::Recording { .. } => None,
            Self::Release { album, .. } => album.as_ref(),
        }
    }

    /// Return the group the provider reports, when it reports one.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn group(&self) -> Option<&str> {
        match self {
            Self::Album { group, .. }
            | Self::Recording { group, .. }
            | Self::Release { group, .. } => group.as_deref(),
        }
    }

    /// Return the kind of work the candidate denotes.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn kind(&self) -> MusicKind {
        match self {
            Self::Album { .. } => MusicKind::Album,
            Self::Recording { .. } => MusicKind::Recording,
            Self::Release { .. } => MusicKind::Release,
        }
    }

    /// Return the mediums of the release, when the candidate is one.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn mediums(&self) -> &[MediumSummary] {
        match self {
            Self::Album { .. } | Self::Recording { .. } => &[],
            Self::Release { mediums, .. } => mediums,
        }
    }

    /// Return the opaque reference the provider issued.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn reference(&self) -> &ExternalRef {
        match self {
            Self::Album { reference, .. }
            | Self::Recording { reference, .. }
            | Self::Release { reference, .. } => reference,
        }
    }

    /// Return the release status, when the candidate is a release and the
    /// provider reports a mappable one.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn status(&self) -> Option<ReleaseStatus> {
        match self {
            Self::Album { .. } | Self::Recording { .. } => None,
            Self::Release { status, .. } => *status,
        }
    }

    /// Return the title the provider reports.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn title(&self) -> &str {
        match self {
            Self::Album { title, .. }
            | Self::Recording { title, .. }
            | Self::Release { title, .. } => title,
        }
    }

    /// Return the release or first-release year, when the provider reports one.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn year(&self) -> Option<u32> {
        match self {
            Self::Album { year, .. }
            | Self::Recording { year, .. }
            | Self::Release { year, .. } => *year,
        }
    }
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
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn kind(&self) -> MusicKind {
        match self {
            Self::Album(_) => MusicKind::Album,
            Self::Recording(_) => MusicKind::Recording,
            Self::Release(_) => MusicKind::Release,
        }
    }

    /// Return the reference the target resolves through.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the arms read fields out of a non-Copy enum matched by reference"
    )]
    pub(crate) fn reference(&self) -> &ExternalRef {
        match self {
            Self::Album(reference) | Self::Recording(reference) | Self::Release(reference) => {
                reference
            }
        }
    }
}
