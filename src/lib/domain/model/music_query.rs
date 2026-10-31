/// Error returned when initialising a `MusicQuery`.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MusicQueryError {
    /// The query title is empty.
    #[error("a music query title must not be empty")]
    InvalidTitle,
}

/// Kind of music work a query may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MusicKind {
    /// The abstract album, independent of any one edition.
    Album,
    /// One performance, independent of the release it appears on.
    Recording,
    /// One edition of an album, which is what owns mediums and tracks.
    Release,
}

/// One music query, for an album, a release or a recording.
///
/// A query carries only evidence the caller has: an artist and a year are
/// absent when the caller does not know them, and a title is never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicQuery {
    /// Artist the caller knows, when any.
    artist: Option<String>,
    /// Kind of work the query searches for.
    kind: MusicKind,
    /// Title to search for; never empty.
    title: String,
    /// Release or first-release year, when the caller knows one.
    year: Option<u32>,
}

impl MusicQuery {
    /// Return the artist the caller knows, when any.
    #[must_use]
    pub fn artist(&self) -> Option<&str> {
        self.artist.as_deref()
    }

    /// Return the kind of work the query searches for.
    #[must_use]
    pub fn kind(&self) -> MusicKind {
        self.kind
    }

    /// Return the title to search for.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Initialise a new `MusicQuery`, validating `title`.
    ///
    /// # Errors
    ///
    /// Returns [`MusicQueryError::InvalidTitle`] when `title` is empty.
    pub fn try_new(
        kind: MusicKind,
        title: String,
        artist: Option<String>,
        year: Option<u32>,
    ) -> Result<Self, MusicQueryError> {
        if title.is_empty() {
            return Err(MusicQueryError::InvalidTitle);
        }
        Ok(Self {
            artist,
            kind,
            title,
            year,
        })
    }

    /// Return the release or first-release year, when the caller knows one.
    #[must_use]
    pub fn year(&self) -> Option<u32> {
        self.year
    }
}
