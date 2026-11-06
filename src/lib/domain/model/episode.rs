use chrono::NaiveDate;

use crate::domain::model::video::Video;

/// The season-scoped identity of an episode.
///
/// An episode is addressed by its index within its season; it carries no
/// separate identifier into the API or the representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct EpisodeIndex(u32);

impl EpisodeIndex {
    /// Initialise a new `EpisodeIndex`.
    #[must_use]
    pub(crate) fn new(value: u32) -> Self {
        Self(value)
    }

    /// Return the season-scoped index of the episode.
    #[must_use]
    pub(crate) fn value(self) -> u32 {
        self.0
    }
}

/// Error returned when initialising an `Episode`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum EpisodeError {
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

/// An episode of a season.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Episode {
    /// Date of the first airing of the episode, when known.
    air_date: Option<NaiveDate>,
    /// Season-scoped identity of the episode.
    index: EpisodeIndex,
    /// Name of the episode.
    name: String,
    /// Plot of the episode.
    plot: String,
    /// Video backing the episode.
    video: Video,
}

impl Episode {
    /// Return the date of the first airing of the episode, if known.
    #[must_use]
    pub(crate) fn air_date(&self) -> Option<NaiveDate> {
        self.air_date
    }

    /// Return the season-scoped identity of the episode.
    #[must_use]
    pub(crate) fn index(&self) -> EpisodeIndex {
        self.index
    }

    /// Return the name of the episode.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Return the plot of the episode.
    #[must_use]
    pub(crate) fn plot(&self) -> &str {
        &self.plot
    }

    /// Initialise a new `Episode`, validating `name`.
    ///
    /// # Errors
    ///
    /// Returns [`EpisodeError::InvalidName`] when `name` is empty.
    pub(crate) fn try_new(
        index: EpisodeIndex,
        name: String,
        plot: String,
        air_date: Option<NaiveDate>,
        video: Video,
    ) -> Result<Self, EpisodeError> {
        if name.is_empty() {
            return Err(EpisodeError::InvalidName);
        }
        Ok(Self {
            air_date,
            index,
            name,
            plot,
            video,
        })
    }

    /// Return the video backing the episode.
    #[must_use]
    pub(crate) fn video(&self) -> &Video {
        &self.video
    }
}
