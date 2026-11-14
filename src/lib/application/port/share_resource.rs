use std::future::Future;
use std::pin::Pin;

use crate::application::port::PermissionError;
use crate::domain::NumericID;
use crate::domain::model::{Grant, Principal, ResourceRef, ResourceRole};

/// Command adding or updating one read grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ShareResourceCommand {
    /// The addressed resource.
    pub address: ResourceRef,
    /// Identifier of the authenticated caller.
    pub caller_id: NumericID,
    /// The principal the share reaches.
    pub principal: Principal,
    /// The read role the share carries.
    pub role: ResourceRole,
}

/// Use case adding or updating one read grant.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ShareResourceUseCase: Send + Sync {
    /// Add or update one read grant.
    fn execute<'future>(
        &'future self,
        command: ShareResourceCommand,
    ) -> Pin<Box<dyn Future<Output = Result<Grant, PermissionError>> + Send + 'future>>;
}
