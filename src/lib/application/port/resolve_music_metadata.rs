use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::model::{MusicMetadata, MusicTarget};

/// Command for one music metadata resolution run.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ResolveMusicMetadataCommand {
    /// Caller on whose behalf the resolution runs.
    caller_id: NumericID,
    /// The item whose metadata the caller wants resolved.
    target: MusicTarget,
}

impl ResolveMusicMetadataCommand {
    /// Return the caller the resolution runs for.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, target: MusicTarget) -> Self {
        Self { caller_id, target }
    }

    /// Return the item to resolve.
    #[must_use]
    pub(crate) fn target(&self) -> &MusicTarget {
        &self.target
    }
}

/// Response of a successful resolution run.
///
/// The metadata is the provider-independent document of the target. A target
/// the provider does not know is reported as no response, not as an error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ResolveMusicMetadataResponse {
    /// Metadata the provider reported for the target.
    metadata: MusicMetadata,
}

impl ResolveMusicMetadataResponse {
    /// Return the metadata the provider reported.
    #[must_use]
    pub(crate) fn metadata(&self) -> &MusicMetadata {
        &self.metadata
    }

    /// Create a new response.
    #[must_use]
    pub(crate) fn new(metadata: MusicMetadata) -> Self {
        Self { metadata }
    }
}

/// Error returned when resolving music metadata fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ResolveMusicMetadataError {
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// The caller may not read the library, so identification is concealed.
    #[error("music identification is not available")]
    NotPermitted,
    /// No music metadata provider is configured, so resolution cannot run.
    #[error("no music metadata provider is configured")]
    Unavailable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The reference names a provider other than the configured one.
    #[error("the music metadata provider is not configured")]
    UnknownProvider,
}

/// Use case turning a music target into the metadata of a configured provider.
///
/// It resolves once, reports an item the provider does not know as no response
/// and never persists or logs the target. It is guarded by `ViewMetadata` on the catalogue.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ResolveMusicMetadataUseCase: Send + Sync {
    /// Resolve the command's target through the configured provider, reporting
    /// an item the provider does not know as `Ok(None)`.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn ResolveMusicMetadataUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ResolveMusicMetadataCommand,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<
                        Option<ResolveMusicMetadataResponse>,
                        ResolveMusicMetadataError,
                    >,
                > + Send
                + 'future,
        >,
    >;
}
