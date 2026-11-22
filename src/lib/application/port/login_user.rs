use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::domain::port::IssuedToken;

/// Command to authenticate a user.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct LoginUserCommand {
    /// Username or email address of the user to authenticate.
    identifier: String,
    /// Password of the user to authenticate.
    password: String,
}

impl LoginUserCommand {
    /// Return the username or email address.
    #[must_use]
    pub(crate) fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Create a new authentication command.
    #[must_use]
    pub(crate) fn new(identifier: String, password: String) -> Self {
        Self {
            identifier,
            password,
        }
    }

    /// Return the password.
    #[must_use]
    pub(crate) fn password(&self) -> &str {
        &self.password
    }
}

/// Error returned when authenticating a user.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum LoginUserError {
    /// No user exists with the provided identifier.
    #[error("no user matches the provided identifier")]
    InvalidIdentifier,
    /// The password does not satisfy the stored hash of the user.
    #[error("the password does not match the stored hash")]
    InvalidPassword,
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
    fn execute<'future>(
        &'future self,
        command: LoginUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<IssuedToken, LoginUserError>> + Send + 'future>>;
}
