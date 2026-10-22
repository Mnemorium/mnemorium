use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::domain::service::username_policy::MAX_USERNAME_LENGTH;
use crate::domain::service::username_policy::MIN_USERNAME_LENGTH;

/// Command to update the profile of an existing user.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UpdateUserCommand {
    /// Identifier of the authenticated caller requesting the update.
    caller_id: NumericID,
    /// New email address, when provided; `None` leaves it unchanged.
    email: Option<String>,
    /// New role, when provided; `None` leaves it unchanged.
    role: Option<Role>,
    /// Identifier of the user to update.
    user_id: NumericID,
    /// New username, when provided; `None` leaves it unchanged.
    username: Option<String>,
}

impl UpdateUserCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the new email address, if any.
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Create a new update-user command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        user_id: NumericID,
        username: Option<String>,
        email: Option<String>,
        role: Option<Role>,
    ) -> Self {
        Self {
            caller_id,
            email,
            role,
            user_id,
            username,
        }
    }

    /// Return the new role, if any.
    #[must_use]
    pub fn role(&self) -> Option<Role> {
        self.role
    }

    /// Return the identifier of the user to update.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }

    /// Return the new username, if any.
    #[must_use]
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }
}

/// Response of a successful user update.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UpdateUserResponse {
    /// Email address of the user, when one was provided.
    email: Option<String>,
    /// Unique identifier of the user.
    id: NumericID,
    /// Role of the user.
    role: Role,
    /// Username of the user.
    username: String,
}

impl UpdateUserResponse {
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

    /// Create a new update-user response.
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

/// Error returned when updating the profile of a user.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UpdateUserError {
    /// A user with this email already exists.
    #[error("email is already taken")]
    EmailAlreadyExists,
    /// The email does not respect a valid email format.
    #[error("email has an invalid format")]
    InvalidEmail,
    /// The username does not satisfy the username policy.
    #[error(
        "username must be between {MIN_USERNAME_LENGTH} and {MAX_USERNAME_LENGTH} characters long, contain no uppercase letter, and not begin or end with whitespace"
    )]
    InvalidUsername,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No user matches the requested identifier.
    #[error("a user with this identifier does not exist")]
    NoSuchUser,
    /// The caller is not an administrator.
    #[error("only administrators may update users")]
    NotAdmin,
    /// Only the Root Admin may change the role of a user.
    #[error("only the Root Admin may change the role of a user")]
    RoleChangeForbidden,
    /// The target user cannot be modified by the caller.
    #[error("the targeted user cannot be modified by this caller")]
    TargetNotModifiable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// A concurrent update won the race for the username or the email.
    #[error("a user with this username or email already exists")]
    UserAlreadyExists,
    /// A user with this username already exists.
    #[error("username is already taken")]
    UsernameAlreadyExists,
}

/// Use case for updating the profile of a user.
#[cfg_attr(test, mockall::automock)]
pub trait UpdateUserUseCase: Send + Sync {
    /// Update the profile of the requested user.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn UpdateUserUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: UpdateUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<UpdateUserResponse, UpdateUserError>> + Send + 'future>>;
}
