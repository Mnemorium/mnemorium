/// Error returned when initialising a `MusicQuery`.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MusicQueryError {
    /// The query title is empty.
    #[error("a music query title must not be empty")]
    InvalidTitle,
}

/// Kind of music work a query may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum MusicKind {
    /// The abstract album, independent of any one edition.
    Album,
    /// One performance, independent of the release it appears on.
    Recording,
    /// One edition of an album, which is what owns mediums and tracks.
    Release,
}

impl MusicKind {
    /// Render the kind as its wire name (`album`, `release`, `recording`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::Recording => "recording",
            Self::Release => "release",
        }
    }

    /// Parse the kind a query or a route names (`album`, `release`,
    /// `recording`).
    ///
    /// Returns `None` when `value` is not a member of the discrete list, so the
    /// inbound handler can classify the rejection as `enum_validation_failed`
    /// (`OBS-006`).
    #[must_use]
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "album" => Some(Self::Album),
            "recording" => Some(Self::Recording),
            "release" => Some(Self::Release),
            _ => None,
        }
    }
}

/// One music query, for an album, a release or a recording.
///
/// A query carries only evidence the caller has: a group and a year are
/// absent when the caller does not know them, and a title is never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MusicQuery {
    /// Group the caller knows, when any.
    group: Option<String>,
    /// Kind of work the query searches for.
    kind: MusicKind,
    /// Title to search for; never empty.
    title: String,
    /// Release or first-release year, when the caller knows one.
    year: Option<u32>,
}

impl MusicQuery {
    /// Return the group the caller knows, when any.
    #[must_use]
    pub(crate) fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }

    /// Return the kind of work the query searches for.
    #[must_use]
    pub(crate) fn kind(&self) -> MusicKind {
        self.kind
    }

    /// Return the title to search for.
    #[must_use]
    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    /// Initialise a new `MusicQuery`, validating `title`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicQueryError::InvalidTitle`] when `title` is empty.
    pub(crate) fn try_new(
        kind: MusicKind,
        title: String,
        group: Option<String>,
        year: Option<u32>,
    ) -> Result<Self, MusicQueryError> {
        if title.is_empty() {
            return Err(MusicQueryError::InvalidTitle);
        }
        Ok(Self {
            group,
            kind,
            title,
            year,
        })
    }

    /// Return the release or first-release year, when the caller knows one.
    #[must_use]
    pub(crate) fn year(&self) -> Option<u32> {
        self.year
    }
}
