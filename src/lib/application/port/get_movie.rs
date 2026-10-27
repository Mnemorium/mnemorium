use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::port::video_summary::VideoSummary;
use crate::domain::alias::NumericID;

/// Command to fetch one film.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetMovieCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the film to fetch.
    movie_id: NumericID,
}

impl GetMovieCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the film to fetch.
    #[must_use]
    pub fn movie_id(&self) -> NumericID {
        self.movie_id
    }

    /// Create a new fetch-film command.
    #[must_use]
    pub fn new(caller_id: NumericID, movie_id: NumericID) -> Self {
        Self {
            caller_id,
            movie_id,
        }
    }
}

/// Response of a successful film lookup: one film.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GetMovieResponse {
    /// Country of origin of the film.
    country_of_origin: String,
    /// Genres of the film, ordered by genre identifier.
    genre_ids: Vec<String>,
    /// Unique identifier of the film.
    id: NumericID,
    /// Name of the film.
    name: String,
    /// Plot of the film.
    plot: String,
    /// Date of the release of the film.
    release_date: NaiveDate,
    /// Video backing the film.
    video: VideoSummary,
}

impl GetMovieResponse {
    /// Return the country of origin of the film.
    #[must_use]
    pub fn country_of_origin(&self) -> &str {
        &self.country_of_origin
    }

    /// Return the genres of the film, ordered by genre identifier.
    #[must_use]
    pub fn genre_ids(&self) -> &[String] {
        &self.genre_ids
    }

    /// Return the unique identifier of the film.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name of the film.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetch-film response.
    #[must_use]
    pub fn new(
        id: NumericID,
        name: String,
        plot: String,
        country_of_origin: String,
        release_date: NaiveDate,
        genre_ids: Vec<String>,
        video: VideoSummary,
    ) -> Self {
        Self {
            country_of_origin,
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
    pub fn plot(&self) -> &str {
        &self.plot
    }

    /// Return the date of the release of the film.
    #[must_use]
    pub fn release_date(&self) -> NaiveDate {
        self.release_date
    }

    /// Return the video backing the film.
    #[must_use]
    pub fn video(&self) -> &VideoSummary {
        &self.video
    }
}

/// Error returned when fetching one film.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetMovieError {
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
pub trait GetMovieUseCase: Send + Sync {
    /// Fetch the requested film.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetMovieUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetMovieResponse, GetMovieError>> + Send + 'future>>;
}
