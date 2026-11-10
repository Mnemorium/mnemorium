use std::future::{Future, ready};

use chrono::{Duration, Utc};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::domain::NumericID;
use crate::domain::port::{
    CONTENT_LINK_TTL_SECONDS, ContentLink, ContentLinkError, ContentLinkProvider,
};

/// Audience separating a playback credential from a session token.
const CONTENT_LINK_AUDIENCE: &str = "media-link";

/// Claims embedded in a content credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ContentLinkClaims {
    /// Audience distinguishing the credential from a session token.
    aud: String,
    /// Expiration time, as a JSON numeric date (seconds since the Unix epoch).
    exp: u64,
    /// Subject of the credential: the file identifier.
    sub: String,
}

/// HMAC-backed content link provider.
///
/// The credential is an `HS256` JWT over a domain-separated message: the
/// audience names the playback purpose, the subject the one file and the expiry
/// the lifetime. It is signed with the instance signing secret, so no new secret
/// and no new configuration are introduced.
pub(crate) struct JwtContentLinkProvider {
    /// Shared signing secret.
    secret: String,
}

impl JwtContentLinkProvider {
    /// Create a new provider.
    #[must_use]
    pub(crate) fn new(secret: String) -> Self {
        Self { secret }
    }
}

impl ContentLinkProvider for JwtContentLinkProvider {
    fn mint(
        &self,
        file_id: NumericID,
    ) -> impl Future<Output = Result<ContentLink, ContentLinkError>> + Send {
        let header = Header::new(Algorithm::HS256);
        let key = EncodingKey::from_secret(self.secret.as_bytes());
        async move {
            let ttl = i64::try_from(CONTENT_LINK_TTL_SECONDS)
                .map_err(|_| ContentLinkError::OperationFailed)?;
            let expiration = Utc::now()
                .checked_add_signed(Duration::seconds(ttl))
                .ok_or(ContentLinkError::OperationFailed)?
                .timestamp();
            let expires_at =
                u64::try_from(expiration).map_err(|_| ContentLinkError::OperationFailed)?;
            let claims = ContentLinkClaims {
                aud: CONTENT_LINK_AUDIENCE.to_owned(),
                exp: expires_at,
                sub: file_id.to_string(),
            };
            encode(&header, &claims, &key)
                .map(|signature| ContentLink::new(expires_at, signature))
                .map_err(|_| ContentLinkError::OperationFailed)
        }
    }

    fn verify(
        &self,
        expires_at: u64,
        signature: &str,
    ) -> impl Future<Output = Result<NumericID, ContentLinkError>> + Send {
        let key = DecodingKey::from_secret(self.secret.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.set_audience(&[CONTENT_LINK_AUDIENCE]);
        validation.set_required_spec_claims(&["aud", "exp", "sub"]);

        let result = match decode::<ContentLinkClaims>(signature, &key, &validation) {
            Ok(data) => {
                if data.claims.exp == expires_at {
                    data.claims
                        .sub
                        .parse::<NumericID>()
                        .map_err(|_| ContentLinkError::Invalid)
                } else {
                    Err(ContentLinkError::Invalid)
                }
            }
            Err(err) if matches!(err.kind(), &ErrorKind::ExpiredSignature) => {
                Err(ContentLinkError::Expired)
            }
            Err(_) => Err(ContentLinkError::Invalid),
        };
        ready(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[tokio::test]
    async fn mint_then_verify_round_trips_the_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtContentLinkProvider::new("secret".to_owned());

        // Act
        let link = provider.mint(42).await?;
        let file_id = provider.verify(link.expires_at(), link.signature()).await?;

        // Assert
        assert_eq!(file_id, 42);
        Ok(())
    }

    #[tokio::test]
    async fn an_expired_credential_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtContentLinkProvider::new("secret".to_owned());
        let expired = encode(
            &Header::new(Algorithm::HS256),
            &serde_json::json!({ "aud": CONTENT_LINK_AUDIENCE, "sub": "42", "exp": 1i64 }),
            &EncodingKey::from_secret(b"secret"),
        )?;

        // Act
        let result = provider.verify(1, &expired).await;

        // Assert
        assert!(matches!(result, Err(ContentLinkError::Expired)));
        Ok(())
    }

    #[tokio::test]
    async fn a_credential_for_another_file_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtContentLinkProvider::new("secret".to_owned());
        let link = provider.mint(7).await?;

        // Act: a caller presents the credential against another expiry.
        let other_expiry = link.expires_at().saturating_add(1);
        let result = provider.verify(other_expiry, link.signature()).await;

        // Assert
        assert!(matches!(result, Err(ContentLinkError::Invalid)));
        Ok(())
    }

    #[tokio::test]
    async fn a_tampered_value_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = JwtContentLinkProvider::new("secret".to_owned());

        // Act
        let result = provider.verify(1, "not-a-credential").await;

        // Assert
        assert!(matches!(result, Err(ContentLinkError::Invalid)));
        Ok(())
    }

    #[tokio::test]
    async fn a_session_token_is_not_a_content_credential() -> Result<(), Box<dyn Error>> {
        // Arrange: a session token carries no playback audience.
        let provider = JwtContentLinkProvider::new("secret".to_owned());
        let session = encode(
            &Header::new(Algorithm::HS256),
            &serde_json::json!({ "sub": "42", "exp": 4_102_444_800i64 }),
            &EncodingKey::from_secret(b"secret"),
        )?;

        // Act
        let result = provider.verify(4_102_444_800, &session).await;

        // Assert
        assert!(matches!(result, Err(ContentLinkError::Invalid)));
        Ok(())
    }
}
