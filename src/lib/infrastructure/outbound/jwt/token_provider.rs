use std::future::{Future, ready};

use chrono::Duration;
use chrono::Utc;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::domain::alias::NumericID;
use crate::domain::port::error::TokenProviderError;
use crate::domain::port::token_provider::IssuedToken;
use crate::domain::port::token_provider::TokenProvider;

/// Default lifetime of an issued token, in seconds.
const TOKEN_TTL_SECONDS: u64 = 3600;

/// Claims embedded in a `JwtTokenProvider` token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Claims {
    /// Expiration time of the token, as a JSON numeric date (seconds since
    /// the Unix epoch).
    exp: u64,
    /// Subject of the token: the user identifier.
    sub: String,
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
    fn issue(
        &self,
        user_id: NumericID,
    ) -> impl Future<Output = Result<IssuedToken, TokenProviderError>> + Send {
        let header = Header::new(Algorithm::HS256);
        let key = EncodingKey::from_secret(self.secret.as_bytes());
        async move {
            let ttl = i64::try_from(TOKEN_TTL_SECONDS)
                .map_err(|_| TokenProviderError::OperationFailed)?;
            let expiration = Utc::now()
                .checked_add_signed(Duration::seconds(ttl))
                .ok_or(TokenProviderError::OperationFailed)?
                .timestamp();
            let exp = u64::try_from(expiration).map_err(|_| TokenProviderError::OperationFailed)?;
            let claims = Claims {
                exp,
                sub: user_id.to_string(),
            };
            encode(&header, &claims, &key)
                .map(|token| IssuedToken::new(token, TOKEN_TTL_SECONDS))
                .map_err(|_| TokenProviderError::InvalidClaims)
        }
    }

    fn validate(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<NumericID, TokenProviderError>> + Send {
        let key = DecodingKey::from_secret(self.secret.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.set_required_spec_claims(&["exp", "sub"]);

        let result = match decode::<Claims>(token, &key, &validation) {
            Ok(data) => data
                .claims
                .sub
                .parse::<NumericID>()
                .map_err(|_| TokenProviderError::InvalidToken),
            Err(err) if matches!(err.kind(), &ErrorKind::ExpiredSignature) => {
                Err(TokenProviderError::TokenExpired)
            }
            Err(_) => Err(TokenProviderError::InvalidToken),
        };

        ready(result)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde_json::json;

    use crate::domain::port::error::TokenProviderError;
    use crate::domain::port::token_provider::TokenProvider as _;

    use super::JwtTokenProvider;

    #[tokio::test]
    async fn issue_then_validate_round_trips_user_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtTokenProvider::new();

        // Act
        let issued = provider.issue(42).await?;
        let user_id = provider.validate(issued.value()).await?;

        // Assert
        assert_eq!(user_id, 42);
        assert_eq!(issued.expires_in(), 3600);
        Ok(())
    }

    #[tokio::test]
    async fn issue_returns_non_empty_token() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtTokenProvider::new();

        // Act
        let issued = provider.issue(7).await?;

        // Assert
        assert!(!issued.value().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn validate_garbage_token_returns_invalid_token() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtTokenProvider::new();

        // Act
        let result = provider.validate("not-a-token").await;

        // Assert
        assert!(matches!(result, Err(TokenProviderError::InvalidToken)));
        Ok(())
    }

    #[tokio::test]
    async fn validate_expired_token_returns_token_expired() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtTokenProvider::new();
        let expired = encode(
            &Header::new(Algorithm::HS256),
            &json!({ "sub": "42", "exp": 1i64 }),
            &EncodingKey::from_secret(b"tmptmp"),
        )?;

        // Act
        let result = provider.validate(&expired).await;

        // Assert
        assert!(matches!(result, Err(TokenProviderError::TokenExpired)));
        Ok(())
    }

    #[tokio::test]
    async fn validate_token_with_non_numeric_sub_returns_invalid_token()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtTokenProvider::new();
        let token = encode(
            &Header::new(Algorithm::HS256),
            &json!({ "sub": "alice", "exp": 4_102_444_800i64 }),
            &EncodingKey::from_secret(b"tmptmp"),
        )?;

        // Act
        let result = provider.validate(&token).await;

        // Assert
        assert!(matches!(result, Err(TokenProviderError::InvalidToken)));
        Ok(())
    }
}
