use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::domain::port::IssuedToken;

/// Command to authenticate a user.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct LoginUserCommand {
    /// Password of the user to authenticate.
    password: String,
    /// Username of the user to authenticate.
    username: String,
}

impl LoginUserCommand {
    /// Create a new authentication command.
    #[must_use]
    pub(crate) fn new(username: String, password: String) -> Self {
        Self { password, username }
    }

    /// Return the password.
    #[must_use]
    pub(crate) fn password(&self) -> &str {
        &self.password
    }

    /// Return the username.
    #[must_use]
    pub(crate) fn username(&self) -> &str {
        &self.username
    }
}

/// Error returned when authenticating a user.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum LoginUserError {
    /// The password does not satisfy the stored hash of the user.
    #[error("the password does not match the stored hash")]
    InvalidPassword,
    /// No user exists with the provided username.
    #[error("no user matches the provided username")]
    InvalidUsername,
    /// The account's login quota is spent.
    #[error("too many authentication attempts; retry in {} seconds", .0.as_secs())]
    RateLimited(Duration),
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for authenticating a user.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait LoginUserUseCase: Send + Sync {
    /// Authenticate a user and issue a token.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn LoginUserUseCase>`. `Box` keeps the future on the heap at a
    /// stable address. `Pin` encodes the guarantee that the future is not moved
    /// once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: LoginUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<IssuedToken, LoginUserError>> + Send + 'future>>;
}
