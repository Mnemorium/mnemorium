use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::port::video_summary::VideoSummary;
use crate::domain::alias::NumericID;

/// Command to list the films the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListMoviesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Maximum number of films to return, when set.
    limit: Option<usize>,
    /// Only return films whose name matches this value, when set.
    name: Option<String>,
    /// Number of matching films to skip before returning the page.
    offset: Option<usize>,
}

impl ListMoviesCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the maximum number of films to return, if any.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-films command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        name: Option<String>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Self {
        Self {
            caller_id,
            limit,
            name,
            offset,
        }
    }

    /// Return the number of matching films to skip, if any.
    #[must_use]
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }
}

/// One film of a successful listing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ListMoviesItem {
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

impl ListMoviesItem {
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

    /// Create a new listed film.
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

/// Response of a successful film listing: one page of films.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ListMoviesResponse {
    /// The films of the returned page.
    movies: Vec<ListMoviesItem>,
    /// Total number of films, before pagination.
    total: usize,
}

impl ListMoviesResponse {
    /// Return the films of the returned page.
    #[must_use]
    pub fn movies(&self) -> &[ListMoviesItem] {
        &self.movies
    }

    /// Create a new list-films response.
    #[must_use]
    pub fn new(movies: Vec<ListMoviesItem>, total: usize) -> Self {
        Self { movies, total }
    }

    /// Return the total number of films, before pagination.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing films.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListMoviesError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the films the caller may see.
#[cfg_attr(test, mockall::automock)]
pub trait ListMoviesUseCase: Send + Sync {
    /// List the films visible to the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn ListMoviesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ListMoviesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListMoviesResponse, ListMoviesError>> + Send + 'future>>;
}
