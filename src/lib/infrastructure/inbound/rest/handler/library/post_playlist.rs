use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::create_playlist::CreatePlaylistCommand;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a playlist owned by the caller.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostPlaylistRequest {
    /// Whether any authenticated user may read and moderate the playlist.
    #[serde(default)]
    pub is_public: bool,
    /// Name of the playlist.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
}

/// Create a playlist owned by the caller.
///
/// Any authenticated caller may create a playlist. The name is required,
/// trimmed and at most 100 characters long; `is_public` defaults to `false`.
#[utoipa::path(
    post,
    operation_id = "post_playlist",
    path = "/library/music/playlist",
    tag = "library",
    request_body = PostPlaylistRequest,
    responses(
        (
            status = CREATED,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created playlist"),
            ),
            description = "Playlist created",
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
            description = "The playlist name is empty or too long"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller already owns a playlist with this name"
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
    summary = "Create a music playlist"
)]
pub async fn post_playlist(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostPlaylistRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .create_playlist()
        .execute(CreatePlaylistCommand::new(
            caller.user_id(),
            request.name,
            request.is_public,
        ))
        .await?;
    let body = PlaylistResponse::new(response.playlist());
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
    #![expect(
        clippy::single_call_fn,
        reason = "named fixtures keep the tests readable"
    )]

    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use mockall::predicate::eq;
    use rstest::rstest;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_playlist;
    use crate::application::port::create_playlist::CreatePlaylistCommand;
    use crate::application::port::create_playlist::CreatePlaylistError;
    use crate::application::port::create_playlist::CreatePlaylistResponse;
    use crate::application::port::create_playlist::CreatePlaylistUseCase;
    use crate::application::port::create_playlist::MockCreatePlaylistUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    fn created() -> Result<CreatePlaylistResponse, Box<dyn Error>> {
        Ok(CreatePlaylistResponse::new(MusicPlaylist::try_new(
            5,
            "Mix".to_owned(),
            ts(),
            ts(),
            1,
            false,
            Vec::new(),
        )?))
    }

    async fn send(
        use_case: MockCreatePlaylistUseCase,
        caller_id: NumericID,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_playlist()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CreatePlaylistUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/library/music/playlist")
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/playlist", post(post_playlist))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, location, content_type, body))
    }

    #[tokio::test]
    async fn post_playlist_defaults_is_public_to_false() -> Result<(), Box<dyn Error>> {
        // Arrange
        let created = created()?;
        let mut use_case = MockCreatePlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .with(eq(CreatePlaylistCommand::new(1, "Mix".to_owned(), false)))
            .return_once(move |_| Box::pin(async move { Ok(created) }));

        // Act
        let (status, location, content_type, payload) = into_parts(
            send(
                use_case,
                1,
                Body::from(json!({ "name": "Mix" }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            location.as_deref(),
            Some("/api/v1/library/music/playlist/5")
        );
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/music/playlist/5" },
                    "tracks": { "href": "/api/v1/library/music/playlist/5/track" }
                },
                "id": 5i64,
                "name": "Mix",
                "owner_id": 1i64,
                "is_public": false,
                "created_at": "2026-10-06T12:00:00",
                "last_modified_at": "2026-10-06T12:00:00",
                "track_count": 0i64,
                "tracks": []
            })
        );
        Ok(())
    }

    #[rstest]
    #[case::empty_name(1, CreatePlaylistError::InvalidName, StatusCode::UNPROCESSABLE_ENTITY)]
    #[case::too_long_name(1, CreatePlaylistError::NameTooLong, StatusCode::UNPROCESSABLE_ENTITY)]
    #[case::existing_name(1, CreatePlaylistError::NameAlreadyExists, StatusCode::CONFLICT)]
    #[case::unknown_caller(999, CreatePlaylistError::NoSuchCaller, StatusCode::UNAUTHORIZED)]
    #[case::dependency_failure(
        1,
        CreatePlaylistError::Unknown(anyhow::anyhow!("boom")),
        StatusCode::INTERNAL_SERVER_ERROR
    )]
    #[tokio::test]
    async fn post_playlist_error_maps_to_status(
        #[case] caller_id: NumericID,
        #[case] error: CreatePlaylistError,
        #[case] status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreatePlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(
            use_case,
            caller_id,
            Body::from(json!({ "name": "Mix" }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), status);
        assert!(error_message_of(&response).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn post_playlist_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreatePlaylistUseCase::new();

        // Act
        let response = send(use_case, 1, Body::from("not json")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn post_playlist_missing_name_returns_unprocessable_entity() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockCreatePlaylistUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            Body::from(json!({ "is_public": true }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }
}
