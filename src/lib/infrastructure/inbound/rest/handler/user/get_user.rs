use std::sync::Arc;

use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::get_user::GetUserCommand;
use crate::application::port::get_user::GetUserError;
use crate::application::port::get_user::GetUserResponse as GetUserResponseData;
use crate::application::port::get_user::GetUserUseCase;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// User returned by a successful lookup.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GetUserResponse {
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

/// Map the fetched-user response onto its HTTP representation.
impl From<GetUserResponseData> for GetUserResponse {
    fn from(response: GetUserResponseData) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

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
        (status = OK, body = GetUserResponse, description = "User found"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid user identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            description = "Caller is not an administrator"
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
    summary = "Fetch a user by identifier"
)]
pub async fn get_user(
    Path(id): Path<String>,
    State(get_user): State<Arc<dyn GetUserUseCase>>,
    caller: AuthenticatedUser,
) -> Result<Json<GetUserResponse>, ApiError> {
    let Ok(user_id) = id.parse::<NumericID>() else {
        return Err(ApiError::BadRequest("invalid user identifier".to_owned()));
    };
    let response = get_user
        .execute(GetUserCommand::new(caller.user_id(), user_id))
        .await?;
    Ok(Json(GetUserResponse::from(response)))
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
    use axum::routing::get;
    use mockall::predicate::eq;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_user;
    use crate::application::port::get_user::GetUserCommand;
    use crate::application::port::get_user::GetUserError;
    use crate::application::port::get_user::GetUserResponse;
    use crate::application::port::get_user::GetUserUseCase;
    use crate::application::port::get_user::MockGetUserUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

    /// Send a GET to `/api/v1/user/{id}` on behalf of `caller_id`, injecting
    /// the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockGetUserUseCase,
        caller_id: NumericID,
        id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/user/{id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/user/{id}", get(get_user))
            .with_state(Arc::new(use_case) as Arc<dyn GetUserUseCase>);
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
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(
            payload,
            json!({
                "username": "brad",
                "email": "brad@example.com",
                "role": "STANDARD",
                "id": 2i64,
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
        let (status, _, payload) = into_parts(send(use_case, 3, "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            payload,
            json!({ "error": "only administrators may fetch other users" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_target_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUserUseCase::new();
        expect_error(&mut use_case, GetUserError::NoSuchUser);

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({ "error": "a user with this identifier does not exist" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_user_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetUserUseCase::new();
        expect_error(&mut use_case, GetUserError::NoSuchCaller);

        // Act
        let (status, _, payload) = into_parts(send(use_case, 999, "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "the authenticated user does not exist" })
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
        let (status, _, payload) = into_parts(send(use_case, 1, "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }

    #[tokio::test]
    async fn get_user_invalid_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetUserUseCase::new();

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "abc").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(payload, json!({ "error": "invalid user identifier" }));
        Ok(())
    }
}
