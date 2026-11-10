use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::model::{ExternalRef, Movie};

/// Command to apply external metadata to a film.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ApplyMovieCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the film to write.
    movie_id: NumericID,
    /// Reference to the provider's work.
    reference: ExternalRef,
}

impl ApplyMovieCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the film to write.
    #[must_use]
    pub(crate) fn movie_id(&self) -> NumericID {
        self.movie_id
    }

    /// Create a new apply-film command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, movie_id: NumericID, reference: ExternalRef) -> Self {
        Self {
            caller_id,
            movie_id,
            reference,
        }
    }

    /// Return the reference to the provider's work.
    #[must_use]
    pub(crate) fn reference(&self) -> &ExternalRef {
        &self.reference
    }
}

/// Response of a successful film apply: the stored film.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub(crate) struct ApplyMovieResponse {
    /// The film that was stored.
    movie: Movie,
}

impl ApplyMovieResponse {
    /// Return the film that was stored.
    #[must_use]
    pub(crate) fn movie(&self) -> &Movie {
        &self.movie
    }

    /// Create a new applied-film response.
    #[must_use]
    pub(crate) fn new(movie: Movie) -> Self {
        Self { movie }
    }
}

/// Error returned when applying external metadata to a film.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ApplyMovieError {
    /// The write conflicts with the stored catalogue.
    #[error("the applied metadata conflicts with the stored catalogue")]
    Conflict,
    /// The caller may not manage the film.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The provider reports a work the film apply cannot write.
    #[error("the provider reports a work that cannot be applied to a film")]
    InvalidReference,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No film matches the requested identifier.
    #[error("a film with this identifier does not exist")]
    NoSuchMovie,
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

/// Use case for applying external metadata to a film.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ApplyMovieUseCase: Send + Sync {
    /// Resolve the reference and write the film.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn ApplyMovieUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ApplyMovieCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ApplyMovieResponse, ApplyMovieError>> + Send + 'future>>;
}
