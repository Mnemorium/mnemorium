use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::port::error::TokenProviderError;

/// Token issued by a successful authentication.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct IssuedToken {
    /// Lifetime of the token, in seconds.
    expires_in: u64,
    /// The token to present in the `Authorization` header.
    value: String,
}

impl IssuedToken {
    /// Return the lifetime of the token, in seconds.
    #[must_use]
    pub fn expires_in(&self) -> u64 {
        self.expires_in
    }

    /// Create a new issued token.
    #[must_use]
    pub fn new(value: String, expires_in: u64) -> Self {
        Self { expires_in, value }
    }

    /// Return the token value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl Debug for IssuedToken {
    /// Redact the issued token (`OBS-004`): the token value never reaches a log
    /// sink, while the lifetime stays readable.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("IssuedToken")
            .field("expires_in", &self.expires_in)
            .field("value", &"**")
            .finish_non_exhaustive()
    }
}

/// Port for issuing and validating security tokens (for example, sessions or
/// JWTs).
#[cfg_attr(test, mockall::automock)]
pub trait TokenProvider: Send + Sync {
    /// Issue a token embedding the user identifier.
    fn issue(
        &self,
        user_id: NumericID,
    ) -> impl Future<Output = Result<IssuedToken, TokenProviderError>> + Send;

    /// Validate `token` and recover the user identifier it embeds.
    fn validate(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<NumericID, TokenProviderError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::IssuedToken;

    #[test]
    fn issued_token_debug_redacts_the_value_but_keeps_the_lifetime() {
        // Arrange
        let token = IssuedToken::new("LEAK_SENTINEL".to_owned(), 3600);

        // Act
        let debug = format!("{token:?}");

        // Assert
        assert!(
            !debug.contains("LEAK_SENTINEL"),
            "Debug output must not contain the token value, got: {debug}"
        );
        assert!(
            debug.contains("3600"),
            "Debug output must still contain the lifetime, got: {debug}"
        );
    }
}
