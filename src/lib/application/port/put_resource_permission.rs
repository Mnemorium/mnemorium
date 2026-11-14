use std::future::Future;
use std::pin::Pin;

use crate::application::port::{PermissionError, ResourcePermission};
use crate::domain::NumericID;
use crate::domain::model::{
    ChildKind, GrantEffect, Principal, ResourceRef, ResourceRole, Visibility,
};

/// Command replacing one resource's visibility and user grant set.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PutResourcePermissionCommand {
    /// The addressed resource.
    pub address: ResourceRef,
    /// Identifier of the authenticated caller.
    pub caller_id: NumericID,
    /// The complete user grant set that replaces the stored one.
    pub grants: Vec<GrantRequest>,
    /// The visibility the resource takes.
    pub visibility: Visibility,
}

/// One grant a replacement requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GrantRequest {
    /// Kind of child the grant permits when its role includes `CreateChild`.
    pub child_kind: ChildKind,
    /// Whether the grant allows or denies its role's actions.
    pub effect: GrantEffect,
    /// The principal the grant applies to.
    pub principal: Principal,
    /// The resource role the grant carries.
    pub role: ResourceRole,
}

/// Use case replacing one resource's visibility and user grant set.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait PutResourcePermissionUseCase: Send + Sync {
    /// Replace the set, returning the stored permission.
    fn execute<'future>(
        &'future self,
        command: PutResourcePermissionCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ResourcePermission, PermissionError>> + Send + 'future>>;
}
