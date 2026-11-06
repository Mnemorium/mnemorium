use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::media_metadata::MediaMetadata;
use crate::domain::model::video_candidate::Target;

/// Command for one video metadata resolution run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ResolveVideoMetadataCommand {
    /// Caller on whose behalf the resolution runs.
    caller_id: NumericID,
    /// The item whose metadata the caller wants resolved.
    target: Target,
}

impl ResolveVideoMetadataCommand {
    /// Return the caller the resolution runs for.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, target: Target) -> Self {
        Self { caller_id, target }
    }

    /// Return the item to resolve.
    #[must_use]
    pub(crate) fn target(&self) -> &Target {
        &self.target
    }
}

/// Response of a successful resolution run.
///
/// The metadata is the provider-independent document of the target. A target
/// the provider does not know is reported as no response, not as an error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ResolveVideoMetadataResponse {
    /// Metadata the provider reported for the target.
    metadata: MediaMetadata,
}

impl ResolveVideoMetadataResponse {
    /// Return the metadata the provider reported.
    #[must_use]
    pub(crate) fn metadata(&self) -> &MediaMetadata {
        &self.metadata
    }

    /// Create a new response.
    #[must_use]
    pub(crate) fn new(metadata: MediaMetadata) -> Self {
        Self { metadata }
    }
}

/// Error returned when resolving video metadata fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ResolveVideoMetadataError {
    /// The provider resolved the reference to a kind the route did not name.
    #[error("the reference does not resolve to the requested kind")]
    KindMismatch,
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// The caller may not read the library, so identification is concealed.
    #[error("video identification is not available")]
    NotPermitted,
    /// The provider resolved an episode position other than the requested one.
    #[error("the reference does not resolve to the requested episode position")]
    PositionMismatch,
    /// No video metadata provider is configured, so resolution cannot run.
    #[error("no video metadata provider is configured")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case turning a video target into the metadata of a configured provider.
///
/// It resolves once, reports an item the provider does not know as no response
/// and never persists or logs the target. It is guarded by `library.read`.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ResolveVideoMetadataUseCase: Send + Sync {
    /// Resolve the command's target through the configured provider, reporting
    /// an item the provider does not know as `Ok(None)`.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as
    /// `Arc<dyn ResolveVideoMetadataUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ResolveVideoMetadataCommand,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        Option<ResolveVideoMetadataResponse>,
                        ResolveVideoMetadataError,
                    >,
                > + Send
                + 'future,
        >,
    >;
}
