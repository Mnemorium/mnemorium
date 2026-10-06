#![allow(
    clippy::missing_errors_doc,
    reason = "Handler are declared in the OpenAPI spec"
)]

pub mod asset;
pub mod get_health;
pub mod identity;
pub mod user;

use axum::middleware;
use axum::routing::get;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
use crate::infrastructure::inbound::rest::handler::get_health::get_health;
use crate::infrastructure::inbound::rest::handler::identity::identity_routes;
use crate::infrastructure::inbound::rest::handler::user::user_routes;
use crate::infrastructure::inbound::rest::middleware::hal_errors::hal_errors;
use crate::infrastructure::inbound::rest::middleware::rate_limit::LoginRateLimiter;
use crate::infrastructure::inbound::rest::middleware::trace::tracing;

/// Build the application router.
///
/// This is the single composition root: the server mounts only this router, and
/// `hal_errors` is applied **last** so it is the outermost layer. It therefore
/// rewraps every `4xx`/`5xx` the server emits, including the routing fallback
/// (an unknown or malformed URL) and framework rejections, as the one HAL error
/// envelope (`API-039`). It must remain outermost.
///
/// The `limiter` is the process-lifetime login rate limiter the server built;
/// it is applied to the `login` route here.
pub fn setup_routes(state: &AppState, limiter: &LoginRateLimiter) -> axum::Router {
    let v1 = axum::Router::new()
        .nest("/asset", asset_routes(state))
        .nest("/identity", identity_routes(state, limiter))
        .nest("/user", user_routes(state));

    axum::Router::new()
        .route("/health", get(get_health))
        .nest("/api/v1", v1)
        .layer(middleware::from_fn(tracing))
        .layer(middleware::from_fn(hal_errors))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::ConnectInfo;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::setup_routes;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::login_user::LoginUserError;
    use crate::application::port::login_user::LoginUserResponse;
    use crate::application::port::login_user::LoginUserUseCase;
    use crate::application::port::login_user::MockLoginUserUseCase;
    use crate::infrastructure::inbound::rest::middleware::rate_limit::LoginRateLimiter;
    use crate::test_helpers::SECRET_PASSWORD;
    use crate::test_helpers::app_state;
    use crate::test_helpers::app_state_with_identity;

    /// Build a request with `method` and `uri`, optionally carrying a JSON body.
    ///
    /// The request always carries a `ConnectInfo` peer address, so the login
    /// rate limiter can extract a client key under the default (peer IP)
    /// configuration.
    fn request(method: &str, uri: &str, body: Option<&str>) -> Result<Request, Box<dyn Error>> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))));
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        let request_body = body.map_or_else(Body::empty, |text| Body::from(text.to_owned()));
        Ok(builder.body(request_body)?)
    }

    /// Assert `response` is the HAL error envelope for `status` at `href` and
    /// return the error message, so the caller can assert it.
    async fn assert_envelope(
        response: Response,
        status: StatusCode,
        href: &str,
    ) -> Result<String, Box<dyn Error>> {
        assert_eq!(response.status(), status);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/hal+json"),
            "every error must be served as the HAL media type (`API-031`)"
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: Value = serde_json::from_slice(&bytes)?;
        let object = payload
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("the error envelope must be a JSON object"))?;
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["_links", "error"],
            "the envelope must carry exactly `error` and `_links`"
        );
        let message = payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !message.is_empty(),
            "the envelope must carry a non-empty message"
        );
        let actual_href = payload
            .get("_links")
            .and_then(|links| links.get("self"))
            .and_then(|self_link| self_link.get("href"))
            .and_then(Value::as_str);
        assert_eq!(actual_href, Some(href));
        Ok(message.to_owned())
    }

    /// A router whose token provider is built from the fixture configuration.
    fn plain_router() -> Result<axum::Router, Box<dyn Error>> {
        let state = app_state()?;
        let limiter = LoginRateLimiter::new(state.configuration().load().security().rate_limit());
        Ok(setup_routes(&state, &limiter))
    }

    #[tokio::test]
    async fn unknown_route_is_wrapped_and_drops_the_query_string() -> Result<(), Box<dyn Error>> {
        // Act
        let response = plain_router()?
            .oneshot(request("GET", "/api/v1/does-not-exist?x=1", None)?)
            .await?;

        // Assert
        assert_eq!(
            assert_envelope(response, StatusCode::NOT_FOUND, "/api/v1/does-not-exist").await?,
            "Not Found"
        );
        Ok(())
    }

    #[tokio::test]
    async fn method_not_allowed_is_wrapped_and_keeps_allow() -> Result<(), Box<dyn Error>> {
        // Act
        let response = plain_router()?
            .oneshot(request("DELETE", "/health", None)?)
            .await?;

        // Assert
        assert!(
            response.headers().get(header::ALLOW).is_some(),
            "the Allow header must survive the rewrite"
        );
        assert_envelope(response, StatusCode::METHOD_NOT_ALLOWED, "/health").await?;
        Ok(())
    }

    #[tokio::test]
    async fn extractor_rejection_is_wrapped() -> Result<(), Box<dyn Error>> {
        // Act
        let response = plain_router()?
            .oneshot(request("POST", "/api/v1/identity/login", Some("not json"))?)
            .await?;

        // Assert
        assert_envelope(response, StatusCode::BAD_REQUEST, "/api/v1/identity/login").await?;
        Ok(())
    }

    #[tokio::test]
    async fn authentication_failure_is_wrapped() -> Result<(), Box<dyn Error>> {
        // Act
        let response = plain_router()?
            .oneshot(request("GET", "/api/v1/user/me", None)?)
            .await?;

        // Assert
        assert_eq!(
            assert_envelope(response, StatusCode::UNAUTHORIZED, "/api/v1/user/me").await?,
            "missing or malformed authorization header"
        );
        Ok(())
    }

    #[tokio::test]
    async fn use_case_error_is_wrapped() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut login_use_case = MockLoginUserUseCase::new();
        login_use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(LoginUserError::InvalidPassword) }));
        let mut factory = MockIdentityUseCaseFactory::new();
        factory
            .expect_login_user()
            .times(0..=1)
            .return_once(move || Arc::new(login_use_case) as Arc<dyn LoginUserUseCase>);
        let state = app_state_with_identity(Arc::new(factory))?;
        let limiter = LoginRateLimiter::new(state.configuration().load().security().rate_limit());

        // Act
        let body = json!({
            "username": "alice",
            "password": SECRET_PASSWORD,
        })
        .to_string();
        let response = setup_routes(&state, &limiter)
            .oneshot(request("POST", "/api/v1/identity/login", Some(&body))?)
            .await?;

        // Assert
        assert_eq!(
            assert_envelope(response, StatusCode::UNAUTHORIZED, "/api/v1/identity/login").await?,
            "invalid credentials"
        );
        Ok(())
    }

    #[tokio::test]
    async fn login_rate_limit_rejects_after_the_burst() -> Result<(), Box<dyn Error>> {
        // Arrange: every call succeeds, so only the rate limiter can reject.
        let mut factory = MockIdentityUseCaseFactory::new();
        factory.expect_login_user().times(5).returning(|| {
            let mut login_use_case = MockLoginUserUseCase::new();
            login_use_case.expect_execute().returning(|_| {
                Box::pin(async { Ok(LoginUserResponse::new("jwt-token".to_owned(), 3600)) })
            });
            Arc::new(login_use_case) as Arc<dyn LoginUserUseCase>
        });
        let state = app_state_with_identity(Arc::new(factory))?;
        let limiter = LoginRateLimiter::new(state.configuration().load().security().rate_limit());
        let router = setup_routes(&state, &limiter);
        let body = json!({ "username": "alice", "password": SECRET_PASSWORD }).to_string();

        // Act: spend the default burst of five, then one more.
        for _ in 0u8..5u8 {
            let response = router
                .clone()
                .oneshot(request("POST", "/api/v1/identity/login", Some(&body))?)
                .await?;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "a request within the burst must succeed"
            );
        }
        let rejected = router
            .oneshot(request("POST", "/api/v1/identity/login", Some(&body))?)
            .await?;

        // Assert
        assert!(
            rejected.headers().get(header::RETRY_AFTER).is_some(),
            "a rate-limited response must carry Retry-After (`API-027`)"
        );
        assert_eq!(
            assert_envelope(
                rejected,
                StatusCode::TOO_MANY_REQUESTS,
                "/api/v1/identity/login",
            )
            .await?,
            "too many requests"
        );
        Ok(())
    }
}
