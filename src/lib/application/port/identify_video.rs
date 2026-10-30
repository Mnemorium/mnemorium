use std::future::Future;
use std::pin::Pin;

use crate::domain::model::media_candidate::Candidate;

/// Command for one identification run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IdentifyVideoCommand {
    /// The best human-meaningful name the caller has: the root-relative path
    /// for a scanned file, the original client file name for an upload.
    name: String,
}

impl IdentifyVideoCommand {
    /// Return the media name to interpret.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new command.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

/// Response of a successful identification run.
///
/// The candidates come from the first query that yielded any, in the provider's
/// order. An empty list is a valid outcome: the search ran and found nothing,
/// which is distinct from the unavailable error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IdentifyVideoResponse {
    /// Candidates of the first query that yielded any.
    candidates: Vec<Candidate>,
}

impl IdentifyVideoResponse {
    /// Return the candidates of the first query that yielded any.
    #[must_use]
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Create a new response.
    #[must_use]
    pub fn new(candidates: Vec<Candidate>) -> Self {
        Self { candidates }
    }
}

/// Error returned when identifying a media name fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IdentifyVideoError {
    /// No metadata provider is configured, so identification cannot run.
    #[error("no video metadata provider is configured")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case turning a media name into the candidates of a configured provider.
///
/// It is internal: it has no HTTP entry point, persists nothing and ranks
/// nothing. A future upload, manual or background trigger is its caller.
#[cfg_attr(test, mockall::automock)]
pub trait IdentifyVideoUseCase: Send + Sync {
    /// Interpret the command's name, search the configured chain and return the
    /// candidates of the first query that yields any.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn IdentifyVideoUseCase>`.
    fn execute<'future>(
        &'future self,
        command: IdentifyVideoCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<IdentifyVideoResponse, IdentifyVideoError>> + Send + 'future,
        >,
    >;
}
