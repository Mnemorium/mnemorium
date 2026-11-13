use std::future::Future;
use std::pin::Pin;

use crate::application::port::PermissionError;
use crate::domain::NumericID;
use crate::domain::model::ResourceRef;

/// Command revoking one grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct RevokeShareCommand {
    /// The addressed resource.
    pub address: ResourceRef,
    /// Identifier of the authenticated caller.
    pub caller_id: NumericID,
    /// Identifier of the grant to revoke.
    pub grant_id: NumericID,
}

/// Use case revoking one grant.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait RevokeShareUseCase: Send + Sync {
    /// Revoke one grant.
    fn execute<'future>(
        &'future self,
        command: RevokeShareCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), PermissionError>> + Send + 'future>>;
}
