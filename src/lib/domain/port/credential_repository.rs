use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::credential::Credential;
use crate::domain::port::error::RepositoryError;

/// Port for persisting and querying `Credential`.
#[cfg_attr(test, mockall::automock)]
pub trait CredentialRepository: Send + Sync {
    /// Delete the credential identified by `id`.
    ///
    /// Returns `Ok(true)` when a credential matched `id` and was deleted, and
    /// `Ok(false)` when no credential matched. A missing credential is a valid
    /// outcome, not an error.
    fn delete(&self, id: NumericID) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Find the credential identified by `id`, if any.
    ///
    /// Returns `Ok(None)` when no credential matches; a missing credential is a
    /// valid outcome, not an error.
    fn find(
        &self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Credential>, RepositoryError>> + Send;

    /// Insert or update `credential`, returning the persisted credential with its
    /// final identifier.
    fn save(
        &self,
        credential: Credential,
    ) -> impl Future<Output = Result<Credential, RepositoryError>> + Send;
}
