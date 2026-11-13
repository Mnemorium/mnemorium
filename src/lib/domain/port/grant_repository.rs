use std::future::Future;

use crate::domain::NumericID;
use crate::domain::model::{Grant, Principal};
use crate::domain::port::RepositoryError;

/// Search filters for [`GrantRepository::search`]. Every field is optional; an
/// all-`None` filter returns every grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GrantFilter {
    /// Filter on the principal the grant applies to.
    pub principal: Option<Principal>,
    /// Filter on the addressed resource's surrogate identifier.
    pub resource_id: Option<NumericID>,
}

/// Port for reading and writing grants.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GrantRepository: Send + Sync {
    /// Remove one grant stored on `resource_id`, returning whether a row was
    /// removed.
    fn delete(
        &mut self,
        grant_id: NumericID,
        resource_id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert or update one grant, returning its stored row.
    ///
    /// A grant is identified by its principal, resource, role, child kind and
    /// effect, so storing an existing combination updates it in place.
    fn save_grant(
        &mut self,
        grant: Grant,
    ) -> impl Future<Output = Result<Grant, RepositoryError>> + Send;

    /// Search grants matching `filter`, ordered by identifier.
    ///
    /// Returns an empty list when no grant matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &GrantFilter,
    ) -> impl Future<Output = Result<Vec<Grant>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: GrantRepository + ?Sized> GrantRepository for &mut T {
    fn delete(
        &mut self,
        grant_id: NumericID,
        resource_id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(grant_id, resource_id)
    }

    fn save_grant(
        &mut self,
        grant: Grant,
    ) -> impl Future<Output = Result<Grant, RepositoryError>> + Send {
        (**self).save_grant(grant)
    }

    fn search(
        &mut self,
        filter: &GrantFilter,
    ) -> impl Future<Output = Result<Vec<Grant>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
