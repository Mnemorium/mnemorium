use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::alias::NumericID;
use crate::domain::model::video::Video;

/// Command to fetch one film.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetMovieCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the film to fetch.
    movie_id: NumericID,
}

impl GetMovieCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the film to fetch.
    #[must_use]
    pub(crate) fn movie_id(&self) -> NumericID {
        self.movie_id
    }

    /// Create a new fetch-film command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, movie_id: NumericID) -> Self {
        Self {
            caller_id,
            movie_id,
        }
    }
}

/// Response of a successful film lookup: one film.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct GetMovieResponse {
    /// Country of origin of the film, when known.
    country_of_origin: Option<String>,
    /// Identifier of the image holding the film's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Genres of the film, ordered by genre identifier.
    genre_ids: Vec<String>,
    /// Unique identifier of the film.
    id: NumericID,
    /// Name of the film.
    name: String,
    /// Plot of the film.
    plot: String,
    /// Date of the release of the film, when known.
    release_date: Option<NaiveDate>,
    /// Video backing the film.
    video: Video,
}

impl GetMovieResponse {
    /// Return the country of origin of the film, if known.
    #[must_use]
    pub(crate) fn country_of_origin(&self) -> Option<&str> {
        self.country_of_origin.as_deref()
    }

    /// Return the identifier of the image holding the film's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the genres of the film, ordered by genre identifier.
    #[must_use]
    pub(crate) fn genre_ids(&self) -> &[String] {
        &self.genre_ids
    }

    /// Return the unique identifier of the film.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name of the film.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-film response.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the response's fields one-to-one"
    )]
    pub(crate) fn new(
        id: NumericID,
        name: String,
        plot: String,
        country_of_origin: Option<String>,
        release_date: Option<NaiveDate>,
        genre_ids: Vec<String>,
        cover_image_id: Option<NumericID>,
        video: Video,
    ) -> Self {
        Self {
            country_of_origin,
            cover_image_id,
            genre_ids,
            id,
            name,
            plot,
            release_date,
            video,
        }
    }

    /// Return the plot of the film.
    #[must_use]
    pub(crate) fn plot(&self) -> &str {
        &self.plot
    }

    /// Return the date of the release of the film, if known.
    #[must_use]
    pub(crate) fn release_date(&self) -> Option<NaiveDate> {
        self.release_date
    }

    /// Return the video backing the film.
    #[must_use]
    pub(crate) fn video(&self) -> &Video {
        &self.video
    }
}

/// Error returned when fetching one film.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GetMovieError {
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

/// Use case for fetching one film.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetMovieUseCase: Send + Sync {
    /// Fetch the requested film.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetMovieUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetMovieResponse, GetMovieError>> + Send + 'future>>;
}
