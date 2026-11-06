use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::episode::Episode;

/// Command to create or replace one episode of a season.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PutSeriesEpisodeCommand {
    /// Date of the first airing of the episode, when known.
    air_date: Option<NaiveDate>,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Season-scoped index of the episode.
    episode_index: u32,
    /// Name of the episode.
    name: String,
    /// Plot of the episode, when known.
    plot: Option<String>,
    /// Series-scoped index of the season holding the episode.
    season_index: u32,
    /// Identifier of the show that holds the season.
    series_id: NumericID,
    /// Video backing the episode.
    video_id: NumericID,
}

impl PutSeriesEpisodeCommand {
    /// Return the date of the first airing, if known.
    #[must_use]
    pub(crate) fn air_date(&self) -> Option<NaiveDate> {
        self.air_date
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the season-scoped index of the episode.
    #[must_use]
    pub(crate) fn episode_index(&self) -> u32 {
        self.episode_index
    }

    /// Return the name of the episode.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new put-episode command.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the episode's write fields one-to-one"
    )]
    pub(crate) fn new(
        caller_id: NumericID,
        series_id: NumericID,
        season_index: u32,
        episode_index: u32,
        video_id: NumericID,
        name: String,
        plot: Option<String>,
        air_date: Option<NaiveDate>,
    ) -> Self {
        Self {
            air_date,
            caller_id,
            episode_index,
            name,
            plot,
            season_index,
            series_id,
            video_id,
        }
    }

    /// Return the plot, if known.
    #[must_use]
    pub(crate) fn plot(&self) -> Option<&str> {
        self.plot.as_deref()
    }

    /// Return the series-scoped index of the season holding the episode.
    #[must_use]
    pub(crate) fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show that holds the season.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }

    /// Return the video backing the episode.
    #[must_use]
    pub(crate) fn video_id(&self) -> NumericID {
        self.video_id
    }
}

/// Response of a successful episode write: the stored episode and whether the
/// write created it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct PutSeriesEpisodeResponse {
    /// Set when the write created the episode, cleared when it replaced one.
    created: bool,
    /// The episode that was stored.
    episode: Episode,
}

impl PutSeriesEpisodeResponse {
    /// Return whether the write created the episode.
    #[must_use]
    pub(crate) fn created(&self) -> bool {
        self.created
    }

    /// Return the episode that was stored.
    #[must_use]
    pub(crate) fn episode(&self) -> &Episode {
        &self.episode
    }

    /// Create a new written-episode response.
    #[must_use]
    pub(crate) fn new(episode: Episode, created: bool) -> Self {
        Self { created, episode }
    }
}

/// Error returned when writing an episode.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum PutSeriesEpisodeError {
    /// The video is already attached.
    #[error("the episode cannot be written")]
    Conflict,
    /// The caller may not manage the show.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The name is empty or made only of whitespace.
    #[error("the name must not be empty")]
    InvalidName,
    /// The name is longer than the name policy allows.
    #[error("the name is too long")]
    NameTooLong,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// The show holds no season at the requested index.
    #[error("a season with this index does not exist in the show")]
    NoSuchSeason,
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// No video matches the requested identifier.
    #[error("a video with this identifier does not exist")]
    NoSuchVideo,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for creating or replacing a season episode.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait PutSeriesEpisodeUseCase: Send + Sync {
    /// Create or replace the episode at the requested index.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn PutSeriesEpisodeUseCase>`.
    fn execute<'future>(
        &'future self,
        command: PutSeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutSeriesEpisodeResponse, PutSeriesEpisodeError>>
                + Send
                + 'future,
        >,
    >;
}
