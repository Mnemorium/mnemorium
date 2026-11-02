use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to read the content of one asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAssetContentCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Asset whose bytes are requested.
    target: AssetTarget,
}

impl GetAssetContentCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new asset-content command.
    #[must_use]
    pub fn new(caller_id: NumericID, target: AssetTarget) -> Self {
        Self { caller_id, target }
    }

    /// Return the asset whose bytes are requested.
    #[must_use]
    pub fn target(&self) -> AssetTarget {
        self.target
    }
}

/// Response of a successful asset-content read: the bytes and their media type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetAssetContentResponse {
    /// Content of the asset.
    bytes: Vec<u8>,
    /// Identifier of the stored media type of the asset.
    mime_type_id: String,
}

impl GetAssetContentResponse {
    /// Return the content of the asset.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return the identifier of the stored media type of the asset.
    #[must_use]
    pub fn mime_type_id(&self) -> &str {
        &self.mime_type_id
    }

    /// Create a new asset-content response.
    #[must_use]
    pub fn new(mime_type_id: String, bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            mime_type_id,
        }
    }
}

/// The asset a content request addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssetTarget {
    /// A stored file addressed by its own identifier.
    File(NumericID),
    /// A typed image addressed by its identifier, resolved to its file.
    Image(NumericID),
}

/// Error returned when reading the content of an asset fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetAssetContentError {
    /// The asset does not exist, or the caller may not read it.
    ///
    /// Both outcomes are reported the same way so a caller cannot probe for
    /// assets it may not read, and no bytes are read for an unreadable asset.
    #[error("the asset does not exist")]
    NoSuchAsset,
    /// The caller does not exist.
    #[error("the caller does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case reading the content of one asset.
#[cfg_attr(test, mockall::automock)]
pub trait GetAssetContentUseCase: Send + Sync {
    /// Read the requested asset content.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn GetAssetContentUseCase>`.
    fn execute<'future>(
        &'future self,
        command: GetAssetContentCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetAssetContentResponse, GetAssetContentError>>
                + Send
                + 'future,
        >,
    >;
}
