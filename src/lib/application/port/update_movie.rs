use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::FieldUpdate;
use crate::domain::NumericID;
use crate::domain::model::Movie;

/// Command to update a film partially.
///
/// An absent field keeps its stored value; a field the request clears drops an
/// optional value. The backing video is never part of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UpdateMovieCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Replacement country of origin, when the request names one; `Clear` drops
    /// it.
    country_of_origin: FieldUpdate<String>,
    /// Replacement cover image, when the request names one; `Clear` drops it.
    cover_image_id: FieldUpdate<NumericID>,
    /// Replacement genres, when the request names them.
    genres: Option<Vec<String>>,
    /// Identifier of the film to update.
    movie_id: NumericID,
    /// Replacement name, when the request renames the film.
    name: Option<String>,
    /// Replacement plot, when the request names it.
    plot: Option<String>,
    /// Replacement release date, when the request names one; `Clear` drops it.
    release_date: FieldUpdate<NaiveDate>,
}

impl UpdateMovieCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the replacement country of origin, if the request names one.
    #[must_use]
    pub(crate) fn country_of_origin(&self) -> &FieldUpdate<String> {
        &self.country_of_origin
    }

    /// Return the replacement cover image, if the request names one.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> FieldUpdate<NumericID> {
        self.cover_image_id
    }

    /// Return the replacement genres, if the request names them.
    #[must_use]
    pub(crate) fn genres(&self) -> Option<&[String]> {
        self.genres.as_deref()
    }

    /// Return the identifier of the film to update.
    #[must_use]
    pub(crate) fn movie_id(&self) -> NumericID {
        self.movie_id
    }

    /// Return the replacement name, if the request renames the film.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new update-film command.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the film's write fields one-to-one"
    )]
    pub(crate) fn new(
        caller_id: NumericID,
        movie_id: NumericID,
        name: Option<String>,
        plot: Option<String>,
        country_of_origin: FieldUpdate<String>,
        release_date: FieldUpdate<NaiveDate>,
        genres: Option<Vec<String>>,
        cover_image_id: FieldUpdate<NumericID>,
    ) -> Self {
        Self {
            caller_id,
            country_of_origin,
            cover_image_id,
            genres,
            movie_id,
            name,
            plot,
            release_date,
        }
    }

    /// Return the replacement plot, if the request names it.
    #[must_use]
    pub(crate) fn plot(&self) -> Option<&str> {
        self.plot.as_deref()
    }

    /// Return the replacement release date, if the request names one.
    #[must_use]
    pub(crate) fn release_date(&self) -> FieldUpdate<NaiveDate> {
        self.release_date
    }
}

/// Response of a successful film update: the stored film.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct UpdateMovieResponse {
    /// The film that was stored.
    movie: Movie,
}

impl UpdateMovieResponse {
    /// Return the film that was stored.
    #[must_use]
    pub(crate) fn movie(&self) -> &Movie {
        &self.movie
    }

    /// Create a new updated-film response.
    #[must_use]
    pub(crate) fn new(movie: Movie) -> Self {
        Self { movie }
    }
}

/// Error returned when updating a film.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum UpdateMovieError {
    /// The cover is not a library image.
    #[error("the film cannot be updated")]
    Conflict,
    /// The caller may not manage the film.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The country of origin is not a two-letter code.
    #[error("the country of origin must be an ISO 3166-1 alpha-2 code")]
    InvalidCountry,
    /// A genre identifier is empty.
    #[error("a genre identifier must not be empty")]
    InvalidGenre,
    /// The name is empty or made only of whitespace.
    #[error("the name must not be empty")]
    InvalidName,
    /// The name is longer than the name policy allows.
    #[error("the name is too long")]
    NameTooLong,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No film matches the requested identifier.
    #[error("a film with this identifier does not exist")]
    NoSuchMovie,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for updating a film partially.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait UpdateMovieUseCase: Send + Sync {
    /// Update the requested film.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn UpdateMovieUseCase>`.
    fn execute<'future>(
        &'future self,
        command: UpdateMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<UpdateMovieResponse, UpdateMovieError>> + Send + 'future>>;
}
