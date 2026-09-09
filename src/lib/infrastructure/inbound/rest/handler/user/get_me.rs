use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::get_current_user::GetCurrentUserCommand;
use crate::application::port::get_current_user::GetCurrentUserError;
use crate::application::port::get_current_user::GetCurrentUserResponse;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Current user returned by a successful lookup.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GetMeResponse {
    /// Email address of the caller, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the caller.
    pub id: NumericID,
    /// Role of the caller.
    pub role: Role,
    /// Username of the caller.
    pub username: String,
}

/// Map the current-user response onto its HTTP representation.
impl From<GetCurrentUserResponse> for GetMeResponse {
    fn from(response: GetCurrentUserResponse) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Map a current-user error to its API error.
impl From<GetCurrentUserError> for ApiError {
    fn from(err: GetCurrentUserError) -> Self {
        match err {
            GetCurrentUserError::NoSuchUser => Self::Unauthorized(err.to_string()),
            GetCurrentUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch the account of the authenticated caller.
///
/// Returns the caller's own profile — identifier, username, email and role.
#[utoipa::path(
    get,
    operation_id = "get_me",
    path = "/user/me",
    tag = "user",
    responses(
        (status = OK, body = GetMeResponse, description = "Current user"),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
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
    summary = "Fetch the current user"
)]
pub async fn get_me(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Json<GetMeResponse>, ApiError> {
    let response = state
        .get_current_user()
        .execute(GetCurrentUserCommand::new(caller.user_id()))
        .await?;
    Ok(Json(GetMeResponse::from(response)))
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

    use super::get_me;
    use crate::application::port::get_current_user::GetCurrentUserCommand;
    use crate::application::port::get_current_user::GetCurrentUserError;
    use crate::application::port::get_current_user::GetCurrentUserResponse;
    use crate::application::port::get_current_user::MockGetCurrentUserUseCase;
    use crate::application::port::login_user::MockLoginUserUseCase;
    use crate::application::port::register_user::MockRegisterUserUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

    /// Send a request through the endpoint router on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockGetCurrentUserUseCase,
        caller_id: NumericID,
    ) -> Result<Response, Box<dyn Error>> {
        let mut request = Request::builder()
            .method("GET")
            .uri("/api/v1/user/me")
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let state = AppState::new(
            Arc::new(MockRegisterUserUseCase::new()),
            Arc::new(MockLoginUserUseCase::new()),
            Arc::new(use_case),
            Arc::new(JwtTokenProvider::new("tmptmp".to_owned(), 3600)),
        );
        let router = axum::Router::new()
            .route("/api/v1/user/me", get(get_me))
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

    /// Make the mocked use case fail with `error`.
    fn expect_error(use_case: &mut MockGetCurrentUserUseCase, error: GetCurrentUserError) {
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));
    }

    #[tokio::test]
    async fn get_me_existing_caller_returns_profile() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected_command = GetCurrentUserCommand::new(1);
        let current_user = GetCurrentUserResponse::new(
            1,
            "alice".to_owned(),
            Some("alice@example.com".to_owned()),
            Role::Standard,
        );
        let mut use_case = MockGetCurrentUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| Box::pin(async { Ok(current_user) }));

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
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
    async fn get_me_caller_without_email_returns_null_email() -> Result<(), Box<dyn Error>> {
        // Arrange
        let current_user = GetCurrentUserResponse::new(2, "brad".to_owned(), None, Role::Admin);
        let mut use_case = MockGetCurrentUserUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.user_id() == 2)
            .return_once(|_| Box::pin(async { Ok(current_user) }));

        // Act
        let (status, _, payload) = into_parts(send(use_case, 2).await?).await?;

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
    async fn get_me_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetCurrentUserUseCase::new();
        expect_error(&mut use_case, GetCurrentUserError::NoSuchUser);

        // Act
        let (status, _, payload) = into_parts(send(use_case, 999).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            payload,
            json!({ "error": "the authenticated user does not exist" })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_me_dependency_failure_returns_internal_server_error() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockGetCurrentUserUseCase::new();
        expect_error(
            &mut use_case,
            GetCurrentUserError::Unknown(anyhow::anyhow!("boom")),
        );

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload, json!({ "error": "an unexpected error occurred" }));
        Ok(())
    }
}
