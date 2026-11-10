use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::model::{ExternalRef, Series};

/// Command to apply external metadata to a show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ApplySeriesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Reference to the provider's work.
    reference: ExternalRef,
    /// Identifier of the show to write.
    series_id: NumericID,
}

impl ApplySeriesCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new apply-show command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, series_id: NumericID, reference: ExternalRef) -> Self {
        Self {
            caller_id,
            reference,
            series_id,
        }
    }

    /// Return the reference to the provider's work.
    #[must_use]
    pub(crate) fn reference(&self) -> &ExternalRef {
        &self.reference
    }

    /// Return the identifier of the show to write.
    #[must_use]
    pub(crate) fn series_id(&self) -> NumericID {
        self.series_id
    }
}

/// Response of a successful show apply: the stored show.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct ApplySeriesResponse {
    /// The show that was stored.
    series: Series,
}

impl ApplySeriesResponse {
    /// Create a new applied-show response.
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

/// Error returned when applying external metadata to a show.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ApplySeriesError {
    /// The write conflicts with the stored catalogue.
    #[error("the applied metadata conflicts with the stored catalogue")]
    Conflict,
    /// The caller may not manage the show.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The provider reports a work the show apply cannot write.
    #[error("the provider reports a work that cannot be applied to a show")]
    InvalidReference,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No show matches the requested identifier.
    #[error("a show with this identifier does not exist")]
    NoSuchSeries,
    /// The provider failed while resolving the reference.
    #[error("the video metadata provider failed")]
    ProviderFailure,
    /// No video metadata provider is configured.
    #[error("no video metadata provider is configured")]
    ProviderUnavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The provider does not know the reference.
    #[error("the video metadata provider does not know the reference")]
    UnknownReference,
}

/// Use case for applying external metadata to a show.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ApplySeriesUseCase: Send + Sync {
    /// Resolve the reference and write the show.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn ApplySeriesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ApplySeriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ApplySeriesResponse, ApplySeriesError>> + Send + 'future>>;
}
