use std::future::Future;
use std::pin::Pin;

/// Command to authenticate a user.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoginUserCommand {
    /// Password of the user to authenticate.
    password: String,
    /// Username of the user to authenticate.
    username: String,
}

impl LoginUserCommand {
    /// Create a new authentication command.
    #[must_use]
    pub fn new(username: String, password: String) -> Self {
        Self { password, username }
    }

    /// Return the password.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    /// Return the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

/// Response of a successful authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoginUserResponse {
    /// Token to present in the `Authorization` header of subsequent requests.
    access_token: String,
    /// Lifetime of the token, in seconds.
    expires_in: u64,
}

impl LoginUserResponse {
    /// Return the token to present in the `Authorization` header.
    #[must_use]
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Return the lifetime of the token, in seconds.
    #[must_use]
    pub fn expires_in(&self) -> u64 {
        self.expires_in
    }

    /// Create a new authentication response.
    #[must_use]
    pub fn new(access_token: String, expires_in: u64) -> Self {
        Self {
            access_token,
            expires_in,
        }
    }
}

/// Error returned when authenticating a user.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoginUserError {
    /// The password does not satisfy the stored hash of the user.
    #[error("the password does not match the stored hash")]
    InvalidPassword,
    /// No user exists with the provided username.
    #[error("no user matches the provided username")]
    InvalidUsername,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for authenticating a user.
#[cfg_attr(test, mockall::automock)]
pub trait LoginUserUseCase: Send + Sync {
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
    ) -> Pin<Box<dyn Future<Output = Result<LoginUserResponse, LoginUserError>> + Send + 'future>>;
}
