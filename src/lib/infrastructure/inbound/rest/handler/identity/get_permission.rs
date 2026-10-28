use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_permission::GetPermissionCommand;
use crate::application::port::get_permission::GetPermissionError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::identity::PermissionResponse;
use crate::infrastructure::inbound::rest::handler::user::parse_user_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a fetch-permission error to its API error.
impl From<GetPermissionError> for ApiError {
    fn from(err: GetPermissionError) -> Self {
        match err {
            GetPermissionError::Forbidden => Self::Forbidden(err.to_string()),
            GetPermissionError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetPermissionError::NoSuchUser => Self::NotFound(err.to_string()),
            GetPermissionError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Read a user's effective permissions.
///
/// Requires the Root Admin. Returns one entry per configurable action: the
/// scope it resolves to, whether a stored override supplies it, and the
/// aggregates a `LISTED` scope names.
#[utoipa::path(
    get,
    operation_id = "get_permission",
    path = "/user/{user_id}/permission",
    tag = "identity",
    params(
        ("user_id" = NumericID, Path, description = "Identifier of the user whose permissions are read"),
    ),
    responses(
        (status = OK, body = PermissionResponse, content_type = "application/hal+json", description = "Effective permissions"),
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
            description = "Missing or invalid credentials, or a deleted caller"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not the Root Admin, or the target is the Root Admin"
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
    summary = "Read a user's effective permissions"
)]
pub async fn get_permission(
    Path(user_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&user_id_value)?;
    let response = state
        .identity_use_case_factory()
        .get_permission()
        .execute(GetPermissionCommand::new(caller.user_id(), user_id))
        .await?;
    Ok(hal_json(PermissionResponse::from(response)))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::indexing_slicing,
        reason = "the JSON fixture shape is fixed by the assertion"
    )]

    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use mockall::predicate::eq;
    use tower::ServiceExt as _;

    use crate::application::port::get_permission::GetPermissionCommand;
    use crate::application::port::get_permission::GetPermissionError;
    use crate::application::port::get_permission::GetPermissionResponse;
    use crate::application::port::get_permission::GetPermissionUseCase;
    use crate::application::port::get_permission::MockGetPermissionUseCase;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::PermissionPreset;
    use crate::domain::model::permission::Scope;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_identity;
    use crate::test_helpers::into_status_and_body;

    use super::get_permission;

    /// Send a GET to the permission route on behalf of `caller_id`.
    async fn send(
        use_case: MockGetPermissionUseCase,
        caller_id: NumericID,
        user_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockIdentityUseCaseFactory::new();
        factory
            .expect_get_permission()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetPermissionUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/user/{user_id}/permission"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/user/{user_id}/permission", get(get_permission))
            .with_state(app_state_with_identity(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_permission_root_admin_returns_effective_permissions() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let expected_command = GetPermissionCommand::new(0, 5);
        let mut use_case = MockGetPermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetPermissionResponse::new(
                        5,
                        Role::Standard,
                        vec![PermissionPreset::new(Action::CatalogRead, Scope::All)],
                        Vec::new(),
                    ))
                })
            });

        // Act
        let (status, body) = into_status_and_body(send(use_case, 0, "5").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user_id"], 5i64);
        assert_eq!(body["role"], "STANDARD");
        assert_eq!(body["defaults"][0]["action"], "catalog.read");
        assert_eq!(body["defaults"][0]["scope"], "ALL");
        assert_eq!(body["overrides"].as_array().map(Vec::len), Some(0));
        assert_eq!(body["_links"]["self"]["href"], "/api/v1/user/5/permission");
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_non_root_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetPermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetPermissionError::Forbidden) }));

        // Act
        let response = send(use_case, 1, "5").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_unknown_user_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetPermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetPermissionError::NoSuchUser) }));

        // Act
        let response = send(use_case, 0, "5").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_deleted_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetPermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetPermissionError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 0, "5").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn get_permission_malformed_user_id_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetPermissionUseCase::new();

        // Act
        let response = send(use_case, 0, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }
}
