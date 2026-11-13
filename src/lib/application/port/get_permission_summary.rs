use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::model::{Action, Grant, ResourceRef, Role};
use crate::domain::service::Explanation;

/// Command returning one user's effective permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetPermissionSummaryCommand {
    /// Identifier of the authenticated caller.
    pub caller_id: NumericID,
    /// Decision to explain, when the caller asked for one.
    pub decision: Option<PermissionDecisionQuery>,
    /// Identifier of the user whose summary is requested.
    pub user_id: NumericID,
}

/// Error returned by a permission use case.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum PermissionError {
    /// The caller may not perform the operation.
    #[error("the caller may not perform the operation")]
    Forbidden,
    /// The requested grant set is invalid.
    #[error("the requested grant is invalid: {0}")]
    Invalid(&'static str),
    /// The addressed resource or grant does not exist.
    #[error("the addressed permission does not exist")]
    NotFound,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// One decision the summary may explain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PermissionDecisionQuery {
    /// Action the decision answers.
    pub action: Action,
    /// Resource the decision addresses.
    pub resource: ResourceRef,
}

/// A grant together with the resource it reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GrantedResource {
    /// The stored grant.
    pub grant: Grant,
    /// The resource the grant is stored on.
    pub resource: ResourceRef,
}

/// Effective permissions of one user, with an optional decision explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct PermissionSummary {
    /// Whether the caller may see grant internals.
    pub detailed: bool,
    /// Explanation of the requested decision, when one was requested.
    pub explanation: Option<Explanation>,
    /// Grants that apply to the user.
    pub grants: Vec<GrantedResource>,
    /// Role of the user the summary describes.
    pub role: Role,
    /// Identifier of the user the summary describes.
    pub user_id: NumericID,
}

/// Use case returning one user's effective permissions.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetPermissionSummaryUseCase: Send + Sync {
    /// Return the summary the caller is allowed to read.
    fn execute<'future>(
        &'future self,
        command: GetPermissionSummaryCommand,
    ) -> Pin<Box<dyn Future<Output = Result<PermissionSummary, PermissionError>> + Send + 'future>>;
}
