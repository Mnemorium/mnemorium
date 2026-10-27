use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::series::Series;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`SeriesRepository::search`]. Every field is optional; an
/// all-`None` filter returns every show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SeriesFilter {
    /// Filter on the show identifier.
    pub id: Option<NumericID>,
    /// Filter on the show name.
    pub name: Option<String>,
}

/// Port for querying `Series` with its seasons, their episodes and the videos
/// that back them.
#[cfg_attr(test, mockall::automock)]
pub trait SeriesRepository: Send + Sync {
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
}

#[cfg(test)]
impl<T: SeriesRepository + ?Sized> SeriesRepository for &mut T {
    fn search(
        &mut self,
        filter: &SeriesFilter,
    ) -> impl Future<Output = Result<Vec<Series>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
