use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::update_music_group::UpdateMusicGroupCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MusicGroupResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_group_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a music group.
///
/// Every field is optional: an omitted field keeps its stored value.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PatchMusicGroupRequest {
    /// Replacement identifier of the image holding the group's artwork.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// New stage name of the group.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
}

/// Update a music group.
///
/// Only a caller whose `library.manage` covers the group may rename it or
/// replace its artwork. The name is trimmed and must be unique; the cover must
/// be a `LIBRARY` image. The seeded default group is protected and refuses
/// every update with `409 Conflict`.
#[utoipa::path(
    patch,
    operation_id = "patch_music_group",
    path = "/music/group/{group_id}",
    tag = "library",
    params(
        ("group_id" = NumericID, Path, description = "Identifier of the group to update"),
    ),
    request_body = PatchMusicGroupRequest,
    responses(
        (
            status = OK,
            body = MusicGroupResponse,
            content_type = "application/hal+json",
            description = "Group updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music group identifier or malformed request body"
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
            description = "The caller may not manage the library"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or inaccessible music group"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The name is taken, the cover is not a library image, or the group is protected"
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
            description = "The group name is empty or too long"
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
    summary = "Update a music group"
)]
pub async fn patch_music_group(
    Path(group_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchMusicGroupRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let group_id = parse_group_id(&group_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .update_music_group()
        .execute(UpdateMusicGroupCommand::new(
            caller.user_id(),
            group_id,
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let group = response.group();
    Ok(hal_json(MusicGroupResponse::new(
        group.id(),
        group.stage_name().to_owned(),
        group.cover_image_id(),
    )))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::patch;
    use mockall::predicate::eq;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::patch_music_group;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::update_music_group::MockUpdateMusicGroupUseCase;
    use crate::application::port::update_music_group::UpdateMusicGroupCommand;
    use crate::application::port::update_music_group::UpdateMusicGroupError;
    use crate::application::port::update_music_group::UpdateMusicGroupResponse;
    use crate::application::port::update_music_group::UpdateMusicGroupUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_group::MusicGroup;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a PATCH to `/api/v1/library/music/group/{group}` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth middleware
    /// does.
    async fn send(
        use_case: MockUpdateMusicGroupUseCase,
        caller_id: NumericID,
        group_id: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_update_music_group()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn UpdateMusicGroupUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/v1/library/music/group/{group_id}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/group/{group_id}",
                patch(patch_music_group),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn patch_music_group_renames_the_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let renamed = MusicGroup::try_new(7, "The Renamed".to_owned(), Some(19))?;
        let mut use_case = MockUpdateMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(UpdateMusicGroupCommand::new(
                1,
                7,
                Some("The Renamed".to_owned()),
                None,
            )))
            .return_once(move |_| {
                let stored = UpdateMusicGroupResponse::new(renamed.clone());
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", r#"{"name":"The Renamed"}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "cover": { "href": "/api/v1/asset/image/19" },
                    "self": { "href": "/api/v1/library/music/group/7" }
                },
                "cover_image_id": 19i64,
                "id": 7i64,
                "name": "The Renamed"
            })
        );
        Ok(())
    }

    #[rstest]
    #[case::forbidden(
        UpdateMusicGroupError::Forbidden,
        StatusCode::FORBIDDEN,
        "the caller may not manage the library"
    )]
    #[case::duplicate(
        UpdateMusicGroupError::NameAlreadyExists,
        StatusCode::CONFLICT,
        "a music group with this name already exists"
    )]
    #[case::protected(
        UpdateMusicGroupError::Conflict,
        StatusCode::CONFLICT,
        "the music group cannot be updated"
    )]
    #[case::unknown_group(
        UpdateMusicGroupError::NoSuchGroup,
        StatusCode::NOT_FOUND,
        "a music group with this identifier does not exist"
    )]
    #[case::invalid_name(
        UpdateMusicGroupError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[tokio::test]
    async fn patch_music_group_error_maps_to_status(
        #[case] error: UpdateMusicGroupError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", r#"{"name":"The Renamed"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn patch_music_group_malformed_identifier_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdateMusicGroupUseCase::new();

        // Act
        let response = send(use_case, 1, "abc", r#"{"name":"The Renamed"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid music group identifier")
        );
        Ok(())
    }
}
