use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::FieldUpdate;
use crate::domain::NumericID;
use crate::domain::model::Episode;

/// Command to update an episode partially.
///
/// An absent field keeps its stored value; a field the request clears drops an
/// optional value. The backing video is never part of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UpdateSeriesEpisodeCommand {
    /// Replacement air date, when the request names one; `Clear` drops it.
    air_date: FieldUpdate<NaiveDate>,
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Season-scoped index of the episode.
    episode_index: u32,
    /// Replacement name, when the request renames the episode.
    name: Option<String>,
    /// Replacement plot, when the request names it.
    plot: Option<String>,
    /// Series-scoped index of the season holding the episode.
    season_index: u32,
    /// Identifier of the show that holds the season.
    series_id: NumericID,
}

impl UpdateSeriesEpisodeCommand {
    /// Return the replacement air date, if the request names one.
    #[must_use]
    pub(crate) fn air_date(&self) -> FieldUpdate<NaiveDate> {
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

    /// Return the replacement name, if the request renames the episode.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new update-episode command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        series_id: NumericID,
        season_index: u32,
        episode_index: u32,
        name: Option<String>,
        plot: Option<String>,
        air_date: FieldUpdate<NaiveDate>,
    ) -> Self {
        Self {
            air_date,
            caller_id,
            episode_index,
            name,
            plot,
            season_index,
            series_id,
        }
    }

    /// Return the replacement plot, if the request names it.
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
}

/// Response of a successful episode update: the stored episode.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct UpdateSeriesEpisodeResponse {
    /// The episode that was stored.
    episode: Episode,
}

impl UpdateSeriesEpisodeResponse {
    /// Return the episode that was stored.
    #[must_use]
    pub(crate) fn episode(&self) -> &Episode {
        &self.episode
    }

    /// Create a new updated-episode response.
    #[must_use]
    pub(crate) fn new(episode: Episode) -> Self {
        Self { episode }
    }
}

/// Error returned when updating an episode.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum UpdateSeriesEpisodeError {
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
    /// The season holds no episode at the requested index.
    #[error("an episode with this index does not exist in the season")]
    NoSuchEpisode,
    /// The show holds no season at the requested index.
    #[error("a season with this index does not exist in the show")]
    NoSuchSeason,
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for updating a season episode partially.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait UpdateSeriesEpisodeUseCase: Send + Sync {
    /// Update the episode at the requested index.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn UpdateSeriesEpisodeUseCase>`.
    fn execute<'future>(
        &'future self,
        command: UpdateSeriesEpisodeCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateSeriesEpisodeResponse, UpdateSeriesEpisodeError>>
                + Send
                + 'future,
        >,
    >;
}
