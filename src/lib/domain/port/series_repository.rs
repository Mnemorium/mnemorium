use std::future::Future;

use crate::domain::NumericID;
use crate::domain::model::{Episode, EpisodeIndex, Season, Series};
use crate::domain::port::RepositoryError;

/// Search filters for [`SeriesRepository::search`]. Every field is optional; an
/// all-`None` filter returns every show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct SeriesFilter {
    /// Filter on the show identifier.
    pub id: Option<NumericID>,
    /// Filter on the show name.
    pub name: Option<String>,
}

/// Search filters for [`SeriesRepository::search_episodes`]. Every field is
/// optional; an all-`None` filter returns every episode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct EpisodeFilter {
    /// Filter on the season-scoped episode index.
    pub index: Option<EpisodeIndex>,
    /// Filter on the episode name.
    pub name: Option<String>,
    /// Filter on the video backing the episode.
    pub video_id: Option<NumericID>,
}

/// Port for querying and mutating `Series` with its seasons, their episodes and
/// the videos that back them.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait SeriesRepository: Send + Sync {
    /// Insert a new `series` and its genre links, returning the stored show
    /// with the identifier it was assigned.
    ///
    /// The identifier of `series` is ignored: the repository assigns a fresh
    /// identity. The show is inserted without seasons, and its seasons are
    /// never written by this operation.
    fn create(
        &mut self,
        series: Series,
    ) -> impl Future<Output = Result<Series, RepositoryError>> + Send;

    /// Delete every show matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one show was deleted, and `Ok(false)`
    /// when none matched. An all-`None` filter deletes every show; seasons and
    /// episodes cascade.
    #[expect(
        dead_code,
        reason = "part of the type's declared contract; no caller exercises it yet"
    )]
    fn delete(
        &mut self,
        filter: &SeriesFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Delete every episode matching `filter`.
    ///
    /// The season and the show are retained. Returns `Ok(true)` when at least
    /// one episode was deleted, and `Ok(false)` when none matched. An
    /// all-`None` filter deletes every episode.
    fn delete_episode(
        &mut self,
        filter: &EpisodeFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Persist the metadata and cover of `series` over the stored show carrying
    /// its identifier, replacing its genre links, seasons and episodes.
    ///
    /// The show is inserted when no row carries its identifier.
    fn save(
        &mut self,
        series: Series,
    ) -> impl Future<Output = Result<Series, RepositoryError>> + Send;

    /// Create or replace the episode at `episode.index()` of the season at
    /// `season_index` of `series_id`, replacing its video when a different one
    /// is named.
    ///
    /// Returns the stored episode, or `Ok(None)` when the show or the season
    /// does not exist; a missing target is a valid outcome, not an error. A
    /// video already backing another film or episode is reported as
    /// [`RepositoryError::AlreadyExist`] or [`RepositoryError::Conflict`].
    fn save_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode: Episode,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send;

    /// Create or replace the season at `season.index()` of `series_id`, keeping
    /// the episodes it already sequences.
    ///
    /// Returns the stored season, or `Ok(None)` when no show matches
    /// `series_id`; a missing show is a valid outcome, not an error.
    fn save_season(
        &mut self,
        series_id: NumericID,
        season: Season,
    ) -> impl Future<Output = Result<Option<Season>, RepositoryError>> + Send;

    /// Search the shows matching `filter`, returned as `Vec<Series>`.
    ///
    /// Each show carries its genres, its seasons and their episodes with the
    /// video that backs each episode, so the caller reads the aggregate whole
    /// and never stitches a projection from per-season lookups.
    ///
    /// Returns an empty list when no show matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &SeriesFilter,
    ) -> impl Future<Output = Result<Vec<Series>, RepositoryError>> + Send;

    /// Search the episodes matching `filter`, returned as `Vec<Episode>`.
    ///
    /// Returns an empty list when no episode matches; a missing match is a
    /// valid outcome, not an error.
    fn search_episodes(
        &mut self,
        filter: &EpisodeFilter,
    ) -> impl Future<Output = Result<Vec<Episode>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: SeriesRepository + ?Sized> SeriesRepository for &mut T {
    fn create(
        &mut self,
        series: Series,
    ) -> impl Future<Output = Result<Series, RepositoryError>> + Send {
        (**self).create(series)
    }

    fn delete(
        &mut self,
        filter: &SeriesFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn delete_episode(
        &mut self,
        filter: &EpisodeFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete_episode(filter)
    }

    fn save(
        &mut self,
        series: Series,
    ) -> impl Future<Output = Result<Series, RepositoryError>> + Send {
        (**self).save(series)
    }

    fn save_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode: Episode,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send {
        (**self).save_episode(series_id, season_index, episode)
    }

    fn save_season(
        &mut self,
        series_id: NumericID,
        season: Season,
    ) -> impl Future<Output = Result<Option<Season>, RepositoryError>> + Send {
        (**self).save_season(series_id, season)
    }

    fn search(
        &mut self,
        filter: &SeriesFilter,
    ) -> impl Future<Output = Result<Vec<Series>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn search_episodes(
        &mut self,
        filter: &EpisodeFilter,
    ) -> impl Future<Output = Result<Vec<Episode>, RepositoryError>> + Send {
        (**self).search_episodes(filter)
    }
}
