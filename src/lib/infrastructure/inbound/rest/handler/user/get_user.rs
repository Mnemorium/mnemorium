use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_user::GetUserCommand;
use crate::application::port::get_user::GetUserError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::user::GetUserResponse;
use crate::infrastructure::inbound::rest::handler::user::parse_user_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a fetch-user error to its API error.
impl From<GetUserError> for ApiError {
    fn from(err: GetUserError) -> Self {
        match err {
            GetUserError::NotAdmin => Self::Forbidden(err.to_string()),
            GetUserError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetUserError::NoSuchUser => Self::NotFound(err.to_string()),
            GetUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch a user by its identifier.
///
/// Requires an `Admin` caller. Returns the profile — identifier, username,
/// email and role — of the requested user.
#[utoipa::path(
    get,
    operation_id = "get_user",
    path = "/user/{id}",
    tag = "user",
    params(
        ("id" = NumericID, Path, description = "Identifier of the user to fetch"),
    ),
    responses(
        (status = OK, body = GetUserResponse, content_type = "application/hal+json", description = "User found"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid user identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not an administrator"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown user"
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
    summary = "Fetch a user by identifier"
)]
pub async fn get_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&id)?;
    let response = state
        .user_use_case_factory()
        .get_user()
        .execute(GetUserCommand::new(caller.user_id(), user_id))
        .await?;
    Ok(hal_json(GetUserResponse::from(response)))
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
    use axum::routing::get;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::get_user;
    use crate::application::port::get_user::GetUserCommand;
    use crate::application::port::get_user::GetUserError;
    use crate::application::port::get_user::GetUserResponse;
    use crate::application::port::get_user::GetUserUseCase;
    use crate::application::port::get_user::MockGetUserUseCase;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_user;
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

    /// Send a GET to `/api/v1/user/{id}` on behalf of `caller_id`, injecting
    /// the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockGetUserUseCase,
        caller_id: NumericID,
        id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockUserUseCaseFactory::new();
        factory
            .expect_get_user()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetUserUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/user/{id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/user/{id}", get(get_user))
            .with_state(app_state_with_user(Arc::new(factory))?);
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

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockGetUserUseCase, error: GetUserError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn get_user_existing_user_returns_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = GetUserCommand::new(1, 2);
        let fetched_user = GetUserResponse::new(
            2,
            "brad".to_owned(),
            Some("brad@example.com".to_owned()),
            Role::Standard,
        );
        let mut use_case = MockGetUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(fetched_user) }));

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "username": "brad",
                "email": "brad@example.com",
                "role": "STANDARD",
                "id": 2i64,
                "_links": { "self": { "href": "/api/v1/user/2" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_target_without_email_returns_null_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let fetched_user = GetUserResponse::new(2, "brad".to_owned(), None, Role::Admin);
        let mut use_case = MockGetUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command: &GetUserCommand| command.user_id() == 2)
            .return_once(|_| Box::pin(async { Ok(fetched_user) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            payload,
            json!({
                "username": "brad",
                "email": null,
                "role": "ADMIN",
                "id": 2i64,
                "_links": { "self": { "href": "/api/v1/user/2" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_non_admin_caller_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUserUseCase::new();
        expect_error(&mut use_case, GetUserError::NotAdmin);

        // Act
        let response = send(use_case, 3, "2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("only administrators may fetch other users")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_target_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUserUseCase::new();
        expect_error(&mut use_case, GetUserError::NoSuchUser);

        // Act
        let response = send(use_case, 1, "2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("a user with this identifier does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUserUseCase::new();
        expect_error(&mut use_case, GetUserError::NoSuchCaller);

        // Act
        let response = send(use_case, 999, "2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUserUseCase::new();
        expect_error(
            &mut use_case,
            GetUserError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, "2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        // Install a capturing subscriber on the current thread. The guard must
        // stay alive for the whole request: `#[tokio::test]` runs on a
        // current-thread runtime, so the awaited request is polled on this
        // thread and sees the scoped default dispatcher.
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let _capture = set_default(subscriber);
        let use_case = MockGetUserUseCase::new();

        // Act
        let response = send(use_case, 1, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid user identifier")
        );
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        let logs = String::from_utf8_lossy(bytes.as_slice()).into_owned();
        assert_eq!(
            logs.matches("WARN").count(),
            1,
            "the malformed identifier must emit exactly one warn event: {logs}"
        );
        assert_eq!(
            logs.matches("event=\"input_validation_failed\"").count(),
            1,
            "the malformed identifier must emit exactly one input_validation_failed event: {logs}"
        );
        assert!(
            logs.contains("security"),
            "the event must target the reserved security target: {logs}"
        );
        assert!(
            logs.contains("field=\"user_id\""),
            "the event must name the rejected field: {logs}"
        );
        assert!(
            logs.contains("reason=\"invalid_identifier\""),
            "the event must classify the reason as a malformed identifier: {logs}"
        );
        assert!(
            !logs.contains("abc"),
            "the event must not echo the rejected identifier: {logs}"
        );
        Ok(())
    }
}
