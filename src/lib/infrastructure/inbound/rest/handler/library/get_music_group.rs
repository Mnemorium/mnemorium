use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_music_group::GetMusicGroupCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{MusicGroupResponse, parse_group_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one music group by its identifier.
///
/// Any authenticated caller with library access may read a group; the group is
/// not owned and carries no visibility flag. A caller whose library access is
/// `NONE` sees the same `404` as an unknown identifier.
#[utoipa::path(
    get,
    operation_id = "get_music_group",
    path = "/music/group/{group_id}",
    tag = "library",
    params(
        ("group_id" = NumericID, Path, description = "Identifier of the group to fetch"),
    ),
    responses(
        (
            status = OK,
            body = MusicGroupResponse,
            content_type = "application/hal+json",
            description = "Group found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music group identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or inaccessible music group"
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
    summary = "Fetch a music group"
)]
pub(crate) async fn get_music_group(
    Path(group_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let group_id = parse_group_id(&group_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_music_group()
        .execute(GetMusicGroupCommand::new(caller.user_id(), group_id))
        .await?;
    Ok(hal_json(MusicGroupResponse::from(&response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::get_music_group::{
        GetMusicGroupError, GetMusicGroupResponse, GetMusicGroupUseCase, MockGetMusicGroupUseCase,
    };
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::routing::get;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// Send a GET to `/api/v1/library/music/group/{group}` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth middleware
    /// does.
    async fn send(
        use_case: MockGetMusicGroupUseCase,
        caller_id: NumericID,
        group_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_music_group()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetMusicGroupUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/group/{group_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/group/{group_id}",
                get(get_music_group),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_music_group_returns_the_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.group_id() == 7)
            .return_once(|_| {
                Box::pin(async { Ok(GetMusicGroupResponse::new("The Group".to_owned(), 7, None)) })
            });

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/music/group/7" }
                },
                "cover_image_id": null,
                "id": 7i64,
                "name": "The Group"
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_with_artwork_exposes_the_cover_link() -> Result<(), Box<dyn Error>> {
        // Arrange: the group's cover is image 19.
        let mut use_case = MockGetMusicGroupUseCase::new();
        use_case.expect_execute().times(1).return_once(|_| {
            Box::pin(async {
                Ok(GetMusicGroupResponse::new(
                    "The Group".to_owned(),
                    7,
                    Some(19),
                ))
            })
        });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("cover_image_id"), Some(&json!(19i64)));
        assert_eq!(
            payload.pointer("/_links/cover/href"),
            Some(&json!("/api/v1/asset/image/19"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockGetMusicGroupUseCase::new();

        // Act
        let response = send(use_case, 1, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid music group identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_unknown_group_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetMusicGroupError::NoSuchGroup) }));

        // Act
        let response = send(use_case, 1, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetMusicGroupError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_music_group_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/group/7")
            .body(Body::empty())?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
