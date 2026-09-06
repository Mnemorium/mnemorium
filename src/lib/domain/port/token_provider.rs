use std::future::Future;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::domain::port::error::TokenProviderError;

/// Port for issuing and validating security tokens (for example, sessions or
/// JWTs).
#[cfg_attr(test, mockall::automock)]
pub trait TokenProvider: Send + Sync {
    /// Issue a token embedding `claims`.
    fn issue<T>(
        &self,
        claims: &T,
    ) -> impl Future<Output = Result<String, TokenProviderError>> + Send
    where
        T: Serialize + Sync + 'static;

    /// Validate `token` and recover the claims it embeds.
    fn validate<T>(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<T, TokenProviderError>> + Send
    where
        T: DeserializeOwned + Send + 'static;
}
