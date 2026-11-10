use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::NumericID;
use crate::domain::model::Video;

/// Command to list the films the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ListMoviesCommand {
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
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the maximum number of films to return, if any.
    #[must_use]
    pub(crate) fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-films command.
    #[must_use]
    pub(crate) fn new(
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
    pub(crate) fn offset(&self) -> Option<usize> {
        self.offset
    }
}

/// One film of a successful listing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct ListMoviesItem {
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

impl ListMoviesItem {
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

    /// Create a new listed film.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the item's fields one-to-one"
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

/// Response of a successful film listing: one page of films.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct ListMoviesResponse {
    /// The films of the returned page.
    movies: Vec<ListMoviesItem>,
    /// Total number of films, before pagination.
    total: usize,
}

impl ListMoviesResponse {
    /// Return the films of the returned page.
    #[must_use]
    pub(crate) fn movies(&self) -> &[ListMoviesItem] {
        &self.movies
    }

    /// Create a new list-films response.
    #[must_use]
    pub(crate) fn new(movies: Vec<ListMoviesItem>, total: usize) -> Self {
        Self { movies, total }
    }

    /// Return the total number of films, before pagination.
    #[must_use]
    pub(crate) fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing films.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ListMoviesError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the films the caller may see.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ListMoviesUseCase: Send + Sync {
    /// List the films visible to the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn ListMoviesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ListMoviesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListMoviesResponse, ListMoviesError>> + Send + 'future>>;
}
