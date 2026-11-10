use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::NumericID;
use crate::domain::model::Movie;

/// Command to create a film from a video.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CreateMovieCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Country of origin of the film, when known.
    country_of_origin: Option<String>,
    /// Identifier of the image holding the film's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Genres the film is associated with.
    genres: Vec<String>,
    /// Name of the film.
    name: String,
    /// Plot of the film, when known.
    plot: Option<String>,
    /// Date of the release of the film, when known.
    release_date: Option<NaiveDate>,
    /// Video backing the film.
    video_id: NumericID,
}

impl CreateMovieCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the country of origin, if known.
    #[must_use]
    pub(crate) fn country_of_origin(&self) -> Option<&str> {
        self.country_of_origin.as_deref()
    }

    /// Return the identifier of the image holding the film's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the genres the film is associated with.
    #[must_use]
    pub(crate) fn genres(&self) -> &[String] {
        &self.genres
    }

    /// Return the name of the film.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new create-film command.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the film's write fields one-to-one"
    )]
    pub(crate) fn new(
        caller_id: NumericID,
        video_id: NumericID,
        name: String,
        plot: Option<String>,
        country_of_origin: Option<String>,
        release_date: Option<NaiveDate>,
        genres: Vec<String>,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            caller_id,
            country_of_origin,
            cover_image_id,
            genres,
            name,
            plot,
            release_date,
            video_id,
        }
    }

    /// Return the plot, if known.
    #[must_use]
    pub(crate) fn plot(&self) -> Option<&str> {
        self.plot.as_deref()
    }

    /// Return the date of the release, if known.
    #[must_use]
    pub(crate) fn release_date(&self) -> Option<NaiveDate> {
        self.release_date
    }

    /// Return the video backing the film.
    #[must_use]
    pub(crate) fn video_id(&self) -> NumericID {
        self.video_id
    }
}

/// Response of a successful film creation: the stored film and whether the
/// write created it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct CreateMovieResponse {
    /// Set when the film was created, cleared when it already existed.
    created: bool,
    /// The film that was stored.
    movie: Movie,
}

impl CreateMovieResponse {
    /// Return whether the write created the film.
    #[must_use]
    pub(crate) fn created(&self) -> bool {
        self.created
    }

    /// Return the film that was stored.
    #[must_use]
    pub(crate) fn movie(&self) -> &Movie {
        &self.movie
    }

    /// Create a new created-film response.
    #[must_use]
    pub(crate) fn new(movie: Movie, created: bool) -> Self {
        Self { created, movie }
    }
}

/// Error returned when creating a film.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CreateMovieError {
    /// The video is already attached, or the cover is not a library image.
    #[error("the film cannot be created")]
    Conflict,
    /// The caller may not manage the library catalogue.
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
    /// No video matches the requested identifier.
    #[error("a video with this identifier does not exist")]
    NoSuchVideo,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for creating a film from a video.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait CreateMovieUseCase: Send + Sync {
    /// Create the film, returning the existing one when the video already backs
    /// a film.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn CreateMovieUseCase>`.
    fn execute<'future>(
        &'future self,
        command: CreateMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<CreateMovieResponse, CreateMovieError>> + Send + 'future>>;
}
