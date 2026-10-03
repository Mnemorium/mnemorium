use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;

/// Command to list the users of the instance.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListUsersCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Only return users whose email matches this value, when set.
    email: Option<String>,
    /// Only return users holding this role, when set.
    role: Option<Role>,
    /// Only return users whose username matches this value, when set.
    username: Option<String>,
}

impl ListUsersCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the email filter, if any.
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Create a new list-users command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        email: Option<String>,
        role: Option<Role>,
        username: Option<String>,
    ) -> Self {
        Self {
            caller_id,
            email,
            role,
            username,
        }
    }

    /// Return the role filter, if any.
    #[must_use]
    pub fn role(&self) -> Option<Role> {
        self.role
    }

    /// Return the username filter, if any.
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }
}

/// Response of a successful user listing: the profile of one matching user.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListUsersResponse {
    /// Email address of the user, when one was provided.
    email: Option<String>,
    /// Unique identifier of the user.
    id: NumericID,
    /// Role of the user.
    role: Role,
    /// Username of the user.
    username: String,
}

impl ListUsersResponse {
    /// Return the email address, if any.
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Return the unique identifier of the user.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Create a new list-users response.
    #[must_use]
    pub fn new(id: NumericID, username: String, email: Option<String>, role: Role) -> Self {
        Self {
            email,
            id,
            role,
            username,
        }
    }

    /// Return the role of the user.
    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    /// Return the username of the user.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

/// Error returned when listing the users of the instance.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListUsersError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// The caller is not an administrator.
    #[error("only administrators may list users")]
    NotAdmin,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the users of the instance.
#[cfg_attr(test, mockall::automock)]
pub trait ListUsersUseCase: Send + Sync {
    /// List the users matching the command's optional filters.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn ListUsersUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: ListUsersCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<Vec<ListUsersResponse>, ListUsersError>> + Send + 'future>,
    >;
}
