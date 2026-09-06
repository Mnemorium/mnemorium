use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::login_user::LoginUserCommand;
use crate::application::port::login_user::LoginUserError;
use crate::application::port::login_user::LoginUserResponse;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;

/// Payload to authenticate a user.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct LoginRequest {
    /// Password used to authenticate.
    #[schema(write_only)]
    pub password: String,
    /// Username of the user to authenticate.
    #[schema(write_only)]
    pub username: String,
}

/// Token issued by a successful authentication.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct LoginResponse {
    /// Token to present in the `Authorization` header of subsequent requests.
    pub access_token: String,
    /// Lifetime of the token, in seconds.
    pub expires_in: u64,
    /// Token type, always `bearer`.
    pub token_type: String,
}

/// Map the login-user response onto its HTTP representation.
impl From<LoginUserResponse> for LoginResponse {
    fn from(response: LoginUserResponse) -> Self {
        Self {
            access_token: response.access_token().to_owned(),
            expires_in: response.expires_in(),
            token_type: "bearer".to_owned(),
        }
    }
}

/// Map a login-user error to its API error.
impl From<LoginUserError> for ApiError {
    fn from(err: LoginUserError) -> Self {
        match err {
            LoginUserError::InvalidPassword | LoginUserError::InvalidUsername => {
                Self::Unauthorized(err.to_string())
            }
            LoginUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Authenticate a user and issue a token.
///
/// The caller presents a username and a password. On success the endpoint
/// returns an access token to present in the `Authorization` header of
/// subsequent requests, together with the number of seconds it stays valid.
///
/// # Errors
///
/// Returns [`ApiError`] when the request cannot be completed, mapping the
/// use-case errors to their HTTP responses.
#[utoipa::path(
    post,
    operation_id = "post_login",
    path = "/identity/login",
    tag = "identity",
    request_body = LoginRequest,
    responses(
        (status = OK, body = LoginResponse, description = "User authenticated"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Invalid credentials"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            description = "Unexpected error"
        ),
    ),
    summary = "Authenticate a user"
)]
pub async fn post_login(
    State(state): State<AppState>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .login_user()
        .execute(LoginUserCommand::new(request.username, request.password))
        .await?;
    Ok(Json(LoginResponse::from(response)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_login;
    use crate::application::port::login_user::LoginUserCommand;
    use crate::application::port::login_user::LoginUserError;
    use crate::application::port::login_user::LoginUserResponse;
    use crate::application::port::login_user::MockLoginUserUseCase;
    use crate::application::port::register_user::MockRegisterUserUseCase;
    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

    /// Send `body` through the endpoint router.
    async fn send(
        login_use_case: MockLoginUserUseCase,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/identity/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;

        let state = AppState::new(
            Arc::new(MockRegisterUserUseCase::new()),
            Arc::new(login_use_case),
            Arc::new(JwtTokenProvider::new()),
        );
        let router = axum::Router::new()
            .route("/api/v1/identity/login", post(post_login))
            .with_state(state);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Content-Type` header and decoded
    /// JSON body.
    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, content_type, body))
    }

    /// Build a valid JSON request body for a login.
    fn request_body(username: &str, password: &str) -> Value {
        json!({
            "username": username,
            "password": password,
        })
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(login_use_case: &mut MockLoginUserUseCase, error: LoginUserError) {
        login_use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn post_login_valid_credentials_returns_ok() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = LoginUserCommand::new("alice".to_owned(), "super-secret".to_owned());
        let issued_token = LoginUserResponse::new("jwt-token".to_owned(), 3600);
        let mut login_use_case = MockLoginUserUseCase::new();
        login_use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(issued_token) }));

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                login_use_case,
                Body::from(request_body("alice", "super-secret").to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(
            payload,
            json!({
                "access_token": "jwt-token",
                "expires_in": 3600u64,
                "token_type": "bearer",
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_login_missing_required_field_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let login_use_case = MockLoginUserUseCase::new();

        // Act
        let (status, _, payload) = into_parts(
            send(
                login_use_case,
                Body::from(
                    json!({
                        "username": "alice",
                    })
                    .to_string(),
                ),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_login_wrong_type_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let login_use_case = MockLoginUserUseCase::new();

        // Act
        let (status, _, payload) = into_parts(
            send(
                login_use_case,
                Body::from(
                    json!({
                        "username": "alice",
                        "password": 123i64,
                    })
                    .to_string(),
                ),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_login_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let login_use_case = MockLoginUserUseCase::new();

        // Act
        let (status, _, payload) =
            into_parts(send(login_use_case, Body::from("not json")).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_login_unknown_username_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut login_use_case = MockLoginUserUseCase::new();
        expect_error(&mut login_use_case, LoginUserError::InvalidUsername);

        // Act
        let (status, _, payload) = into_parts(
            send(
                login_use_case,
                Body::from(request_body("ghost", "super-secret").to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "no user matches the provided username" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_login_wrong_password_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut login_use_case = MockLoginUserUseCase::new();
        expect_error(&mut login_use_case, LoginUserError::InvalidPassword);

        // Act
        let (status, _, payload) = into_parts(
            send(
                login_use_case,
                Body::from(request_body("alice", "wrong-password").to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "the password does not match the stored hash" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_login_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut login_use_case = MockLoginUserUseCase::new();
        expect_error(
            &mut login_use_case,
            LoginUserError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, _, payload) = into_parts(
            send(
                login_use_case,
                Body::from(request_body("alice", "super-secret").to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }
}
