use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDate;

use crate::application::FieldUpdate;
use crate::domain::NumericID;
use crate::domain::model::Series;

/// Command to update a show partially.
///
/// An absent field keeps its stored value; a field the request clears drops an
/// optional value. The show's seasons and episodes are never part of the
/// command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UpdateSeriesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Replacement country of origin, when the request names one; `Clear` drops
    /// it.
    country_of_origin: FieldUpdate<String>,
    /// Replacement cover image, when the request names one; `Clear` drops it.
    cover_image_id: FieldUpdate<NumericID>,
    /// Replacement first-air date, when the request names one; `Clear` drops it.
    first_air_date: FieldUpdate<NaiveDate>,
    /// Replacement genres, when the request names them.
    genres: Option<Vec<String>>,
    /// Replacement name, when the request renames the show.
    name: Option<String>,
    /// Replacement plot, when the request names it.
    plot: Option<String>,
    /// Identifier of the show to update.
    series_id: NumericID,
}

impl UpdateSeriesCommand {
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

    /// Return the replacement first-air date, if the request names one.
    #[must_use]
    pub(crate) fn first_air_date(&self) -> FieldUpdate<NaiveDate> {
        self.first_air_date
    }

    /// Return the replacement genres, if the request names them.
    #[must_use]
    pub(crate) fn genres(&self) -> Option<&[String]> {
        self.genres.as_deref()
    }

    /// Return the replacement name, if the request renames the show.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new update-show command.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the show's write fields one-to-one"
    )]
    pub(crate) fn new(
        caller_id: NumericID,
        series_id: NumericID,
        name: Option<String>,
        plot: Option<String>,
        country_of_origin: FieldUpdate<String>,
        first_air_date: FieldUpdate<NaiveDate>,
        genres: Option<Vec<String>>,
        cover_image_id: FieldUpdate<NumericID>,
    ) -> Self {
        Self {
            caller_id,
            country_of_origin,
            cover_image_id,
            first_air_date,
            genres,
            name,
            plot,
            series_id,
        }
    }

    /// Return the replacement plot, if the request names it.
    #[must_use]
    pub(crate) fn plot(&self) -> Option<&str> {
        self.plot.as_deref()
    }

    /// Return the identifier of the show to update.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Response of a successful show update: the stored show.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct UpdateSeriesResponse {
    /// The show that was stored.
    series: Series,
}

impl UpdateSeriesResponse {
    /// Create a new updated-show response.
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

/// Error returned when updating a show.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum UpdateSeriesError {
    /// The cover is not a library image.
    #[error("the show cannot be updated")]
    Conflict,
    /// The caller may not manage the show.
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
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for updating a show partially.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait UpdateSeriesUseCase: Send + Sync {
    /// Update the requested show.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn UpdateSeriesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: UpdateSeriesCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<UpdateSeriesResponse, UpdateSeriesError>> + Send + 'future>,
    >;
}
