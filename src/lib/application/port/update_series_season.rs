use std::future::Future;
use std::pin::Pin;

use crate::application::FieldUpdate;
use crate::domain::NumericID;
use crate::domain::model::Season;

/// Command to update a season partially.
///
/// An absent field keeps its stored value; a field the request clears drops an
/// optional value. The season's episodes are never part of the command.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UpdateSeriesSeasonCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Replacement cover image, when the request names one; `Clear` drops it.
    cover_image_id: FieldUpdate<NumericID>,
    /// Replacement name, when the request names one; `Clear` drops it.
    name: FieldUpdate<String>,
    /// Series-scoped index of the season.
    season_index: u32,
    /// Identifier of the show that holds the season.
    series_id: NumericID,
}

impl UpdateSeriesSeasonCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the replacement cover image, if the request names one.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> FieldUpdate<NumericID> {
        self.cover_image_id
    }

    /// Return the replacement name, if the request names one.
    #[must_use]
    pub(crate) fn name(&self) -> FieldUpdate<String> {
        self.name.clone()
    }

    /// Create a new update-season command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        series_id: NumericID,
        season_index: u32,
        name: FieldUpdate<String>,
        cover_image_id: FieldUpdate<NumericID>,
    ) -> Self {
        Self {
            caller_id,
            cover_image_id,
            name,
            season_index,
            series_id,
        }
    }

    /// Return the series-scoped index of the season.
    #[must_use]
    pub(crate) fn season_index(&self) -> u32 {
        self.season_index
    }

    /// Return the identifier of the show that holds the season.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Response of a successful season update: the stored season.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct UpdateSeriesSeasonResponse {
    /// The season that was stored, with the episodes it keeps.
    season: Season,
}

impl UpdateSeriesSeasonResponse {
    /// Create a new updated-season response.
    #[must_use]
    pub(crate) fn new(season: Season) -> Self {
        Self { season }
    }

    /// Return the season that was stored.
    #[must_use]
    pub(crate) fn season(&self) -> &Season {
        &self.season
    }
}

/// Error returned when updating a season.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum UpdateSeriesSeasonError {
    /// The cover is not a library image.
    #[error("the season cannot be updated")]
    Conflict,
    /// The caller may not manage the show.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// The show holds no season at the requested index.
    #[error("a season with this index does not exist in the show")]
    NoSuchSeason,
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for updating a show season partially.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait UpdateSeriesSeasonUseCase: Send + Sync {
    /// Update the season at the requested index.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn UpdateSeriesSeasonUseCase>`.
    fn execute<'future>(
        &'future self,
        command: UpdateSeriesSeasonCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateSeriesSeasonResponse, UpdateSeriesSeasonError>>
                + Send
                + 'future,
        >,
    >;
}
