use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::create_music_group::CreateMusicGroupCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MusicGroupResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a music group.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostMusicGroupRequest {
    /// Identifier of the image holding the group's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Stage name of the group.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
}

/// Create a music group.
///
/// Only a caller whose `library.manage` covers the catalogue may create a
/// group. The name is trimmed and must be unique; the optional cover must be a
/// `LIBRARY` image.
#[utoipa::path(
    post,
    operation_id = "post_music_group",
    path = "/music/group",
    tag = "library",
    request_body = PostMusicGroupRequest,
    responses(
        (
            status = CREATED,
            body = MusicGroupResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created group"),
            ),
            description = "Group created",
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller may not manage the library"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The name is taken, or the cover is not a library image"
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
    summary = "Create a music group"
)]
pub async fn post_music_group(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostMusicGroupRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .create_music_group()
        .execute(CreateMusicGroupCommand::new(
            caller.user_id(),
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let group = response.group();
    let body = MusicGroupResponse::new(
        group.id(),
        group.stage_name().to_owned(),
        group.cover_image_id(),
    );
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
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use mockall::predicate::eq;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_music_group;
    use crate::application::port::create_music_group::CreateMusicGroupCommand;
    use crate::application::port::create_music_group::CreateMusicGroupError;
    use crate::application::port::create_music_group::CreateMusicGroupResponse;
    use crate::application::port::create_music_group::CreateMusicGroupUseCase;
    use crate::application::port::create_music_group::MockCreateMusicGroupUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_group::MusicGroup;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a POST to `/api/v1/library/music/group` on behalf of `caller_id`,
    /// injecting the caller identifier the way the auth middleware does.
    async fn send(
        use_case: MockCreateMusicGroupUseCase,
        caller_id: NumericID,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_music_group()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CreateMusicGroupUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/library/music/group")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/group", post(post_music_group))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn post_music_group_creates_the_group() -> Result<(), Box<dyn Error>> {
        // Arrange
        let created = MusicGroup::try_new(7, "The Group".to_owned(), Some(19))?;
        let mut use_case = MockCreateMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(CreateMusicGroupCommand::new(
                1,
                "The Group".to_owned(),
                Some(19),
            )))
            .return_once(move |_| {
                let stored = CreateMusicGroupResponse::new(created.clone());
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, r#"{"name":"The Group","cover_image_id":19}"#).await?)
                .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
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
                "name": "The Group"
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_music_group_returns_the_location_of_the_new_group() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let created = MusicGroup::try_new(7, "The Group".to_owned(), None)?;
        let mut use_case = MockCreateMusicGroupUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let stored = CreateMusicGroupResponse::new(created.clone());
            Box::pin(async move { Ok(stored) })
        });

        // Act
        let response = send(use_case, 1, r#"{"name":"The Group"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/library/music/group/7")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_music_group_standard_user_is_forbidden() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(CreateMusicGroupError::Forbidden) }));

        // Act
        let response = send(use_case, 1, r#"{"name":"The Group"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the caller may not manage the library")
        );
        Ok(())
    }

    #[rstest]
    #[case::duplicate(
        CreateMusicGroupError::NameAlreadyExists,
        StatusCode::CONFLICT,
        "a music group with this name already exists"
    )]
    #[case::user_cover(
        CreateMusicGroupError::InvalidCover,
        StatusCode::CONFLICT,
        "the cover image is not a library image"
    )]
    #[case::invalid_name(
        CreateMusicGroupError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[tokio::test]
    async fn post_music_group_error_maps_to_status(
        #[case] error: CreateMusicGroupError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateMusicGroupUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, r#"{"name":"The Group"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn post_music_group_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreateMusicGroupUseCase::new();

        // Act
        let response = send(use_case, 1, "not json").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn post_music_group_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/music/group")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"name":"The Group"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
