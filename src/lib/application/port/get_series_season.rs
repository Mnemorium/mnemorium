use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to fetch one season of a show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetSeriesSeasonCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Series-scoped index of the season to fetch.
    season_index: u32,
    /// Identifier of the show holding the season.
    series_id: NumericID,
}

impl GetSeriesSeasonCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-season command.
    #[must_use]
    pub fn new(caller_id: NumericID, series_id: NumericID, season_index: u32) -> Self {
        Self {
            caller_id,
            season_index,
            series_id,
        }
    }

    /// Return the series-scoped index of the season to fetch.
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

/// Response of a successful season lookup: one season summary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetSeriesSeasonResponse {
    /// Identifier of the file holding the season's artwork, when set.
    cover_file_id: Option<NumericID>,
    /// Number of episodes the season holds.
    episode_count: usize,
    /// Series-scoped index of the season.
    index: u32,
    /// Name of the season, when set.
    name: Option<String>,
    /// Identifier of the show holding the season.
    series_id: NumericID,
}

impl GetSeriesSeasonResponse {
    /// Return the identifier of the file holding the season's artwork, if any.
    #[must_use]
    pub fn cover_file_id(&self) -> Option<NumericID> {
        self.cover_file_id
    }

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

    /// Create a new fetch-season response.
    #[must_use]
    pub fn new(
        series_id: NumericID,
        index: u32,
        name: Option<String>,
        episode_count: usize,
        cover_file_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover_file_id,
            episode_count,
            index,
            name,
            series_id,
        }
    }

    /// Return the identifier of the show holding the season.
    #[must_use]
    pub fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Error returned when fetching one season of a show.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetSeriesSeasonError {
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

/// Use case for fetching one season of a show.
#[cfg_attr(test, mockall::automock)]
pub trait GetSeriesSeasonUseCase: Send + Sync {
    /// Fetch the requested season, scoped to its show.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetSeriesSeasonUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetSeriesSeasonCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetSeriesSeasonResponse, GetSeriesSeasonError>>
                + Send
                + 'future,
        >,
    >;
}
