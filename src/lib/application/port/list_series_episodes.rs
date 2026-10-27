use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::port::video_summary::VideoSummary;
use crate::domain::alias::NumericID;

/// Command to list the episodes of one season.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListSeriesEpisodesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Series-scoped index of the season holding the episodes.
    season_index: u32,
    /// Identifier of the show holding the season.
    series_id: NumericID,
}

impl ListSeriesEpisodesCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new list-episodes command.
    #[must_use]
    pub fn new(caller_id: NumericID, series_id: NumericID, season_index: u32) -> Self {
        Self {
            caller_id,
            season_index,
            series_id,
        }
    }

    /// Return the series-scoped index of the season holding the episodes.
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

/// One episode of a listed season.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ListSeriesEpisodesItem {
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

impl ListSeriesEpisodesItem {
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

    /// Create a new listed episode.
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

/// Response of a successful episode listing: every episode of the season.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ListSeriesEpisodesResponse {
    /// The episodes of the season, ordered by index.
    episodes: Vec<ListSeriesEpisodesItem>,
}

impl ListSeriesEpisodesResponse {
    /// Return the episodes of the season, ordered by index.
    #[must_use]
    pub fn episodes(&self) -> &[ListSeriesEpisodesItem] {
        &self.episodes
    }

    /// Create a new list-episodes response.
    #[must_use]
    pub fn new(episodes: Vec<ListSeriesEpisodesItem>) -> Self {
        Self { episodes }
    }
}

/// Error returned when listing the episodes of a season.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListSeriesEpisodesError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
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

/// Use case for listing the episodes of one season.
#[cfg_attr(test, mockall::automock)]
pub trait ListSeriesEpisodesUseCase: Send + Sync {
    /// List the episodes of the requested season, scoped to its show.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn ListSeriesEpisodesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ListSeriesEpisodesCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListSeriesEpisodesResponse, ListSeriesEpisodesError>>
                + Send
                + 'future,
        >,
    >;
}
