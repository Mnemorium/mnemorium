use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::user::MIN_USERNAME_LENGTH;
use crate::domain::model::user::Role;

/// Command to register a new user on behalf of an authenticated caller.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RegisterUserCommand {
    /// Identifier of the authenticated caller requesting the registration.
    caller_id: NumericID,
    /// Email address of the user to register, when provided.
    email: Option<String>,
    /// Password of the user to register.
    password: String,
    /// Role to grant to the user to register.
    role: Role,
    /// Username of the user to register.
    username: String,
}

impl RegisterUserCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the email address, if any.
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Create a new registration command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        username: String,
        email: Option<String>,
        password: String,
        role: Role,
    ) -> Self {
        Self {
            caller_id,
            email,
            password,
            role,
            username,
        }
    }

    /// Return the password.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    /// Return the role to grant.
    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    /// Return the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

/// Response of a successful registration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RegisterUserResponse {
    /// Email address of the registered user, when one was provided.
    email: Option<String>,
    /// Unique identifier of the registered user.
    id: NumericID,
    /// Role granted to the registered user.
    role: Role,
    /// Username of the registered user.
    username: String,
}

impl RegisterUserResponse {
    /// Return the email address, if any.
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Return the unique identifier of the registered user.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Create a new registration response.
    #[must_use]
    pub fn new(id: NumericID, username: String, email: Option<String>, role: Role) -> Self {
        Self {
            email,
            id,
            role,
            username,
        }
    }

    /// Return the role granted to the registered user.
    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    /// Return the username of the registered user.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

/// Error returned when registering a new user.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RegisterUserError {
    /// The caller is not allowed to register a user with the requested role.
    #[error("the caller is not allowed to register the requested role")]
    Forbidden,
    /// The email does not respect a valid email format.
    #[error("email has an invalid format")]
    InvalidEmail,
    /// The password does not satisfy the password policy.
    #[error("password does not satisfy the password policy")]
    InvalidPassword,
    /// The username is shorter than [`MIN_USERNAME_LENGTH`].
    #[error("username must be at least {MIN_USERNAME_LENGTH} characters long")]
    InvalidUsername,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// A user with this username or email already exists.
    #[error("a user with this username or email already exists")]
    UserAlreadyExists,
}

/// Use case for registering a new user.
#[cfg_attr(test, mockall::automock)]
pub trait RegisterUserUseCase: Send + Sync {
    /// Register a new user.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn RegisterUserUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: RegisterUserCommand,
    ) -> Pin<
        Box<dyn Future<Output = Result<RegisterUserResponse, RegisterUserError>> + Send + 'future>,
    >;
}
