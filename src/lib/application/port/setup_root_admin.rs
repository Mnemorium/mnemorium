use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// Command exchanging a Root Admin setup token for a chosen credential.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct SetupRootAdminCommand {
    /// Optional email address to store on the Root Admin.
    email: Option<String>,
    /// Password the Root Admin chooses; satisfies the credential policy.
    password: String,
    /// One-time setup token the first start revealed.
    token: String,
}

impl SetupRootAdminCommand {
    /// Return the optional email address.
    #[must_use]
    pub(crate) fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Create a new setup command.
    #[must_use]
    pub(crate) fn new(email: Option<String>, password: String, token: String) -> Self {
        Self {
            email,
            password,
            token,
        }
    }

    /// Return the chosen password.
    #[must_use]
    pub(crate) fn password(&self) -> &str {
        &self.password
    }

    /// Return the one-time setup token.
    #[must_use]
    pub(crate) fn token(&self) -> &str {
        &self.token
    }
}

impl Debug for SetupRootAdminCommand {
    /// Redact the token, the password and the email: none may reach a log sink.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("SetupRootAdminCommand")
            .field("email", &"**")
            .field("password", &"**")
            .field("token", &"**")
            .finish_non_exhaustive()
    }
}

/// Error returned when exchanging a setup token.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum SetupRootAdminError {
    /// Another account already holds the submitted email address.
    #[error("email is already taken")]
    EmailTaken,
    /// The email does not respect a valid email format.
    #[error("email has an invalid format")]
    InvalidEmail,
    /// The token is unknown, expired or already consumed.
    #[error("the setup token is invalid")]
    InvalidToken,
    /// The process-wide setup budget is spent.
    #[error("too many setup attempts; retry in {} seconds", wait.as_secs().max(1))]
    RateLimited {
        /// How long to wait before another attempt.
        wait: Duration,
    },
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The submitted password does not satisfy the credential policy.
    #[error("the password does not satisfy the credential policy")]
    WeakPassword,
}

/// Use case exchanging a Root Admin setup token for a chosen credential.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait SetupRootAdminUseCase: Send + Sync {
    /// Exchange a live setup token for the Root Admin credential.
    fn execute<'future>(
        &'future self,
        command: SetupRootAdminCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), SetupRootAdminError>> + Send + 'future>>;
}
