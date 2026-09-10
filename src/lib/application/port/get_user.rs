use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;

/// Command to fetch an account by its identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetUserCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the account to fetch.
    user_id: NumericID,
}

impl GetUserCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new fetch-user command.
    #[must_use]
    pub fn new(caller_id: NumericID, user_id: NumericID) -> Self {
        Self { caller_id, user_id }
    }

    /// Return the identifier of the account to fetch.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Response of a successful fetch-user lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetUserResponse {
    /// Email address of the caller, when set.
    email: Option<String>,
    /// Unique identifier of the caller.
    id: NumericID,
    /// Role of the caller.
    role: Role,
    /// Username of the caller.
    username: String,
}

impl GetUserResponse {
    /// Return the email address, if any.
    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Return the unique identifier of the caller.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Create a new fetch-user response.
    #[must_use]
    pub fn new(id: NumericID, username: String, email: Option<String>, role: Role) -> Self {
        Self {
            email,
            id,
            role,
            username,
        }
    }

    /// Return the role of the caller.
    #[must_use]
    pub fn role(&self) -> Role {
        self.role
    }

    /// Return the username of the caller.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

/// Error returned when fetching a user by identifier.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetUserError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No user matches the requested identifier.
    #[error("a user with this identifier does not exist")]
    NoSuchUser,
    /// The caller is not an administrator.
    #[error("only administrators may fetch other users")]
    NotAdmin,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching a user by identifier.
#[cfg_attr(test, mockall::automock)]
pub trait GetUserUseCase: Send + Sync {
    /// Fetch the account of the requested user.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetUserUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetUserResponse, GetUserError>> + Send + 'future>>;
}
