use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::port::error::ContentLinkError;

/// Lifetime of a content link, in seconds (twelve hours).
///
/// Long enough for a film plus seeking, short enough to bound a revoked
/// permission.
pub(crate) const CONTENT_LINK_TTL_SECONDS: u64 = 43_200;

/// A minted, file-scoped playback credential.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ContentLink {
    /// Instant at which the credential expires, as Unix seconds.
    expires_at: u64,
    /// Opaque signed credential.
    signature: String,
}

impl ContentLink {
    /// Return the instant at which the credential expires, as Unix seconds.
    #[must_use]
    pub(crate) fn expires_at(&self) -> u64 {
        self.expires_at
    }

    /// Create a new content link.
    #[must_use]
    pub(crate) fn new(expires_at: u64, signature: String) -> Self {
        Self {
            expires_at,
            signature,
        }
    }

    /// Return the opaque signed credential.
    #[must_use]
    pub(crate) fn signature(&self) -> &str {
        &self.signature
    }
}

/// Port minting and verifying a file-scoped, expiring playback credential.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ContentLinkProvider: Send + Sync {
    /// Mint a credential scoped to `file_id`.
    fn mint(
        &self,
        file_id: NumericID,
    ) -> impl Future<Output = Result<ContentLink, ContentLinkError>> + Send;

    /// Verify `signature` at `expires_at` and recover the file it names.
    ///
    /// A credential minted for another file, a tampered value and an expired
    /// credential are all rejected.
    fn verify(
        &self,
        expires_at: u64,
        signature: &str,
    ) -> impl Future<Output = Result<NumericID, ContentLinkError>> + Send;
}
