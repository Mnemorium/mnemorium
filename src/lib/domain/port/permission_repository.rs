use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::permission::{Action, Permission};
use crate::domain::port::error::RepositoryError;

/// Search filters for [`PermissionRepository::search`]. Every field is
/// optional; an all-`None` filter returns every permission.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PermissionFilter {
    /// Filter on the overridden action.
    pub action: Option<Action>,
    /// Filter on the identifier of the user the permission applies to.
    pub user_id: Option<NumericID>,
}

/// Port for persisting and querying `Permission` overrides.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait PermissionRepository: Send + Sync {
    /// Delete every permission matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one permission was deleted, and
    /// `Ok(false)` when none matched. An all-`None` filter deletes every
    /// permission; a filter naming only `user_id` deletes every permission of
    /// that user, and naming `(user_id, action)` deletes that one override.
    fn delete(
        &mut self,
        filter: &PermissionFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert or replace `permission`, returning the persisted permission.
    ///
    /// The replacement is atomic for a `LISTED` scope: the stored child set
    /// becomes exactly the permission's named resources.
    fn save(
        &mut self,
        permission: Permission,
    ) -> impl Future<Output = Result<Permission, RepositoryError>> + Send;

    /// Search permissions matching `filter`, returned as `Vec<Permission>`.
    ///
    /// Returns an empty list when no permission matches; a missing match is a
    /// valid outcome, not an error.
    fn search(
        &mut self,
        filter: &PermissionFilter,
    ) -> impl Future<Output = Result<Vec<Permission>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: PermissionRepository + ?Sized> PermissionRepository for &mut T {
    fn delete(
        &mut self,
        filter: &PermissionFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn save(
        &mut self,
        permission: Permission,
    ) -> impl Future<Output = Result<Permission, RepositoryError>> + Send {
        (**self).save(permission)
    }

    fn search(
        &mut self,
        filter: &PermissionFilter,
    ) -> impl Future<Output = Result<Vec<Permission>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
