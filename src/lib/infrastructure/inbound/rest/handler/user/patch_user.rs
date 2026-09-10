use std::sync::Arc;

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::update_user::UpdateUserCommand;
use crate::application::port::update_user::UpdateUserError;
use crate::application::port::update_user::UpdateUserResponse as UpdateUserResponseData;
use crate::application::port::update_user::UpdateUserUseCase;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update the profile of a user.
///
/// Every attribute is optional; an omitted attribute leaves the stored value
/// unchanged.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PatchUserRequest {
    /// New email address.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// New role. Only the Root Admin may change the role of a user.
    pub role: Option<Role>,
    /// New username; must be at least 4 characters long.
    #[schema(min_length = 4)]
    pub username: Option<String>,
}

/// Updated user returned by a successful update.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PatchUserResponse {
    /// Email address of the user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the user.
    pub id: NumericID,
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
                Self::BadRequest(err.to_string())
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
        (status = OK, body = PatchUserResponse, description = "User updated"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid user identifier or payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            description = "Caller is not an administrator, the target cannot be modified, or the role change is not allowed"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            description = "Unknown user"
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
    summary = "Update a user profile"
)]
pub async fn patch_user(
    State(update_user): State<Arc<dyn UpdateUserUseCase>>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
    payload: Result<Json<PatchUserRequest>, JsonRejection>,
) -> Result<Json<PatchUserResponse>, ApiError> {
    let Ok(user_id) = id.parse::<NumericID>() else {
        return Err(ApiError::BadRequest("invalid user identifier".to_owned()));
    };
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = update_user
        .execute(UpdateUserCommand::new(
            caller.user_id(),
            user_id,
            request.username,
            request.email,
            request.role,
        ))
        .await?;
    Ok(Json(PatchUserResponse::from(response)))
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
    use axum::routing::patch;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::patch_user;
    use crate::application::port::update_user::MockUpdateUserUseCase;
    use crate::application::port::update_user::UpdateUserCommand;
    use crate::application::port::update_user::UpdateUserError;
    use crate::application::port::update_user::UpdateUserResponse;
    use crate::application::port::update_user::UpdateUserUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

    /// Send `body` through the endpoint router on behalf of `caller_id`,
    /// targeting user `user_id`, injecting the caller the way the auth
    /// middleware does.
    async fn send(
        use_case: MockUpdateUserUseCase,
        caller_id: NumericID,
        user_id: &str,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
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
            .with_state(Arc::new(use_case) as Arc<dyn UpdateUserUseCase>);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status and decoded JSON body.
    async fn into_parts(response: Response) -> Result<(StatusCode, Value), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, body))
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
        let (status, payload) = into_parts(
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
        assert_eq!(
            payload,
            json!({
                "username": "brad-renamed",
                "email": "new@example.com",
                "role": "ADMIN",
                "id": 2i64,
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
        let (status, payload) =
            into_parts(send(use_case, 1, "2", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
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
        let (status, _) = into_parts(
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
        let (status, payload) =
            into_parts(send(use_case, 3, "2", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "only administrators may update users" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_unknown_target_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::NoSuchUser);

        // Act
        let (status, payload) =
            into_parts(send(use_case, 1, "2", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({ "error": "a user with this identifier does not exist" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::NoSuchCaller);

        // Act
        let (status, payload) =
            into_parts(send(use_case, 999, "2", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "the authenticated user does not exist" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_target_not_modifiable_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::TargetNotModifiable);

        // Act
        let (status, payload) =
            into_parts(send(use_case, 0, "0", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "the targeted user cannot be modified by this caller" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_role_change_forbidden_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::RoleChangeForbidden);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                1,
                "2",
                Body::from(json!({ "role": "STANDARD" }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "only the Root Admin may change the role of a user" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_invalid_username_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::InvalidUsername);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                0,
                "2",
                Body::from(json!({ "username": "ap" }).to_string()),
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
    async fn patch_user_invalid_email_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(&mut use_case, UpdateUserError::InvalidEmail);

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                0,
                "2",
                Body::from(json!({ "email": "not-an-email" }).to_string()),
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
    async fn patch_user_dependency_failure_returns_internal_server_error()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateUserUseCase::new();
        expect_error(
            &mut use_case,
            UpdateUserError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, payload) =
            into_parts(send(use_case, 1, "2", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateUserUseCase::new();

        // Act
        let (status, payload) =
            into_parts(send(use_case, 1, "abc", Body::from(json!({}).to_string())).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "invalid user identifier" }));
        Ok(())
    }

    #[tokio::test]
    async fn patch_user_wrong_type_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateUserUseCase::new();

        // Act
        let (status, payload) = into_parts(
            send(
                use_case,
                1,
                "2",
                Body::from(json!({ "username": 1i64 }).to_string()),
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
    async fn patch_user_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateUserUseCase::new();

        // Act
        let (status, payload) =
            into_parts(send(use_case, 1, "2", Body::from("not json")).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            payload.get("error").is_some(),
            "a bad request must carry an error message"
        );
        Ok(())
    }
}
