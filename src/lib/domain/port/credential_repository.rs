use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::credential::Credential;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`CredentialRepository::search`]. Every field is
/// optional; an all-`None` filter returns every credential.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CredentialFilter {
    /// Filter on the credential identifier.
    pub id: Option<NumericID>,
}

/// Port for persisting and querying `Credential`.
#[cfg_attr(test, mockall::automock)]
pub trait CredentialRepository: Send + Sync {
    /// Insert a new `credential`, returning the persisted credential with its
    /// final identifier.
    ///
    /// The identifier of `credential` is ignored: the repository assigns a
    /// fresh identity.
    fn create(
        &self,
        credential: Credential,
    ) -> impl Future<Output = Result<Credential, RepositoryError>> + Send;

    /// Delete the credential identified by `id`.
    ///
    /// Returns `Ok(true)` when a credential matched `id` and was deleted, and
    /// `Ok(false)` when no credential matched. A missing credential is a valid
    /// outcome, not an error.
    fn delete(&self, id: NumericID) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert or update an existing `credential` targeted by its identifier,
    /// returning the persisted credential.
    fn save(
        &self,
        credential: Credential,
    ) -> impl Future<Output = Result<Credential, RepositoryError>> + Send;

    /// Search credentials matching `filter`, returned as `Vec<Credential>`.
    ///
    /// Returns an empty list when no credential matches; a missing match is a
    /// valid outcome, not an error.
    fn search(
        &self,
        filter: &CredentialFilter,
    ) -> impl Future<Output = Result<Vec<Credential>, RepositoryError>> + Send;
}
