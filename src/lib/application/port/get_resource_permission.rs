use std::future::Future;
use std::pin::Pin;

use crate::application::port::PermissionError;
use crate::domain::NumericID;
use crate::domain::model::{Grant, ResourceRef, Visibility};

/// Command returning one resource's visibility and grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetResourcePermissionCommand {
    /// The addressed resource.
    pub address: ResourceRef,
    /// Identifier of the authenticated caller.
    pub caller_id: NumericID,
}

/// Visibility, owner and grants of one resource.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ResourcePermission {
    /// The addressed resource.
    pub address: ResourceRef,
    /// The stored grants.
    pub grants: Vec<Grant>,
    /// Identifier of the owning user, when the resource is owned.
    pub owner_id: Option<NumericID>,
    /// Visibility of the resource.
    pub visibility: Visibility,
}

/// Use case returning one resource's visibility and grants.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetResourcePermissionUseCase: Send + Sync {
    /// Return the resource permission the caller is allowed to read.
    fn execute<'future>(
        &'future self,
        command: GetResourcePermissionCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ResourcePermission, PermissionError>> + Send + 'future>>;
}
