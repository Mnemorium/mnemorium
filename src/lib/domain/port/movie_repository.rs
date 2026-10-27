use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::movie::Movie;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MovieRepository::search`]. Every field is optional; an
/// all-`None` filter returns every film.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MovieFilter {
    /// Filter on the film identifier.
    pub id: Option<NumericID>,
    /// Filter on the film name.
    pub name: Option<String>,
}

/// Port for querying `Movie` with the video that backs it.
#[cfg_attr(test, mockall::automock)]
pub trait MovieRepository: Send + Sync {
    /// Search the films matching `filter`, returned as `Vec<Movie>`.
    ///
    /// Each film carries its genres and the video that backs it, so the caller
    /// reads the aggregate whole and never stitches a projection from per-film
    /// lookups.
    ///
    /// Returns an empty list when no film matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &MovieFilter,
    ) -> impl Future<Output = Result<Vec<Movie>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MovieRepository + ?Sized> MovieRepository for &mut T {
    fn search(
        &mut self,
        filter: &MovieFilter,
    ) -> impl Future<Output = Result<Vec<Movie>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
