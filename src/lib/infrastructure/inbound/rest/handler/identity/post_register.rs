use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
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
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::user::user_self_href;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to register a new user.
///
/// The requested `role` is granted only when the caller holds the matching
/// permission: `user.create_admin` for an `Admin` account and `user.create`
/// for a `Standard` one. The default presets grant `user.create` to Admin and
/// Root and `user.create_admin` to the Root Admin only, and a per-user
/// override can change either.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct RegisterRequest {
    /// Email address of the new user, when provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Password of the new user; never returned by the API.
    ///
    /// Must be at least 8 characters long and contain at least one symbol
    /// (a non-alphanumeric character).
    #[schema(write_only, min_length = 8, pattern = r"[^A-Za-z0-9]")]
    pub password: String,
    /// Role to grant to the new user.
    // The raw wire value is validated by the handler so a value outside the
    // discrete list is classified as `enum_validation_failed` (`OBS-006`)
    // rather than a generic deserialization failure.
    #[schema(value_type = Role)]
    pub role: String,
    /// Username of the new user.
    ///
    /// Must be 4 to 100 characters long, contain no uppercase letter, and not begin or end with whitespace.
    #[schema(min_length = 4, max_length = 100)]
    pub username: String,
}

/// User created by a successful registration.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct RegisterResponse {
    /// Email address of the newly registered user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the newly registered user.
    pub id: NumericID,
    /// Link to the newly registered user resource.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
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
            links: SelfLinks::new(&user_self_href(response.id())),
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
            RegisterUserError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            RegisterUserError::InvalidEmail
            | RegisterUserError::InvalidPassword
            | RegisterUserError::InvalidUsername => Self::UnprocessableEntity(err.to_string()),
            RegisterUserError::Unknown(_) => Self::InternalServerError,
            RegisterUserError::UsernameAlreadyExists
            | RegisterUserError::EmailAlreadyExists
            | RegisterUserError::UserAlreadyExists => Self::Conflict(err.to_string()),
        }
    }
}

