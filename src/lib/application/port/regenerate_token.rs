use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;
use crate::domain::port::IssuedToken;

/// Command to regenerate the authenticated caller's access token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct RegenerateTokenCommand {
    /// Identifier of the authenticated caller.
    user_id: NumericID,
}

impl RegenerateTokenCommand {
    /// Create a new token-regeneration command.
    #[must_use]
    pub(crate) fn new(user_id: NumericID) -> Self {
        Self { user_id }
    }

    /// Return the authenticated caller's identifier.
    #[must_use]
    pub(crate) fn user_id(self) -> NumericID {
        self.user_id
    }
}

/// Error returned when regenerating a token.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum RegenerateTokenError {
    /// No user matches the authenticated subject.
    #[error("no user matches the authenticated caller")]
    NoSuchUser,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for extending an authenticated caller's session.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait RegenerateTokenUseCase: Send + Sync {
    /// Issue a fresh token for the authenticated caller.
    fn execute<'future>(
        &'future self,
        command: RegenerateTokenCommand,
    ) -> Pin<Box<dyn Future<Output = Result<IssuedToken, RegenerateTokenError>> + Send + 'future>>;
}
