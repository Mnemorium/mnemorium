use std::future::Future;
use std::pin::Pin;

use crate::application::port::get_permission::GetPermissionResponse;
use crate::domain::alias::NumericID;
use crate::domain::model::permission::Action;
use crate::domain::model::permission::Scope;

/// One requested override, validated by the use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacePermissionCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// The complete override set the user must end with.
    permissions: Vec<PermissionRequest>,
    /// Identifier of the user whose permissions are replaced.
    user_id: NumericID,
}

impl ReplacePermissionCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Initialise a new `ReplacePermissionCommand`.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        user_id: NumericID,
        permissions: Vec<PermissionRequest>,
    ) -> Self {
        Self {
            caller_id,
            permissions,
            user_id,
        }
    }

    /// Return the complete override set the user must end with.
    #[must_use]
    pub fn permissions(&self) -> &[PermissionRequest] {
        &self.permissions
    }

    /// Return the identifier of the user whose permissions are replaced.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Error returned when replacing a user's permissions fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// The action the request overrides.
    action: Action,
    /// The requested scope.
    scope: Scope,
}

impl PermissionRequest {
    /// Return the overridden action.
    #[must_use]
    pub fn action(&self) -> Action {
        self.action
    }

    /// Initialise a new `PermissionRequest`.
    #[must_use]
    pub fn new(action: Action, scope: Scope) -> Self {
        Self { action, scope }
    }

    /// Return the requested scope.
    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}

/// Command replacing every permission override of a user.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReplacePermissionError {
    /// The same action appears twice in the request.
    #[error("the same action appears twice in the request")]
    DuplicateAction,
    /// The caller may not administer permissions.
    #[error("only the Root Admin may administer permissions")]
    Forbidden,
    /// A requested permission is invalid.
    #[error("a requested permission is invalid")]
    InvalidPermission,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No user matches the requested identifier.
    #[error("a user with this identifier does not exist")]
    NoSuchUser,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case replacing every permission override of a user.
#[cfg_attr(test, mockall::automock)]
pub trait ReplacePermissionUseCase: Send + Sync {
    /// Replace the overrides of the command's user, atomically.
    fn execute<'future>(
        &'future self,
        command: ReplacePermissionCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetPermissionResponse, ReplacePermissionError>>
                + Send
                + 'future,
        >,
    >;
}
