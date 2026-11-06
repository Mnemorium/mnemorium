use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::video::Video;

/// Command to fetch one episode of a season.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetSeriesEpisodeCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Season-scoped index of the episode to fetch.
    episode_index: u32,
    /// Series-scoped index of the season holding the episode.
    season_index: u32,
    /// Identifier of the show holding the season.
    series_id: NumericID,
}

impl GetSeriesEpisodeCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the season-scoped index of the episode to fetch.
    #[must_use]
    pub(crate) fn episode_index(&self) -> u32 {
        self.episode_index
    }

    /// Create a new fetch-episode command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        series_id: NumericID,
        season_index: u32,
        episode_index: u32,
    ) -> Self {
        Self {
            caller_id,
            episode_index,
            season_index,
            series_id,
        }
    }

    /// Return the series-scoped index of the season holding the episode.
    #[must_use]
    pub(crate) fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show holding the season.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Response of a successful episode lookup: one episode.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct GetSeriesEpisodeResponse {
    /// Date of the first airing of the episode, when known.
    air_date: Option<NaiveDate>,
    /// Season-scoped index of the episode.
    id: u32,
    /// Name of the episode.
    name: String,
    /// Plot of the episode.
    plot: String,
    /// Series-scoped index of the season holding the episode.
    season_index: u32,
    /// Identifier of the show holding the season.
    series_id: NumericID,
    /// Video backing the episode.
    video: Video,
}

impl GetSeriesEpisodeResponse {
    /// Return the date of the first airing of the episode, if known.
    #[must_use]
    pub(crate) fn air_date(&self) -> Option<NaiveDate> {
        self.air_date
    }

    /// Return the season-scoped index of the episode.
    #[must_use]
    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    /// Return the name of the episode.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-episode response.
    #[must_use]
    pub(crate) fn new(
        id: u32,
        series_id: NumericID,
        season_index: u32,
        name: String,
        plot: String,
        air_date: Option<NaiveDate>,
        video: Video,
    ) -> Self {
        Self {
            air_date,
            id,
            name,
            plot,
            season_index,
            series_id,
            video,
        }
    }

    /// Return the plot of the episode.
    #[must_use]
    pub(crate) fn plot(&self) -> &str {
        &self.plot
    }

    /// Return the series-scoped index of the season holding the episode.
    #[must_use]
    pub(crate) fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show holding the season.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }

    /// Return the video backing the episode.
    #[must_use]
    pub(crate) fn video(&self) -> &Video {
        &self.video
    }
}

/// Error returned when fetching one episode of a season.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GetSeriesEpisodeError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// The requested episode does not belong to the season.
    #[error("an episode with this index does not exist in the season")]
    NoSuchEpisode,
    /// The requested season does not belong to the show.
    #[error("a season with this index does not exist in the show")]
    NoSuchSeason,
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching one episode of a season.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetSeriesEpisodeUseCase: Send + Sync {
    /// Fetch the requested episode, scoped to its show and season.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn GetSeriesEpisodeUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetSeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetSeriesEpisodeResponse, GetSeriesEpisodeError>>
                + Send
                + 'future,
        >,
    >;
}
