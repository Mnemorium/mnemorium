use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;

/// Command to fetch one show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetSeriesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the show to fetch.
    series_id: NumericID,
}

impl GetSeriesCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-show command.
    #[must_use]
    pub fn new(caller_id: NumericID, series_id: NumericID) -> Self {
        Self {
            caller_id,
            series_id,
        }
    }

    /// Return the identifier of the show to fetch.
    #[must_use]
    pub fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// One season summary of a fetched show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetSeriesSeasonItem {
    /// Number of episodes the season holds.
    episode_count: usize,
    /// Series-scoped index of the season.
    index: u32,
    /// Name of the season, when set.
    name: Option<String>,
}

impl GetSeriesSeasonItem {
    /// Return the number of episodes the season holds.
    #[must_use]
    pub fn episode_count(&self) -> usize {
        self.episode_count
    }

    /// Return the series-scoped index of the season.
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Return the name of the season, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new fetched show season.
    #[must_use]
    pub fn new(index: u32, name: Option<String>, episode_count: usize) -> Self {
        Self {
            episode_count,
            index,
            name,
        }
    }
}

/// Response of a successful show lookup: one show with its season summaries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetSeriesResponse {
    /// Country of origin of the show.
    country_of_origin: String,
    /// Date of the first airing of the show.
    first_air_date: NaiveDate,
    /// Genres of the show, ordered by genre identifier.
    genre_ids: Vec<String>,
    /// Unique identifier of the show.
    id: NumericID,
    /// Name of the show.
    name: String,
    /// Plot of the show.
    plot: String,
    /// Seasons of the show, ordered by index.
    seasons: Vec<GetSeriesSeasonItem>,
}

impl GetSeriesResponse {
    /// Return the country of origin of the show.
    #[must_use]
    pub fn country_of_origin(&self) -> &str {
        &self.country_of_origin
    }

    /// Return the date of the first airing.
    #[must_use]
    pub fn first_air_date(&self) -> NaiveDate {
        self.first_air_date
    }

    /// Return the genres of the show, ordered by genre identifier.
    #[must_use]
    pub fn genre_ids(&self) -> &[String] {
        &self.genre_ids
    }

    /// Return the unique identifier of the show.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name of the show.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-show response.
    #[must_use]
    pub fn new(
        id: NumericID,
        name: String,
        plot: String,
        country_of_origin: String,
        first_air_date: NaiveDate,
        genre_ids: Vec<String>,
        seasons: Vec<GetSeriesSeasonItem>,
    ) -> Self {
        Self {
            country_of_origin,
            first_air_date,
            genre_ids,
            id,
            name,
            plot,
            seasons,
        }
    }

    /// Return the plot of the show.
    #[must_use]
    pub fn plot(&self) -> &str {
        &self.plot
    }

    /// Return the seasons of the show, ordered by index.
    #[must_use]
    pub fn seasons(&self) -> &[GetSeriesSeasonItem] {
        &self.seasons
    }
}

/// Error returned when fetching one show.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetSeriesError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching one show.
#[cfg_attr(test, mockall::automock)]
pub trait GetSeriesUseCase: Send + Sync {
    /// Fetch the requested show with its season summaries.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetSeriesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetSeriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetSeriesResponse, GetSeriesError>> + Send + 'future>>;
}
