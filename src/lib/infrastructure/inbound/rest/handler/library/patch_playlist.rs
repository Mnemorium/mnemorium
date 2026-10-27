use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::update_playlist::UpdatePlaylistCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// A recording to append to a playlist.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct AddPlaylistTrackRequest {
    /// Identifier of the shared recording to append.
    pub recording_id: NumericID,
}

/// Payload to update a playlist's metadata and/or append a recording.
///
/// Every field is optional. `name` and `is_public` are owner-only; `add_track`
/// is allowed to any caller who can read the playlist. A request mixing an
/// owner-only field with `add_track` from a non-owner is rejected as a whole.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PatchPlaylistRequest {
    /// Recording to append to the playlist, when requested.
    pub add_track: Option<AddPlaylistTrackRequest>,
    /// New visibility, when requested.
    pub is_public: Option<bool>,
    /// New name, when requested.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
}

/// Update a playlist's metadata and/or append a recording.
///
/// Omitted fields stay unchanged. `name` and `is_public` require ownership;
/// adding a track requires read access to the playlist. The request is atomic:
/// a rejected field applies nothing.
#[utoipa::path(
    patch,
    operation_id = "patch_playlist",
    path = "/library/music/playlist/{playlist_id}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    request_body = PatchPlaylistRequest,
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier or malformed body"
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller does not own the playlist"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible playlist"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Duplicate playlist name or duplicate recording"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid name or unknown recording"
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
    summary = "Update a music playlist"
)]
pub async fn patch_playlist(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchPlaylistRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .update_playlist()
        .execute(UpdatePlaylistCommand::new(
            caller.user_id(),
            playlist_id,
            request.name,
            request.is_public,
            request.add_track.map(|add_track| add_track.recording_id),
        ))
        .await?;
    Ok(hal_json(PlaylistResponse::new(response.playlist())))
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
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::patch_playlist;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::update_playlist::MockUpdatePlaylistUseCase;
    use crate::application::port::update_playlist::UpdatePlaylistError;
    use crate::application::port::update_playlist::UpdatePlaylistResponse;
    use crate::application::port::update_playlist::UpdatePlaylistUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    fn updated() -> UpdatePlaylistResponse {
        UpdatePlaylistResponse::new(MusicPlaylist::new(
            3,
            "New".to_owned(),
            ts(),
            ts(),
            4,
            true,
            None,
            Vec::new(),
        ))
    }

    async fn send(
        use_case: MockUpdatePlaylistUseCase,
        caller_id: NumericID,
        playlist_id: &str,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_update_playlist()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn UpdatePlaylistUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/v1/library/music/playlist/{playlist_id}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/playlist/{playlist_id}",
                patch(patch_playlist),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn patch_playlist_forwards_every_field() -> Result<(), Box<dyn Error>> {
        // Arrange
        let updated = updated();
        let mut use_case = MockUpdatePlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 4
                    && command.playlist_id() == 3
                    && command.name() == Some("New")
                    && command.is_public() == Some(true)
                    && command.add_track() == Some(30)
            })
            .return_once(move |_| Box::pin(async move { Ok(updated) }));

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                4,
                "3",
                Body::from(
                    json!({ "name": "New", "is_public": true, "add_track": { "recording_id": 30i64 } })
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
            payload
                .get("_links")
                .and_then(|links| links.get("self"))
                .and_then(|link| link.get("href")),
            Some(&json!("/api/v1/library/music/playlist/3"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_playlist_empty_body_forwards_no_field() -> Result<(), Box<dyn Error>> {
        // Arrange
        let updated = updated();
        let mut use_case = MockUpdatePlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.name().is_none()
                    && command.is_public().is_none()
                    && command.add_track().is_none()
            })
            .return_once(move |_| Box::pin(async move { Ok(updated) }));

        // Act
        let response = send(use_case, 4, "3", Body::from("{}")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::invalid_name(UpdatePlaylistError::InvalidName, StatusCode::UNPROCESSABLE_ENTITY)]
    #[case::too_long(UpdatePlaylistError::NameTooLong, StatusCode::UNPROCESSABLE_ENTITY)]
    #[case::name_taken(UpdatePlaylistError::NameAlreadyExists, StatusCode::CONFLICT)]
    #[case::duplicate_recording(UpdatePlaylistError::RecordingAlreadyPresent, StatusCode::CONFLICT)]
    #[case::conflict(UpdatePlaylistError::Conflict, StatusCode::CONFLICT)]
    #[case::forbidden(UpdatePlaylistError::Forbidden, StatusCode::FORBIDDEN)]
    #[case::no_playlist(UpdatePlaylistError::NoSuchPlaylist, StatusCode::NOT_FOUND)]
    #[case::no_recording(UpdatePlaylistError::NoSuchRecording, StatusCode::UNPROCESSABLE_ENTITY)]
    #[case::no_caller(UpdatePlaylistError::NoSuchCaller, StatusCode::UNAUTHORIZED)]
    #[tokio::test]
    async fn patch_playlist_error_maps_to_status(
        #[case] error: UpdatePlaylistError,
        #[case] status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdatePlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 4, "3", Body::from("{}")).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert!(error_message_of(&response).is_some());
        Ok(())
    }

    #[tokio::test]
    async fn patch_playlist_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let use_case = MockUpdatePlaylistUseCase::new();

        // Act
        let response = send(use_case, 4, "abc", Body::from("{}")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid music playlist identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn patch_playlist_malformed_body_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockUpdatePlaylistUseCase::new();

        // Act
        let response = send(use_case, 4, "3", Body::from("not json")).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }
}
