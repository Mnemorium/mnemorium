use std::future::Future;

use crate::domain::NumericID;
use crate::domain::model::Credential;
use crate::domain::port::RepositoryError;

/// Search filters for [`CredentialRepository::search`]. Every field is
/// optional; an all-`None` filter returns every credential.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CredentialFilter {
    /// Filter on the credential identifier.
    pub id: Option<NumericID>,
}

/// Port for persisting and querying `Credential`.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait CredentialRepository: Send + Sync {
    /// Insert a new `credential`, returning the persisted credential with its
    /// final identifier.
    ///
    /// The identifier of `credential` is ignored: the repository assigns a
    /// fresh identity.
    fn create(
        &mut self,
        credential: Credential,
    ) -> impl Future<Output = Result<Credential, RepositoryError>> + Send;

    /// Delete every credential matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one credential was deleted, and
    /// `Ok(false)` when none matched. An all-`None` filter deletes every
    /// credential.
    fn delete(
        &mut self,
        filter: &CredentialFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert or update an existing `credential` targeted by its identifier,
    /// returning the persisted credential.
    fn save(
        &mut self,
        credential: Credential,
    ) -> impl Future<Output = Result<Credential, RepositoryError>> + Send;

    /// Search credentials matching `filter`, returned as `Vec<Credential>`.
    ///
    /// Returns an empty list when no credential matches; a missing match is a
    /// valid outcome, not an error.
    fn search(
        &mut self,
        filter: &CredentialFilter,
    ) -> impl Future<Output = Result<Vec<Credential>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: CredentialRepository + ?Sized> CredentialRepository for &mut T {
    fn create(
        &mut self,
        credential: Credential,
    ) -> impl Future<Output = Result<Credential, RepositoryError>> + Send {
        (**self).create(credential)
    }

    fn delete(
        &mut self,
        filter: &CredentialFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn save(
        &mut self,
        credential: Credential,
    ) -> impl Future<Output = Result<Credential, RepositoryError>> + Send {
        (**self).save(credential)
    }

    fn search(
        &mut self,
        filter: &CredentialFilter,
    ) -> impl Future<Output = Result<Vec<Credential>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
