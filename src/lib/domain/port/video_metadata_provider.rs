use std::future::Future;

use crate::domain::model::media_candidate::Candidate;
use crate::domain::model::media_candidate::Target;
use crate::domain::model::media_metadata::MediaMetadata;
use crate::domain::model::media_query::MediaQuery;
use crate::domain::port::error::ExternalServiceError;

/// Port for asking an external service what a film, a show or an episode is.
///
/// The contract is provider-independent: every adapter answers in the same
/// canonical values, so the failover chain, the use case and a future manual
/// selection flow never depend on which provider replied.
///
/// The two operations are separate because every provider's search endpoint is
/// shallow: a rich search response would force one detail request per hit.
/// `search` is cheap and returns candidates; `lookup` resolves one item and
/// carries the artwork.
#[cfg_attr(test, mockall::automock)]
pub trait VideoMetadataProvider: Send + Sync {
    /// Resolve the rich metadata of `target` through the provider that issued
    /// the target's reference.
    ///
    /// A missing item is a valid outcome, reported as `Ok(None)`; a missing
    /// entity is not a port error. An episode target names the show reference
    /// plus the season and episode position, so the adapter makes one request
    /// rather than walking a season tree.
    ///
    /// # Errors
    ///
    /// Returns an [`ExternalServiceError`] classified at the adapter boundary
    /// when the provider cannot be reached, answers with a status it must not
    /// answer with, or returns a payload the adapter cannot map. The raw
    /// transport error never crosses this boundary.
    fn lookup(
        &self,
        target: &Target,
    ) -> impl Future<Output = Result<Option<MediaMetadata>, ExternalServiceError>> + Send;

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
        query: &MediaQuery,
    ) -> impl Future<Output = Result<Vec<Candidate>, ExternalServiceError>> + Send;
}
