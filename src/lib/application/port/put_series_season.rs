use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::season::Season;

/// Command to create or replace one season of a show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PutSeriesSeasonCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the image holding the season's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Name of the season, when set.
    name: Option<String>,
    /// Series-scoped index of the season.
    season_index: u32,
    /// Identifier of the show that holds the season.
    series_id: NumericID,
}

impl PutSeriesSeasonCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the image holding the season's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the name of the season, if set.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new put-season command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        series_id: NumericID,
        season_index: u32,
        name: Option<String>,
        cover_image_id: Option<NumericID>,
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

/// Response of a successful season write: the stored season and whether the
/// write created it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct PutSeriesSeasonResponse {
    /// Set when the write created the season, cleared when it replaced one.
    created: bool,
    /// The season that was stored, with the episodes it keeps.
    season: Season,
}

impl PutSeriesSeasonResponse {
    /// Return whether the write created the season.
    #[must_use]
    pub(crate) fn created(&self) -> bool {
        self.created
    }

    /// Create a new written-season response.
    #[must_use]
    pub(crate) fn new(season: Season, created: bool) -> Self {
        Self { created, season }
    }

    /// Return the season that was stored.
    #[must_use]
    pub(crate) fn season(&self) -> &Season {
        &self.season
    }
}

/// Error returned when writing a season.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum PutSeriesSeasonError {
    /// The cover is not a library image.
    #[error("the season cannot be written")]
    Conflict,
    /// The caller may not manage the show.
    #[error("the caller may not manage the library")]
    Forbidden,
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

/// Use case for creating or replacing a show season.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait PutSeriesSeasonUseCase: Send + Sync {
    /// Create or replace the season at the requested index.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn PutSeriesSeasonUseCase>`.
    fn execute<'future>(
        &'future self,
        command: PutSeriesSeasonCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<PutSeriesSeasonResponse, PutSeriesSeasonError>>
                + Send
                + 'future,
        >,
    >;
}
