use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;

/// Command to fetch the authenticated caller's own account.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetCurrentUserCommand {
    /// Identifier of the authenticated caller.
    user_id: NumericID,
}

impl GetCurrentUserCommand {
    /// Create a new current-user command.
    #[must_use]
    pub fn new(user_id: NumericID) -> Self {
        Self { user_id }
    }

    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Response of a successful current-user lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetCurrentUserResponse {
    /// Email address of the caller, when set.
    email: Option<String>,
    /// Unique identifier of the caller.
    id: NumericID,
    /// Role of the caller.
    role: Role,
    /// Username of the caller.
    username: String,
}

impl GetCurrentUserResponse {
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

    /// Create a new current-user response.
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

/// Error returned when fetching the current user.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetCurrentUserError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchUser,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching the current user.
#[cfg_attr(test, mockall::automock)]
pub trait GetCurrentUserUseCase: Send + Sync {
    /// Fetch the account of the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetCurrentUserUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetCurrentUserCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetCurrentUserResponse, GetCurrentUserError>>
                + Send
                + 'future,
        >,
    >;
}
