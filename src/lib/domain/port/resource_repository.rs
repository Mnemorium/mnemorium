use std::future::Future;

use crate::domain::NumericID;
use crate::domain::model::{
    Grant, Resource, ResourceAccess, ResourceKind, ResourceRef, User, Visibility,
};
use crate::domain::port::RepositoryError;

/// Search filters for [`ResourceRepository::search`]. Every field is optional;
/// an all-`None` filter returns every resource and every grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ResourceFilter {
    /// Filter on the entity identifier in its own table.
    pub entity_id: Option<NumericID>,
    /// Filter on the resource kind.
    pub kind: Option<ResourceKind>,
    /// Filter on the owning user.
    pub owner_id: Option<NumericID>,
    /// Filter on the resource's own surrogate identifier.
    pub resource_id: Option<NumericID>,
    /// Filter on the visibility.
    pub visibility: Option<Visibility>,
}

/// Port for reading the resource graph and persisting its grants.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ResourceRepository: Send + Sync {
    /// Create a resource, returning its stored row with its assigned identity.
    fn create(
        &mut self,
        resource: NewResource,
    ) -> impl Future<Output = Result<Resource, RepositoryError>> + Send;

    /// Delete the resource addressed by `address`, returning whether a stored
    /// row matched.
    ///
    /// The grants on the resource and its relation edges are removed with it. A
    /// missing row is a valid outcome, not an error.
    fn delete(
        &mut self,
        address: ResourceRef,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Reassign the owner and authorization parent of the resource addressed
    /// by `address`.
    ///
    /// `auth_parent_id` is the surrogate identifier of the new parent; `None`
    /// clears the parent. Returns whether a stored row matched. A missing row
    /// is a valid outcome, not an error.
    fn reassign_owner(
        &mut self,
        address: ResourceRef,
        owner_id: Option<NumericID>,
        auth_parent_id: Option<NumericID>,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert or update a resource, targeted by its surrogate identifier, and
    /// replace the user grant set it carries.
    ///
    /// Seeded role grants are never touched. The caller runs this inside one
    /// transaction, so a rejected set applies nothing.
    fn save(
        &mut self,
        resource: Resource,
        grants: &[Grant],
    ) -> impl Future<Output = Result<Resource, RepositoryError>> + Send;

    /// Search resources matching `filter`, each with its authorization chain,
    /// the grants that apply to `caller` and its relation containers.
    ///
    /// Returns an empty list when no resource matches; a missing match is a
    /// valid outcome, not an error.
    fn search(
        &mut self,
        filter: &ResourceFilter,
        caller: &User,
    ) -> impl Future<Output = Result<Vec<ResourceAccess>, RepositoryError>> + Send;
}

/// A resource to create together with its authorization parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewResource {
    /// Authorization parent the new resource follows, when it has one.
    pub auth_parent: Option<ResourceRef>,
    /// Entity identifier in its own table.
    pub entity_id: NumericID,
    /// Kind of the new resource.
    pub kind: ResourceKind,
    /// Owning user, when the resource is owned.
    pub owner_id: Option<NumericID>,
    /// Visibility the new resource starts with.
    pub visibility: Visibility,
}

#[cfg(test)]
impl<T: ResourceRepository + ?Sized> ResourceRepository for &mut T {
    fn create(
        &mut self,
        resource: NewResource,
    ) -> impl Future<Output = Result<Resource, RepositoryError>> + Send {
        (**self).create(resource)
    }

    fn delete(
        &mut self,
        address: ResourceRef,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(address)
    }

    fn reassign_owner(
        &mut self,
        address: ResourceRef,
        owner_id: Option<NumericID>,
        auth_parent_id: Option<NumericID>,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).reassign_owner(address, owner_id, auth_parent_id)
    }

    fn save(
        &mut self,
        resource: Resource,
        grants: &[Grant],
    ) -> impl Future<Output = Result<Resource, RepositoryError>> + Send {
        (**self).save(resource, grants)
    }

    fn search(
        &mut self,
        filter: &ResourceFilter,
        caller: &User,
    ) -> impl Future<Output = Result<Vec<ResourceAccess>, RepositoryError>> + Send {
        (**self).search(filter, caller)
    }
}
