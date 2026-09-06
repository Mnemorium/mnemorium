use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::port::error::TokenProviderError;

/// Token issued by a successful authentication.
#[derive(Debug, Clone, PartialEq, Eq)]
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
