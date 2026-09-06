use std::future::{Future, ready};

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::domain::port::error::TokenProviderError;
use crate::domain::port::token_provider::TokenProvider;

/// Claims embedded in a `JwtTokenProvider` token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// Expiration time of the token, as a JSON numeric date (seconds since
    /// the Unix epoch).
    pub exp: u64,
    /// Subject of the token: the user identifier.
    pub sub: String,
}

impl Default for JwtTokenProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// JWT token provider, signing and verifying with `HS256`.
pub struct JwtTokenProvider {
    /// Shared signing secret.
    secret: String,
}

impl JwtTokenProvider {
    /// Create a new provider.
    ///
    /// TODO: load the secret from the configuration instead of hard-coding
    /// the temporary value.
    #[must_use]
    pub fn new() -> Self {
        Self {
            secret: "tmptmp".to_owned(),
        }
    }
}

impl TokenProvider for JwtTokenProvider {
    fn issue<T>(
        &self,
        claims: &T,
    ) -> impl Future<Output = Result<String, TokenProviderError>> + Send
    where
        T: Serialize + Sync + 'static,
    {
        let header = Header::new(Algorithm::HS256);
        let key = EncodingKey::from_secret(self.secret.as_bytes());
        ready(encode(&header, claims, &key).map_err(|_| TokenProviderError::InvalidClaims))
    }

    fn validate<T>(&self, token: &str) -> impl Future<Output = Result<T, TokenProviderError>> + Send
    where
        T: DeserializeOwned + Send + 'static,
    {
        let key = DecodingKey::from_secret(self.secret.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.set_required_spec_claims(&["exp", "sub"]);

        let result = match decode::<T>(token, &key, &validation) {
            Ok(data) => Ok(data.claims),
            Err(err) if matches!(err.kind(), &ErrorKind::ExpiredSignature) => {
                Err(TokenProviderError::TokenExpired)
            }
            Err(_) => Err(TokenProviderError::InvalidToken),
        };

        ready(result)
    }
}
