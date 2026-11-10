use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::domain::NumericID;
use crate::domain::model::Series;

/// Command to create a show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CreateSeriesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Country of origin of the show, when known.
    country_of_origin: Option<String>,
    /// Identifier of the image holding the show's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Date of the first airing of the show, when known.
    first_air_date: Option<NaiveDate>,
    /// Genres the show is associated with.
    genres: Vec<String>,
    /// Name of the show.
    name: String,
    /// Plot of the show, when known.
    plot: Option<String>,
}

impl CreateSeriesCommand {
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

    /// Return the identifier of the image holding the show's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the date of the first airing, if known.
    #[must_use]
    pub(crate) fn first_air_date(&self) -> Option<NaiveDate> {
        self.first_air_date
    }

    /// Return the genres the show is associated with.
    #[must_use]
    pub(crate) fn genres(&self) -> &[String] {
        &self.genres
    }

    /// Return the name of the show.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new create-show command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        name: String,
        plot: Option<String>,
        country_of_origin: Option<String>,
        first_air_date: Option<NaiveDate>,
        genres: Vec<String>,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            caller_id,
            country_of_origin,
            cover_image_id,
            first_air_date,
            genres,
            name,
            plot,
        }
    }

    /// Return the plot, if known.
    #[must_use]
    pub(crate) fn plot(&self) -> Option<&str> {
        self.plot.as_deref()
    }
}

/// Response of a successful show creation: the stored show.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct CreateSeriesResponse {
    /// The show that was stored.
    series: Series,
}

impl CreateSeriesResponse {
    /// Create a new created-show response.
    #[must_use]
    pub(crate) fn new(series: Series) -> Self {
        Self { series }
    }

    /// Return the show that was stored.
    #[must_use]
    pub(crate) fn series(&self) -> &Series {
        &self.series
    }
}

/// Error returned when creating a show.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CreateSeriesError {
    /// The cover is not a library image.
    #[error("the show cannot be created")]
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
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for creating a show.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait CreateSeriesUseCase: Send + Sync {
    /// Create the show, which always starts without seasons.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn CreateSeriesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: CreateSeriesCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<CreateSeriesResponse, CreateSeriesError>> + Send + 'future>,
    >;
}
