/// Error returned when initialising a `MediaQuery`.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MediaQueryError {
    /// The query title is empty.
    #[error("a media query title must not be empty")]
    InvalidTitle,
}

/// Kind of work a media name may denote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VideoKind {
    /// A self-contained film.
    Movie,
    /// A show whose story is told across ordered seasons of episodes.
    Series,
}

/// Position of an episode within a show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EpisodePosition {
    /// Index of the episode within its season.
    episode: u32,
    /// Index of the season within the show.
    season: u32,
}

impl EpisodePosition {
    /// Return the index of the episode within its season.
    #[must_use]
    pub fn episode(&self) -> u32 {
        self.episode
    }

    /// Create a new episode position.
    #[must_use]
    pub fn new(season: u32, episode: u32) -> Self {
        Self { episode, season }
    }

    /// Return the index of the season within the show.
    #[must_use]
    pub fn season(&self) -> u32 {
        self.season
    }
}

/// One search query a media name yields.
///
/// The parser produces these in `interpret` order and the use case searches them
/// in the same order. A query carries only evidence the name supports: a year is
/// absent when the name names none, and an episode position is absent unless both
/// the season and the episode are known.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MediaQuery {
    /// Episode position, when the name supports both a season and an episode.
    episode: Option<EpisodePosition>,
    /// Kind of work the query searches for.
    kind: VideoKind,
    /// Title to search for; never empty.
    title: String,
    /// Release or first-air year, when the name supports one.
    year: Option<u32>,
}

impl MediaQuery {
    /// Return the episode position, when the name supports one.
    #[must_use]
    pub fn episode(&self) -> Option<EpisodePosition> {
        self.episode
    }

    /// Return the kind of work the query searches for.
    #[must_use]
    pub fn kind(&self) -> VideoKind {
        self.kind
    }

    /// Return the title to search for.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Initialise a new `MediaQuery`, validating `title`.
    ///
    /// # Errors
    ///
    /// Returns [`MediaQueryError::InvalidTitle`] when `title` is empty.
    pub fn try_new(
        kind: VideoKind,
        title: String,
        year: Option<u32>,
        episode: Option<EpisodePosition>,
    ) -> Result<Self, MediaQueryError> {
        if title.is_empty() {
            return Err(MediaQueryError::InvalidTitle);
        }
        Ok(Self {
            episode,
            kind,
            title,
            year,
        })
    }

    /// Return the release or first-air year, when the name supports one.
    #[must_use]
    pub fn year(&self) -> Option<u32> {
        self.year
    }
}