/// Register a new user on behalf of an authenticated caller.
///
/// Creating a `Standard` account requires `user.create`; creating an `Admin`
/// account requires `user.create_admin`. A caller that does not hold the
/// matching permission receives `403` and creates nothing, and a token naming
/// a deleted account receives `401`.
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
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly registered user"),
            ),
            description = "User registered",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema, or the username, email or password fails validation"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Insufficient privileges to register the requested role"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Username or email is already taken"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
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
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let role = Role::from_wire(&request.role).ok_or_else(|| ApiError::reject_enum("role"))?;
    let response = state
        .identity_use_case_factory()
        .register_user()
        .execute(RegisterUserCommand::new(
            caller.user_id(),
            request.username,
            request.email,
            request.password,
            role,
        ))
        .await?;
    let body = RegisterResponse::from(response);
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

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
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::post_register;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::register_user::MockRegisterUserUseCase;
    use crate::application::port::register_user::RegisterUserCommand;
    use crate::application::port::register_user::RegisterUserError;
    use crate::application::port::register_user::RegisterUserResponse;
    use crate::application::port::register_user::RegisterUserUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::SECRET_PASSWORD;
    use crate::test_helpers::app_state_with_identity;
    use crate::test_helpers::error_message_of;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the raw `write` and `flush` are meaningful for an in-memory capture buffer"
    )]
    impl stdio::Write for CaptureWriter {
        fn flush(&mut self) -> stdio::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> stdio::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// Hand every formatted event to a fresh clone of the shared buffer.
    #[expect(
        clippy::missing_trait_methods,
        reason = "the default `make_writer_for` already routes through `make_writer`"
    )]
    impl<'writer> fmt::MakeWriter<'writer> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// Send `body` through the endpoint router on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockRegisterUserUseCase,
        caller_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockIdentityUseCaseFactory::new();
        factory
            .expect_register_user()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn RegisterUserUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/identity/register")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/identity/register", post(post_register))
            .with_state(app_state_with_identity(Arc::new(factory))?);
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
            "password": SECRET_PASSWORD,
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

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    #[expect(
        clippy::single_call_fn,
        reason = "the module-local capture helper mirrors the pattern the other handler suites use"
    )]
    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let guard = set_default(subscriber);
        (buffer, guard)
    }

    /// Read the captured bytes back as a lossy UTF-8 string.
    #[expect(
        clippy::single_call_fn,
        reason = "the module-local read-back helper mirrors the pattern the other handler suites use"
    )]
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

    #[tokio::test]
    async fn post_register_invalid_role_logs_enum_validation_failed() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let (buffer, _capture) = capture_logs();
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let response = send(
            use_case,
            0,
            Body::from(
                json!({
                    "username": "judy",
                    "password": SECRET_PASSWORD,
                    "role": "SUPERADMIN",
                })
                .to_string(),
            ),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("event=\"enum_validation_failed\"").count(),
            1,
            "an invalid role must emit exactly one enum_validation_failed: {logs}"
        );
        assert!(
            logs.contains("field=role"),
            "the event must name the role field: {logs}"
        );
        assert!(
            !logs.contains("deserialization_failed"),
            "an invalid role must not also emit deserialization_failed: {logs}"
        );
        assert!(
            !logs.contains("SUPERADMIN"),
            "the event must not echo the rejected value: {logs}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_with_email_returns_created() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = RegisterUserCommand::new(
            0,
            "alice".to_owned(),
            Some("alice@example.com".to_owned()),
            SECRET_PASSWORD.to_owned(),
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
        assert_eq!(location.as_deref(), Some("/api/v1/user/1"));
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "username": "alice",
                "email": "alice@example.com",
                "role": "STANDARD",
                "id": 1i64,
                "_links": { "self": { "href": "/api/v1/user/1" } },
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
            SECRET_PASSWORD.to_owned(),
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
        let (status, location, content_type, payload) = into_parts(
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
        assert_eq!(location.as_deref(), Some("/api/v1/user/2"));
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "username": "brad",
                "email": null,
                "role": "STANDARD",
                "id": 2i64,
                "_links": { "self": { "href": "/api/v1/user/2" } },
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
            SECRET_PASSWORD.to_owned(),
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
        let (status, location, content_type, payload) = into_parts(
            send(
                use_case,
                0,
                Body::from(
                    json!({
                        "username": "cara",
                        "password": SECRET_PASSWORD,
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
        assert_eq!(location.as_deref(), Some("/api/v1/user/3"));
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "username": "cara",
                "email": null,
                "role": "ADMIN",
                "id": 3i64,
                "_links": { "self": { "href": "/api/v1/user/3" } },
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
        let response = send(
            use_case,
            5,
            Body::from(
                json!({
                    "username": "erin",
                    "password": SECRET_PASSWORD,
                    "role": "ADMIN",
                })
                .to_string(),
            ),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller is not allowed to register the requested role")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_standard_caller_register_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::Forbidden);

        // Act
        let response = send(
            use_case,
            3,
            Body::from(request_body("frank", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller is not allowed to register the requested role")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_unknown_caller_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::Forbidden);

        // Act
        let response = send(
            use_case,
            999,
            Body::from(request_body("grace", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller is not allowed to register the requested role")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_missing_required_field_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let response = send(
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
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            error_message_of(&response).is_some(),
            "an unprocessable entity must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_wrong_type_returns_unprocessable_entity() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let response = send(
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
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            error_message_of(&response).is_some(),
            "an unprocessable entity must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_invalid_role_value_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let response = send(
            use_case,
            0,
            Body::from(
                json!({
                    "username": "judy",
                    "password": SECRET_PASSWORD,
                    "role": "SUPERADMIN",
                })
                .to_string(),
            ),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            error_message_of(&response).is_some(),
            "an unprocessable entity must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockRegisterUserUseCase::new();

        // Act
        let response = send(use_case, 0, Body::from("not json")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            error_message_of(&response).is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_invalid_username_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::InvalidUsername);

        // Act
        let response = send(
            use_case,
            0,
            Body::from(request_body("kevin", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some(
                "username must be between 4 and 100 characters long, contain no uppercase letter, and not begin or end with whitespace"
            )
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_invalid_email_returns_unprocessable_entity() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::InvalidEmail);

        // Act
        let response = send(
            use_case,
            0,
            Body::from(request_body("laura", Some("not-an-email")).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("email has an invalid format")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_invalid_password_returns_unprocessable_entity()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::InvalidPassword);

        // Act
        let response = send(
            use_case,
            0,
            Body::from(request_body("mark", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("password does not satisfy the password policy")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_existing_user_returns_conflict() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::UserAlreadyExists);

        // Act
        let response = send(
            use_case,
            0,
            Body::from(request_body("nina", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("a user with this username or email already exists")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_duplicate_username_returns_conflict_naming_the_username()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::UsernameAlreadyExists);

        // Act
        let response = send(
            use_case,
            0,
            Body::from(request_body("nina", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("username is already taken")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_register_duplicate_email_returns_conflict_naming_the_email()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegisterUserUseCase::new();
        expect_error(&mut use_case, RegisterUserError::EmailAlreadyExists);

        // Act
        let response = send(
            use_case,
            0,
            Body::from(request_body("nina", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("email is already taken")
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
        let response = send(
            use_case,
            0,
            Body::from(request_body("oscar", None).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }
}
