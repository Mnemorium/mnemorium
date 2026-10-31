use std::future::Future;

use crate::domain::model::music_candidate::MusicCandidate;
use crate::domain::model::music_candidate::MusicTarget;
use crate::domain::model::music_metadata::MusicMetadata;
use crate::domain::model::music_query::MusicQuery;
use crate::domain::port::error::ExternalServiceError;

/// Port for asking an external service what an album, a release or a recording
/// is.
///
/// The contract is provider-independent: every adapter answers in the same
/// canonical values, so the caller never depends on which provider replied.
///
/// `search` is shallow and returns candidates in the provider's order; `lookup`
/// resolves one item and carries its artwork and editions.
#[cfg_attr(test, mockall::automock)]
pub trait MusicMetadataProvider: Send + Sync {
    /// Resolve the rich metadata of `target` through the provider that issued
    /// the target's reference.
    ///
    /// A missing item is a valid outcome, reported as `Ok(None)`; an absent
    /// entity is not a port error.
    ///
    /// # Errors
    ///
    /// Returns an [`ExternalServiceError`] classified at the adapter boundary
    /// when the provider cannot be reached, answers with a status it must not
    /// answer with, or returns a payload the adapter cannot map. The raw
    /// transport error never crosses this boundary.
    fn lookup(
        &self,
        target: &MusicTarget,
    ) -> impl Future<Output = Result<Option<MusicMetadata>, ExternalServiceError>> + Send;

    /// Search `query` and return the candidates in the provider's order.
    ///
    /// A search that matches nothing is a valid outcome, reported as an empty
    /// collection; the system adds no ranking of its own.
    ///
    /// # Errors
    ///
    /// Returns an [`ExternalServiceError`] classified at the adapter boundary
    /// when the provider cannot be reached, rejects the request, or returns a
    /// payload the adapter cannot map. The raw transport error never crosses
    /// this boundary.
    fn search(
        &self,
        query: &MusicQuery,
    ) -> impl Future<Output = Result<Vec<MusicCandidate>, ExternalServiceError>> + Send;
}
