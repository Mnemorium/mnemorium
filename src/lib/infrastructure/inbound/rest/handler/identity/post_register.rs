use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::register_user::RegisterUserCommand;
use crate::application::port::register_user::RegisterUserError;
use crate::application::port::register_user::RegisterUserResponse;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to register a new user.
///
/// The requested `role` is granted only when the authenticated caller is
/// allowed to create it: the Root Admin can register `Admin` or `Standard`
/// users, an `Admin` user can register `Standard` users only.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct RegisterRequest {
    /// Email address of the new user, when provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Password of the new user; never returned by the API.
    #[schema(write_only, min_length = 8)]
    pub password: String,
    /// Role to grant to the new user.
    pub role: Role,
    /// Username of the new user.
    #[schema(min_length = 4, max_length = 100)]
    pub username: String,
}

/// User created by a successful registration.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct RegisterResponse {
    /// Email address of the newly registered user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the newly registered user.
    pub id: NumericID,
    /// Role granted to the newly registered user.
    pub role: Role,
    /// Username of the newly registered user.
    pub username: String,
}

/// Map the register-user response onto its HTTP representation.
impl From<RegisterUserResponse> for RegisterResponse {
    fn from(response: RegisterUserResponse) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Map a register-user error to its API error.
impl From<RegisterUserError> for ApiError {
    fn from(err: RegisterUserError) -> Self {
        match err {
            RegisterUserError::Forbidden => Self::Forbidden(err.to_string()),
            RegisterUserError::InvalidEmail
            | RegisterUserError::InvalidPassword
            | RegisterUserError::InvalidUsername => Self::BadRequest(err.to_string()),
            RegisterUserError::Unknown(_) => Self::InternalServerError,
            RegisterUserError::UserAlreadyExists => Self::Conflict(err.to_string()),
        }
    }
}

/// Register a new user on behalf of an authenticated caller.
///
/// The Root Admin (user identifier `0`) may register `Admin` or `Standard`
/// users; an `Admin` user may register `Standard` users only; any other role is
/// rejected with `403`.
///
/// # Errors
///
/// Returns [`ApiError`] when the request cannot be completed, mapping the
/// use-case errors to their HTTP responses.
#[utoipa::path(
    post,
    operation_id = "post_register",
    path = "/identity/register",
    tag = "identity",
    request_body = RegisterRequest,
    responses(
        (
            status = CREATED,
            body = RegisterResponse,
            headers(
                ("Location" = String, description = "URI of the newly registered user"),
            ),
            description = "User registered",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            description = "Insufficient privileges to register the requested role"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            description = "Username or email is already taken"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Register a new user"
)]
pub async fn post_register(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<RegisterRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .register_user()
        .execute(RegisterUserCommand::new(
            caller.user_id(),
            request.username,
            request.email,
            request.password,
            request.role,
        ))
        .await?;
    Ok((
        StatusCode::CREATED,
        [(
            header::LOCATION,
            format!("/api/v1/identity/users/{}", response.id()),
        )],
        Json(RegisterResponse::from(response)),
    ))
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

    use super::post_register;
    use crate::application::port::login_user::MockLoginUserUseCase;
    use crate::application::port::register_user::MockRegisterUserUseCase;
    use crate::application::port::register_user::RegisterUserCommand;
    use crate::application::port::register_user::RegisterUserError;
    use crate::application::port::register_user::RegisterUserResponse;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

    /// Send `body` through the endpoint router on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockRegisterUserUseCase,
        caller_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/identity/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let state = AppState::new(
            Arc::new(use_case),
            Arc::new(MockLoginUserUseCase::new()),
            Arc::new(JwtTokenProvider::new()),
        );
        let router = axum::Router::new()
            .route("/api/v1/identity/register", post(post_register))
            .with_state(state);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Location` header, `Content-Type`
    /// header and decoded JSON body.
    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, location, content_type, body))
    }

    /// Build a valid JSON request body for a `Standard` registration.
    fn request_body(username: &str, email: Option<&str>) -> Value {
        json!({
            "username": username,
            "email": email,
            "password": "super-secret!",
            "role": "STANDARD",
        })
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockRegisterUserUseCase, error: RegisterUserError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn post_register_with_email_returns_created() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = RegisterUserCommand::new(
            0,
            "alice".to_owned(),
            Some("alice@example.com".to_owned()),
            "super-secret!".to_owned(),
            Role::Standard,
        );
        let registered_user = RegisterUserResponse::new(
            1,
            "alice".to_owned(),
            Some("alice@example.com".to_owned()),
            Role::Standard,
        );
        let mut use_case = MockRegisterUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(registered_user) }));

        // Act
        let (status, location, content_type, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(request_body("alice", Some("alice@example.com")).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(location.as_deref(), Some("/api/v1/identity/users/1"));
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(
            payload,
            json!({
                "username": "alice",
                "email": "alice@example.com",
                "role": "STANDARD",
                "id": 1i64,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_without_email_returns_created() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = RegisterUserCommand::new(
            0,
            "brad".to_owned(),
            None,
            "super-secret!".to_owned(),
            Role::Standard,
        );
        let registered_user = RegisterUserResponse::new(2, "brad".to_owned(), None, Role::Standard);
        let mut use_case = MockRegisterUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(registered_user) }));

        // Act
        let (status, location, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(request_body("brad", None).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(location.as_deref(), Some("/api/v1/identity/users/2"));
        assert_eq!(
            payload,
            json!({
                "username": "brad",
                "email": null,
                "role": "STANDARD",
                "id": 2i64,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_admin_role_returns_created() -> Result<(), Box<dyn Error>> {
        // Combines the `ADMIN` role request variant with the authorization rule
        // that lets the Root Admin (identifier 0) register an `Admin` user.
        // Arrange
        let expected_command = RegisterUserCommand::new(
            0,
            "cara".to_owned(),
            None,
            "super-secret!".to_owned(),
            Role::Admin,
        );
        let registered_user = RegisterUserResponse::new(3, "cara".to_owned(), None, Role::Admin);
        let mut use_case = MockRegisterUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(registered_user) }));

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(
                    json!({
                        "username": "cara",
                        "password": "super-secret!",
                        "role": "ADMIN",
                    })
                    .to_string(),
                ),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            payload,
            json!({
                "username": "cara",
                "email": null,
                "role": "ADMIN",
                "id": 3i64,
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_admin_caller_can_register_standard() -> Result<(), Box<dyn Error>> {
        // Arrange
        let registered_user = RegisterUserResponse::new(4, "dave".to_owned(), None, Role::Standard);
        let mut use_case = MockRegisterUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 5)
            .return_once(|_| Box::pin(async { Ok(registered_user) }));

        // Act
        let (status, _, _, _) = into_parts(
            send(
                use_case,
                5,
                Body::from(request_body("dave", None).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        Ok(())
    }

    #[tokio::test]
    async fn post_register_admin_caller_register_admin_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::Forbidden);

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                5,
                Body::from(
                    json!({
                        "username": "erin",
                        "password": "super-secret!",
                        "role": "ADMIN",
                    })
                    .to_string(),
                ),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "the caller is not allowed to register the requested role" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_standard_caller_register_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::Forbidden);

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                3,
                Body::from(request_body("frank", None).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "the caller is not allowed to register the requested role" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_unknown_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::Forbidden);

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                999,
                Body::from(request_body("grace", None).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "the caller is not allowed to register the requested role" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_missing_required_field_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(
                    json!({
                        "username": "heidi",
                        "role": "STANDARD",
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
    async fn post_register_wrong_type_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(
                    json!({
                        "username": "ivan",
                        "password": 123i64,
                        "role": "STANDARD",
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
    async fn post_register_invalid_role_value_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(
                    json!({
                        "username": "judy",
                        "password": "super-secret!",
                        "role": "SUPERADMIN",
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
    async fn post_register_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let (status, _, _, payload) =
            into_parts(send(use_case, 0, Body::from("not json")).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_invalid_username_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::InvalidUsername);

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(request_body("kevin", None).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "username must be at least 4 characters long" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_invalid_email_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::InvalidEmail);

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(request_body("laura", Some("not-an-email")).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "email has an invalid format" }));
        Ok(())
    }

    #[tokio::test]
    async fn post_register_invalid_password_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::InvalidPassword);

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(request_body("mark", None).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({ "error": "password does not satisfy the password policy" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_existing_user_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::UserAlreadyExists);

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(request_body("nina", None).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            payload,
            json!({ "error": "a user with this username or email already exists" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(
            &mut use_case,
            RegisterUserError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, _, _, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(request_body("oscar", None).to_string()),
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
