use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::permission::Permission;
use crate::domain::model::permission::PermissionPreset;
use crate::domain::model::user::Role;

/// Command fetching a user's effective permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetPermissionCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the user whose permissions are read.
    user_id: NumericID,
}

impl GetPermissionCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Initialise a new `GetPermissionCommand`.
    #[must_use]
    pub fn new(caller_id: NumericID, user_id: NumericID) -> Self {
        Self { caller_id, user_id }
    }

    /// Return the identifier of the user whose permissions are read.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Effective permissions of one user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetPermissionResponse {
    /// Role presets per configurable action, ordered by action name.
    defaults: Vec<PermissionPreset>,
    /// Stored overrides, ordered by action name.
    overrides: Vec<Permission>,
    /// Role the presets are read from.
    role: Role,
    /// Identifier of the user the permissions apply to.
    user_id: NumericID,
}

impl GetPermissionResponse {
    /// Return the role presets per configurable action.
    #[must_use]
    pub fn defaults(&self) -> &[PermissionPreset] {
        &self.defaults
    }

    /// Initialise a new `GetPermissionResponse`.
    #[must_use]
    pub fn new(
        user_id: NumericID,
        role: Role,
        defaults: Vec<PermissionPreset>,
        overrides: Vec<Permission>,
    ) -> Self {
        Self {
            defaults,
            overrides,
            role,
            user_id,
        }
    }

    /// Return the stored overrides.
    #[must_use]
    pub fn overrides(&self) -> &[Permission] {
        &self.overrides
    }

    /// Return the role the presets are read from.
    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    /// Return the identifier of the user the permissions apply to.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Error returned when fetching a user's permissions fails.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetPermissionError {
    /// The caller may not administer permissions.
    #[error("only the Root Admin may administer permissions")]
    Forbidden,
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

/// Use case fetching a user's effective permissions.
#[cfg_attr(test, mockall::automock)]
pub trait GetPermissionUseCase: Send + Sync {
    /// Fetch the effective permissions of the command's user.
    fn execute<'future>(
        &'future self,
        command: GetPermissionCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetPermissionResponse, GetPermissionError>> + Send + 'future,
        >,
    >;
}
