use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::movie::Movie;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MovieRepository::search`]. Every field is optional; an
/// all-`None` filter returns every film.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MovieFilter {
    /// Filter on the film identifier.
    pub id: Option<NumericID>,
    /// Filter on the film name.
    pub name: Option<String>,
    /// Filter on the video backing the film.
    pub video_id: Option<NumericID>,
}

/// Port for querying and mutating `Movie` with the video that backs it.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait MovieRepository: Send + Sync {
    /// Insert a new `movie` and its genre links, returning the stored film with
    /// the identifier it was assigned.
    ///
    /// The identifier of `movie` is ignored: the repository assigns a fresh
    /// identity. A video already backing another film is reported as
    /// [`RepositoryError::AlreadyExist`]; a video backing an episode, or a
    /// cover image the library does not own, is reported as
    /// [`RepositoryError::Conflict`].
    fn create(
        &mut self,
        movie: Movie,
    ) -> impl Future<Output = Result<Movie, RepositoryError>> + Send;

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

    /// Persist the metadata and cover of `movie` over the stored film carrying
    /// its identifier, replacing its genre links.
    ///
    /// The backing video is never rewritten. Returns `Ok(None)` when no film
    /// matches; a missing film is a valid outcome, not an error.
    fn update(
        &mut self,
        movie: Movie,
    ) -> impl Future<Output = Result<Option<Movie>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MovieRepository + ?Sized> MovieRepository for &mut T {
    fn create(
        &mut self,
        movie: Movie,
    ) -> impl Future<Output = Result<Movie, RepositoryError>> + Send {
        (**self).create(movie)
    }

    fn search(
        &mut self,
        filter: &MovieFilter,
    ) -> impl Future<Output = Result<Vec<Movie>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn update(
        &mut self,
        movie: Movie,
    ) -> impl Future<Output = Result<Option<Movie>, RepositoryError>> + Send {
        (**self).update(movie)
    }
}
