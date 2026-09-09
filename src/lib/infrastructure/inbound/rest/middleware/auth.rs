use std::future::Future;
use std::future::ready;
use std::sync::Arc;

use axum::Json;
use axum::extract::FromRequestParts;
use axum::extract::{Request, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};
use serde_json::json;

use crate::domain::alias::NumericID;
use crate::domain::port::token_provider::TokenProvider;
use crate::infrastructure::inbound::rest::api_error::ApiError;

/// Authenticated user identifier attached to a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedUser(NumericID);

impl AuthenticatedUser {
    /// Return the authenticated user identifier.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.0
    }
}

impl From<NumericID> for AuthenticatedUser {
    fn from(user_id: NumericID) -> Self {
        Self(user_id)
    }
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        ready(
            parts
                .extensions
                .get::<AuthenticatedUser>()
                .copied()
                .ok_or(ApiError::InternalServerError),
        )
    }
}

/// Require a valid `Bearer` token and attach the authenticated user to the
/// request.
pub async fn authenticate<P>(
    State(token_provider): State<Arc<P>>,
    mut request: Request,
    next: Next,
) -> Response
where
    P: TokenProvider,
{
    let Some(token) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .and_then(|(scheme, token)| scheme.eq_ignore_ascii_case("bearer").then_some(token))
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "missing or malformed authorization header" })),
        )
            .into_response();
    };
    let Some(user_id) = token_provider.validate(token).await.ok() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "invalid or expired token" })),
        )
            .into_response();
    };
    request
        .extensions_mut()
        .insert(AuthenticatedUser::from(user_id));
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::Json;
    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::HeaderValue;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::middleware;
    use axum::response::Response;
    use axum::routing::get;
    use rstest::rstest;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::AuthenticatedUser;
    use super::authenticate;
    use crate::domain::alias::NumericID;
    use crate::domain::port::error::TokenProviderError;
    use crate::domain::port::token_provider::MockTokenProvider;
    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

    /// Echo the caller identifier the extractor recovered from the request.
    async fn stub(caller: AuthenticatedUser) -> Json<Value> {
        Json(json!({ "user_id": caller.user_id() }))
    }

    /// Application state with a real token provider that is never consulted
    /// (the middleware layer carries its own mock).
    fn state() -> AppState {
        AppState::new(Arc::new(JwtTokenProvider::new("tmptmp".to_owned(), 3600)))
    }

    /// Router guarding `stub` with `authenticate` over a mocked provider.
    fn router_with(provider: MockTokenProvider) -> axum::Router {
        axum::Router::new()
            .route("/", get(stub))
            .route_layer(middleware::from_fn_with_state(
                Arc::new(provider),
                authenticate::<MockTokenProvider>,
            ))
            .with_state(state())
    }

    /// Send a GET to `router` carrying `authorization` as the
    /// `Authorization` header value, when present.
    async fn send(
        router: axum::Router,
        authorization: Option<HeaderValue>,
    ) -> Result<Response, Box<dyn Error>> {
        let mut builder = Request::builder().method("GET").uri("/");
        if let Some(value) = authorization {
            builder = builder.header(header::AUTHORIZATION, value);
        }
        let request = builder.body(Body::empty())?;
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status and decoded JSON body.
    async fn into_parts(response: Response) -> Result<(StatusCode, Value), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, body))
    }

    /// Make the mocked provider accept `token` for `user_id`.
    fn expect_valid_token(
        provider: &mut MockTokenProvider,
        token: &'static str,
        user_id: NumericID,
    ) {
        provider
            .expect_validate()
            .times(1)
            .withf(move |candidate: &str| candidate == token)
            .returning(move |_| Box::pin(async move { Ok(user_id) }));
    }

    #[tokio::test]
    async fn authenticate_valid_bearer_token_calls_next_with_authenticated_user()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut provider = MockTokenProvider::new();
        expect_valid_token(&mut provider, "valid-token", 42);

        // Act
        let (status, payload) = into_parts(
            send(
                router_with(provider),
                Some(HeaderValue::from_static("Bearer valid-token")),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload, json!({ "user_id": 42i64 }));
        Ok(())
    }

    #[tokio::test]
    async fn authenticate_missing_authorization_header_returns_missing_or_malformed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = MockTokenProvider::new();

        // Act
        let (status, payload) = into_parts(send(router_with(provider), None).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "missing or malformed authorization header" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn authenticate_other_scheme_returns_missing_or_malformed() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let provider = MockTokenProvider::new();

        // Act
        let (status, payload) = into_parts(
            send(
                router_with(provider),
                Some(HeaderValue::from_static("Basic dXNlcjpwYXNz")),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "missing or malformed authorization header" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn authenticate_header_without_space_returns_missing_or_malformed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = MockTokenProvider::new();

        // Act
        let (status, payload) = into_parts(
            send(
                router_with(provider),
                Some(HeaderValue::from_static("Bearertoken")),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "missing or malformed authorization header" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn authenticate_non_utf8_header_returns_missing_or_malformed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let provider = MockTokenProvider::new();
        let value = HeaderValue::from_bytes(&[b'B', b'e', b'a', b'r', b'e', b'r', b' ', 0xff])?;

        // Act
        let (status, payload) = into_parts(send(router_with(provider), Some(value)).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "missing or malformed authorization header" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn authenticate_lowercase_bearer_scheme_is_accepted() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut provider = MockTokenProvider::new();
        expect_valid_token(&mut provider, "valid-token", 7);

        // Act
        let (status, payload) = into_parts(
            send(
                router_with(provider),
                Some(HeaderValue::from_static("bearer valid-token")),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload, json!({ "user_id": 7i64 }));
        Ok(())
    }

    #[rstest]
    #[case::invalid_claims(TokenProviderError::InvalidClaims)]
    #[case::invalid_token(TokenProviderError::InvalidToken)]
    #[case::operation_failed(TokenProviderError::OperationFailed)]
    #[case::token_expired(TokenProviderError::TokenExpired)]
    #[case::unknown(TokenProviderError::Unknown(anyhow::anyhow!("boom")))]
    #[tokio::test]
    async fn authenticate_provider_rejection_returns_invalid_or_expired(
        #[case] error: TokenProviderError,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut provider = MockTokenProvider::new();
        provider
            .expect_validate()
            .times(1)
            .return_once(move |_| Box::pin(async move { Err(error) }));

        // Act
        let (status, payload) = into_parts(
            send(
                router_with(provider),
                Some(HeaderValue::from_static("Bearer expired-token")),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(payload, json!({ "error": "invalid or expired token" }));
        Ok(())
    }

    #[tokio::test]
    async fn authenticate_missing_extension_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let router = axum::Router::new()
            .route("/", get(stub))
            .with_state(state());

        // Act
        let (status, payload) = into_parts(send(router, None).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }

    #[tokio::test]
    async fn authenticate_blank_token_is_forwarded_to_provider() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut provider = MockTokenProvider::new();
        provider
            .expect_validate()
            .times(1)
            .withf(|candidate: &str| candidate.is_empty())
            .return_once(|_| Box::pin(async { Err(TokenProviderError::InvalidToken) }));

        // Act
        let (status, payload) = into_parts(
            send(
                router_with(provider),
                Some(HeaderValue::from_static("Bearer ")),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(payload, json!({ "error": "invalid or expired token" }));
        Ok(())
    }
}
