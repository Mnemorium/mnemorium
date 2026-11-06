use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::episode::Episode;
use crate::domain::model::season::Season;
use crate::domain::model::series::Series;
use crate::domain::port::error::RepositoryError;

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

    /// Return the episode backed by `video_id`, if any.
    ///
    /// A video backs at most one film or episode; a video the catalogue does
    /// not attach has no episode. A missing attachment is `Ok(None)`, not an
    /// error.
    fn find_episode_by_video(
        &mut self,
        video_id: NumericID,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send;

    /// Create or replace the episode at `episode.index()` of the season at
    /// `season_index` of `series_id`, replacing its video when a different one
    /// is named.
    ///
    /// Returns the stored episode, or `Ok(None)` when the show or the season
    /// does not exist; a missing target is a valid outcome, not an error. A
    /// video already backing another film or episode is reported as
    /// [`RepositoryError::AlreadyExist`] or [`RepositoryError::Conflict`].
    fn put_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode: Episode,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send;
    /// Create or replace the season at `season.index()` of `series_id`,
    /// keeping the episodes it already sequences.
    ///
    /// Returns the stored season, or `Ok(None)` when no show matches
    /// `series_id`; a missing show is a valid outcome, not an error.
    fn put_season(
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

    /// Persist the metadata and cover of `series` over the stored show carrying
    /// its identifier, replacing its genre links.
    ///
    /// Returns `Ok(None)` when no show matches; a missing show is a valid
    /// outcome, not an error. The show's seasons and episodes are untouched.
    fn update(
        &mut self,
        series: Series,
    ) -> impl Future<Output = Result<Option<Series>, RepositoryError>> + Send;

    /// Persist the name, plot and air date of `episode` over the stored episode
    /// at its index of the addressed season.
    ///
    /// The backing video is never rewritten. Returns `Ok(None)` when the show,
    /// the season or the episode does not exist; a missing target is a valid
    /// outcome, not an error.
    fn update_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode: Episode,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send;
    /// Persist the name and cover of `season` over the stored season at its
    /// index of `series_id`.
    ///
    /// Returns `Ok(None)` when the show does not exist or holds no season at
    /// that index; a missing target is a valid outcome, not an error. The
    /// season's episodes are untouched.
    fn update_season(
        &mut self,
        series_id: NumericID,
        season: Season,
    ) -> impl Future<Output = Result<Option<Season>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: SeriesRepository + ?Sized> SeriesRepository for &mut T {
    fn create(
        &mut self,
        series: Series,
    ) -> impl Future<Output = Result<Series, RepositoryError>> + Send {
        (**self).create(series)
    }

    fn find_episode_by_video(
        &mut self,
        video_id: NumericID,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send {
        (**self).find_episode_by_video(video_id)
    }

    fn put_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode: Episode,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send {
        (**self).put_episode(series_id, season_index, episode)
    }
    fn put_season(
        &mut self,
        series_id: NumericID,
        season: Season,
    ) -> impl Future<Output = Result<Option<Season>, RepositoryError>> + Send {
        (**self).put_season(series_id, season)
    }

    fn search(
        &mut self,
        filter: &SeriesFilter,
    ) -> impl Future<Output = Result<Vec<Series>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn update(
        &mut self,
        series: Series,
    ) -> impl Future<Output = Result<Option<Series>, RepositoryError>> + Send {
        (**self).update(series)
    }

    fn update_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode: Episode,
    ) -> impl Future<Output = Result<Option<Episode>, RepositoryError>> + Send {
        (**self).update_episode(series_id, season_index, episode)
    }
    fn update_season(
        &mut self,
        series_id: NumericID,
        season: Season,
    ) -> impl Future<Output = Result<Option<Season>, RepositoryError>> + Send {
        (**self).update_season(series_id, season)
    }
}
