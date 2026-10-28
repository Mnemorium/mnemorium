use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;

use crate::application::port::replace_permission::PermissionRequest as PermissionRequestData;
use crate::application::port::replace_permission::ReplacePermissionCommand;
use crate::application::port::replace_permission::ReplacePermissionError;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::identity::PermissionRequest;
use crate::infrastructure::inbound::rest::handler::identity::PermissionResponse;
use crate::infrastructure::inbound::rest::handler::user::parse_user_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a replace-permission error to its API error.
impl From<ReplacePermissionError> for ApiError {
    fn from(err: ReplacePermissionError) -> Self {
        match err {
            ReplacePermissionError::DuplicateAction | ReplacePermissionError::InvalidPermission => {
                Self::UnprocessableEntity(err.to_string())
            }
            ReplacePermissionError::Forbidden => Self::Forbidden(err.to_string()),
            ReplacePermissionError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ReplacePermissionError::NoSuchUser => Self::NotFound(err.to_string()),
            ReplacePermissionError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Replace a user's permission overrides.
///
/// Requires the Root Admin. The body carries the complete override set the
/// user must end with: an action absent from the body falls back to its role
/// preset. The request is validated as a whole and applied atomically.
#[utoipa::path(
    put,
    operation_id = "put_permission",
    path = "/user/{user_id}/permission",
    tag = "identity",
    request_body = PermissionRequest,
    params(
        ("user_id" = NumericID, Path, description = "Identifier of the user whose permissions are replaced"),
    ),
    responses(
        (status = OK, body = PermissionResponse, content_type = "application/hal+json", description = "Permissions replaced"),
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
            description = "Missing or invalid credentials, or a deleted caller"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not the Root Admin, or the target is the Root Admin"
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
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown user"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "An action, scope or listed resource is unknown, or a permission is invalid"
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
    summary = "Replace a user's permission overrides"
)]
pub async fn put_permission(
    Path(user_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PermissionRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&user_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let permissions = request
        .permissions
        .into_iter()
        .map(PermissionRequestData::try_from)
        .collect::<Result<Vec<_>, ApiError>>()?;
    let response = state
        .identity_use_case_factory()
        .replace_permission()
        .execute(ReplacePermissionCommand::new(
            caller.user_id(),
            user_id,
            permissions,
        ))
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
    use axum::routing::put;
    use mockall::predicate::eq;
    use serde_json::json;
    use tower::ServiceExt as _;

    use crate::application::port::get_permission::GetPermissionResponse;
    use crate::application::port::identity_use_case_factory::MockIdentityUseCaseFactory;
    use crate::application::port::replace_permission::MockReplacePermissionUseCase;
    use crate::application::port::replace_permission::PermissionRequest as PermissionRequestData;
    use crate::application::port::replace_permission::ReplacePermissionCommand;
    use crate::application::port::replace_permission::ReplacePermissionError;
    use crate::application::port::replace_permission::ReplacePermissionUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::permission::Action;
    use crate::domain::model::permission::Permission;
    use crate::domain::model::permission::PermissionPreset;
    use crate::domain::model::permission::ResourceKind;
    use crate::domain::model::permission::ResourceRef;
    use crate::domain::model::permission::Scope;
    use crate::domain::model::user::Role;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_identity;
    use crate::test_helpers::into_status_and_body;

    use super::put_permission;

    /// Send a PUT to the permission route on behalf of `caller_id`.
    async fn send(
        use_case: MockReplacePermissionUseCase,
        caller_id: NumericID,
        user_id: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockIdentityUseCaseFactory::new();
        factory
            .expect_replace_permission()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ReplacePermissionUseCase>);

        let mut request = Request::builder()
            .method("PUT")
            .uri(format!("/api/v1/user/{user_id}/permission"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/user/{user_id}/permission", put(put_permission))
            .with_state(app_state_with_identity(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn put_permission_valid_set_returns_effective_permissions() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let expected_command = ReplacePermissionCommand::new(
            0,
            5,
            vec![PermissionRequestData::new(
                Action::PlaylistRead,
                Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicPlaylist, 3)]),
            )],
        );
        let stored = Permission::try_new(
            5,
            Action::PlaylistRead,
            Scope::Listed(vec![ResourceRef::new(ResourceKind::MusicPlaylist, 3)]),
        )?;
        let mut use_case = MockReplacePermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(expected_command))
            .return_once(move |_| {
                let override_permission = stored.clone();
                Box::pin(async move {
                    Ok(GetPermissionResponse::new(
                        5i64,
                        Role::Standard,
                        vec![PermissionPreset::new(
                            Action::PlaylistRead,
                            Scope::PublicOrOwned,
                        )],
                        vec![override_permission],
                    ))
                })
            });
        let body = json!({
            "permissions": [{
                "action": "playlist.read",
                "scope": "LISTED",
                "resources": [{ "kind": "MUSIC_PLAYLIST", "id": 3i64 }]
            }]
        })
        .to_string();

        // Act
        let (status, payload) = into_status_and_body(send(use_case, 0, "5", &body).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["defaults"][0]["action"], "playlist.read");
        assert_eq!(payload["defaults"][0]["scope"], "PUBLIC_OR_OWNED");
        assert_eq!(payload["overrides"][0]["action"], "playlist.read");
        assert_eq!(payload["overrides"][0]["scope"], "LISTED");
        assert_eq!(payload["overrides"][0]["resources"][0]["id"], 3i64);
        Ok(())
    }

    #[tokio::test]
    async fn put_permission_unknown_action_returns_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockReplacePermissionUseCase::new();
        let body = json!({
            "permissions": [{ "action": "bogus.read", "scope": "ALL", "resources": [] }]
        })
        .to_string();

        // Act
        let response = send(use_case, 0, "5", &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn put_permission_unknown_scope_returns_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockReplacePermissionUseCase::new();
        let body = json!({
            "permissions": [{ "action": "gallery.read", "scope": "BOGUS", "resources": [] }]
        })
        .to_string();

        // Act
        let response = send(use_case, 0, "5", &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn put_permission_unknown_resource_kind_returns_unprocessable()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockReplacePermissionUseCase::new();
        let body = json!({
            "permissions": [{
                "action": "gallery.read",
                "scope": "LISTED",
                "resources": [{ "kind": "BOGUS", "id": 3i64 }]
            }]
        })
        .to_string();

        // Act
        let response = send(use_case, 0, "5", &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn put_permission_invalid_permission_returns_unprocessable() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockReplacePermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ReplacePermissionError::InvalidPermission) }));
        let body = json!({ "permissions": [] }).to_string();

        // Act
        let response = send(use_case, 0, "5", &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn put_permission_non_root_returns_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockReplacePermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ReplacePermissionError::Forbidden) }));
        let body = json!({ "permissions": [] }).to_string();

        // Act
        let response = send(use_case, 1, "5", &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn put_permission_unknown_user_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockReplacePermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ReplacePermissionError::NoSuchUser) }));
        let body = json!({ "permissions": [] }).to_string();

        // Act
        let response = send(use_case, 0, "5", &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn put_permission_deleted_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockReplacePermissionUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ReplacePermissionError::NoSuchCaller) }));
        let body = json!({ "permissions": [] }).to_string();

        // Act
        let response = send(use_case, 0, "5", &body).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
