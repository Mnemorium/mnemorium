use std::future::Future;
use std::pin::Pin;

use crate::domain::model::music_candidate::MusicCandidate;
use crate::domain::model::music_query::MusicQuery;

/// Command for one music identification run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IdentifyMusicCommand {
    /// The query the caller wants searched.
    query: MusicQuery,
}

impl IdentifyMusicCommand {
    /// Create a new command.
    #[must_use]
    pub fn new(query: MusicQuery) -> Self {
        Self { query }
    }

    /// Return the query to search.
    #[must_use]
    pub fn query(&self) -> &MusicQuery {
        &self.query
    }
}

/// Response of a successful identification run.
///
/// The candidates come from the provider's single search, in its own order. An
/// empty list is a valid outcome: the search ran and found nothing, which is
/// distinct from the unavailable error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IdentifyMusicResponse {
    /// Candidates the provider reported, in its order.
    candidates: Vec<MusicCandidate>,
}

impl IdentifyMusicResponse {
    /// Return the candidates the provider reported, in its order.
    #[must_use]
    pub fn candidates(&self) -> &[MusicCandidate] {
        &self.candidates
    }

    /// Create a new response.
    #[must_use]
    pub fn new(candidates: Vec<MusicCandidate>) -> Self {
        Self { candidates }
    }
}

/// Error returned when identifying music fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IdentifyMusicError {
    /// No music metadata provider is configured, so identification cannot run.
    #[error("no music metadata provider is configured")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case turning a music query into the candidates of a configured provider.
///
/// It is internal: it has no HTTP entry point, persists nothing and ranks
/// nothing. A future ingest, manual or background trigger is its caller.
#[cfg_attr(test, mockall::automock)]
pub trait IdentifyMusicUseCase: Send + Sync {
    /// Search the configured provider for the command's query and return its
    /// candidates in its order.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn IdentifyMusicUseCase>`.
    fn execute<'future>(
        &'future self,
        command: IdentifyMusicCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<IdentifyMusicResponse, IdentifyMusicError>> + Send + 'future,
        >,
    >;
}
