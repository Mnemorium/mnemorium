use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::port::video_summary::VideoSummary;
use crate::domain::alias::NumericID;

/// Command to fetch one episode of a season.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetSeriesEpisodeCommand {
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
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the season-scoped index of the episode to fetch.
    #[must_use]
    pub fn episode_index(&self) -> u32 {
        self.episode_index
    }

    /// Create a new fetch-episode command.
    #[must_use]
    pub fn new(
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
    pub fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show holding the season.
    #[must_use]
    pub fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Response of a successful episode lookup: one episode.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GetSeriesEpisodeResponse {
    /// Date of the first airing of the episode.
    air_date: NaiveDate,
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
    video: VideoSummary,
}

impl GetSeriesEpisodeResponse {
    /// Return the date of the first airing of the episode.
    #[must_use]
    pub fn air_date(&self) -> NaiveDate {
        self.air_date
    }

    /// Return the season-scoped index of the episode.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Return the name of the episode.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-episode response.
    #[must_use]
    pub fn new(
        id: u32,
        series_id: NumericID,
        season_index: u32,
        name: String,
        plot: String,
        air_date: NaiveDate,
        video: VideoSummary,
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
    pub fn plot(&self) -> &str {
        &self.plot
    }

    /// Return the series-scoped index of the season holding the episode.
    #[must_use]
    pub fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show holding the season.
    #[must_use]
    pub fn series_id(&self) -> NumericID {
        self.series_id
    }

    /// Return the video backing the episode.
    #[must_use]
    pub fn video(&self) -> &VideoSummary {
        &self.video
    }
}

/// Error returned when fetching one episode of a season.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetSeriesEpisodeError {
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
pub trait GetSeriesEpisodeUseCase: Send + Sync {
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
