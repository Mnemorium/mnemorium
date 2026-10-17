use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::update_user::UpdateUserCommand;
use crate::application::port::update_user::UpdateUserError;
use crate::application::port::update_user::UpdateUserResponse as UpdateUserResponseData;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::user::parse_user_id;
use crate::infrastructure::inbound::rest::handler::user::user_self_href;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update the profile of a user.
///
/// Every attribute is optional; an omitted attribute leaves the stored value
/// unchanged.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PatchUserRequest {
    /// New email address.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// New role. Only the Root Admin may change the role of a user.
    // The raw wire value is validated by the handler so a value outside the
    // discrete list is classified as `enum_validation_failed` (`OBS-006`)
    // rather than a generic deserialization failure.
    #[schema(value_type = Option<Role>)]
    pub role: Option<String>,
    /// New username; must be at least 4 characters long.
    #[schema(min_length = 4)]
    pub username: Option<String>,
}

/// Updated user returned by a successful update.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PatchUserResponse {
    /// Email address of the user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the user.
    pub id: NumericID,
    /// Link to the updated user resource.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Role of the user.
    pub role: Role,
    /// Username of the user.
    pub username: String,
}

/// Map the updated-user response onto its HTTP representation.
impl From<UpdateUserResponseData> for PatchUserResponse {
    fn from(response: UpdateUserResponseData) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            links: SelfLinks::new(&user_self_href(response.id())),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Map an update-user error to its API error.
impl From<UpdateUserError> for ApiError {
    fn from(err: UpdateUserError) -> Self {
        match err {
            UpdateUserError::InvalidEmail | UpdateUserError::InvalidUsername => {
                Self::UnprocessableEntity(err.to_string())
            }
            UpdateUserError::NotAdmin
            | UpdateUserError::TargetNotModifiable
            | UpdateUserError::RoleChangeForbidden => Self::Forbidden(err.to_string()),
            UpdateUserError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            UpdateUserError::NoSuchUser => Self::NotFound(err.to_string()),
            UpdateUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Update the profile of a user by its identifier.
///
/// Requires an `Admin` caller. The root admin (identifier `0`) cannot be
/// updated by anyone. A non-root `Admin` may update its own profile (username
/// and email) but may not modify another admin; it may update standard users
/// of the instance. Only the Root Admin may change the role of a user.
#[utoipa::path(
    patch,
    operation_id = "update_user",
    path = "/user/{id}",
    tag = "user",
    request_body = PatchUserRequest,
    params(
        ("id" = NumericID, Path, description = "Identifier of the user to update"),
    ),
    responses(
        (status = OK, body = PatchUserResponse, content_type = "application/hal+json", description = "User updated"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid user identifier or malformed request body"
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
            description = "The request body does not match the expected schema, or the username or email fails validation"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not an administrator, the target cannot be modified, or the role change is not allowed"
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
    summary = "Update a user profile"
)]
pub async fn patch_user(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
    payload: Result<Json<PatchUserRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&id)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let role = request
        .role
        .as_deref()
        .map(|value| Role::from_wire(value).ok_or_else(|| ApiError::reject_enum("role")))
        .transpose()?;
    let response = state
        .user_use_case_factory()
        .update_user()
        .execute(UpdateUserCommand::new(
            caller.user_id(),
            user_id,
            request.username,
            request.email,
            role,
        ))
        .await?;
    Ok(hal_json(PatchUserResponse::from(response)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use axum::body::Body;

    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::patch;
    use mockall::predicate::eq;

    use serde_json::json;
    use tower::ServiceExt as _;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::patch_user;
    use crate::application::port::update_user::MockUpdateUserUseCase;
    use crate::application::port::update_user::UpdateUserCommand;
    use crate::application::port::update_user::UpdateUserError;
    use crate::application::port::update_user::UpdateUserResponse;
    use crate::application::port::update_user::UpdateUserUseCase;
    use crate::application::port::user_use_case_factory::MockUserUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_user;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

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
    /// targeting user `user_id`, injecting the caller the way the auth
    /// middleware does.
    async fn send(
        use_case: MockUpdateUserUseCase,
        caller_id: NumericID,
        user_id: &str,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockUserUseCaseFactory::new();
        factory
            .expect_update_user()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn UpdateUserUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/v1/user/{user_id}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/user/{id}", patch(patch_user))
            .with_state(app_state_with_user(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockUpdateUserUseCase, error: UpdateUserError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn patch_user_full_body_returns_updated_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = UpdateUserCommand::new(
            0,
            2,
            Some("brad-renamed".to_owned()),
            Some("new@example.com".to_owned()),
            Some(Role::Admin),
        );
        let updated_user = UpdateUserResponse::new(
            2,
            "brad-renamed".to_owned(),
            Some("new@example.com".to_owned()),
            Role::Admin,
        );
        let mut use_case = MockUpdateUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(updated_user) }));

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                0,
                "2",
                Body::from(
                    json!({
                        "username": "brad-renamed",
                        "email": "new@example.com",
                        "role": "ADMIN",
                    })
                    .to_string(),
                ),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "username": "brad-renamed",
                "email": "new@example.com",
                "role": "ADMIN",
                "id": 2i64,
                "_links": { "self": { "href": "/api/v1/user/2" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_empty_body_returns_unchanged_profile() -> Result<(), Box<dyn Error>> {
        // Combines PATCH partial semantics with the `200` success contract.
        // Arrange
        let expected_command = UpdateUserCommand::new(1, 2, None, None, None);
        let fetched_user = UpdateUserResponse::new(2, "brad".to_owned(), None, Role::Standard);
        let mut use_case = MockUpdateUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(fetched_user) }));

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "2", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
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
    async fn patch_user_only_username_returns_updated_username() -> Result<(), Box<dyn Error>> {
        // Arrange
        let fetched_user = UpdateUserResponse::new(2, "only-name".to_owned(), None, Role::Standard);
        let mut use_case = MockUpdateUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(move |command: &UpdateUserCommand| {
                command.user_id() == 2
                    && command.username() == Some("only-name")
                    && command.email().is_none()
                    && command.role().is_none()
            })
            .return_once(|_| Box::pin(async { Ok(fetched_user) }));

        // Act
        let (status, _, _) = into_parts(
            send(
                use_case,
                1,
                "2",
                Body::from(json!({ "username": "only-name" }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_non_admin_caller_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::NotAdmin);

        // Act
        let response = send(use_case, 3, "2", Body::from(json!({}).to_string())).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("only administrators may update users")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_unknown_target_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::NoSuchUser);

        // Act
        let response = send(use_case, 1, "2", Body::from(json!({}).to_string())).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("a user with this identifier does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::NoSuchCaller);

        // Act
        let response = send(use_case, 999, "2", Body::from(json!({}).to_string())).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_target_not_modifiable_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::TargetNotModifiable);

        // Act
        let response = send(use_case, 0, "0", Body::from(json!({}).to_string())).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the targeted user cannot be modified by this caller")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_role_change_forbidden_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::RoleChangeForbidden);

        // Act
        let response = send(
            use_case,
            1,
            "2",
            Body::from(json!({ "role": "STANDARD" }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("only the Root Admin may change the role of a user")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_invalid_username_returns_unprocessable_entity() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::InvalidUsername);

        // Act
        let response = send(
            use_case,
            0,
            "2",
            Body::from(json!({ "username": "ap" }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("username must be at least 4 characters long")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_invalid_email_returns_unprocessable_entity() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::InvalidEmail);

        // Act
        let response = send(
            use_case,
            0,
            "2",
            Body::from(json!({ "email": "not-an-email" }).to_string()),
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
    async fn patch_user_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(
            &mut use_case,
            UpdateUserError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let response = send(use_case, 1, "2", Body::from(json!({}).to_string())).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("an unexpected error occurred")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
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
        let use_case = MockUpdateUserUseCase::new();

        // Act
        let response = send(use_case, 1, "abc", Body::from(json!({}).to_string())).await?;

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

    #[tokio::test]
    async fn patch_user_wrong_type_returns_unprocessable_entity() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateUserUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            "2",
            Body::from(json!({ "username": 1i64 }).to_string()),
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
    async fn patch_user_invalid_role_returns_unprocessable_entity() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateUserUseCase::new();

        // Act
        let response = send(
            use_case,
            0,
            "2",
            Body::from(json!({ "role": "WIZARD" }).to_string()),
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
    async fn patch_user_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateUserUseCase::new();

        // Act
        let response = send(use_case, 1, "2", Body::from("not json")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(
            error_message_of(&response).is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }
}
