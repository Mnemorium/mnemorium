use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::patch_album_track::PatchAlbumTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    TrackResponse, deserialize_field_update, parse_album_id, parse_write_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to edit the shared recording behind a track address.
///
/// An absent field keeps its value; a `null` clears an optional value.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchAlbumTrackRequest {
    /// ISRC code of the recording; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub isrc_code: FieldUpdate<String>,
    /// Name of the recording; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub name: FieldUpdate<String>,
}

/// Edit the shared recording behind a track address.
///
/// Only a caller whose `library.manage` scope covers the album may write it.
/// The recording is shared, so the change is visible in every playlist that
/// references it.
#[utoipa::path(
    patch,
    operation_id = "patch_album_track",
    path = "/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium"),
        ("track_index" = u8, Path, description = "Index of the track within the medium"),
    ),
    request_body = PatchAlbumTrackRequest,
    responses(
        (
            status = OK,
            body = TrackResponse,
            content_type = "application/hal+json",
            description = "Recording updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or track address"
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
            description = "Unknown or invisible album, medium or track"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The ISRC code is used by another recording"
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
            description = "The name is empty, the ISRC code is not twelve characters, an index is out of range, or the album is the default one"
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
    summary = "Edit the shared recording behind a track"
)]
pub(crate) async fn patch_album_track(
    Path((album_id_value, medium_index_value, track_index_value)): Path<(String, String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchAlbumTrackRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_write_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let track_index = parse_write_index(
        &track_index_value,
        "track_index",
        "invalid music track index",
    )?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .patch_album_track()
        .execute(PatchAlbumTrackCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            track_index,
            request.name,
            request.isrc_code,
        ))
        .await?;
    let body = TrackResponse::from_album(
        response.album(),
        response.medium_index(),
        response.track_index(),
    )
    .ok_or(ApiError::InternalServerError)?;
    Ok(hal_json(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::patch_album_track::{
        MockPatchAlbumTrackUseCase, PatchAlbumTrackError, PatchAlbumTrackResponse,
        PatchAlbumTrackUseCase,
    };
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::{MediumIndex, MusicMedium};
    use crate::domain::model::music_recording::{MusicRecording, TrackIndex};
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::test_helpers::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::{StatusCode, header};
    use axum::routing::patch;
    use rstest::rstest;
    use serde_json::json;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;

    /// Send a PATCH to the track route on behalf of `caller_id`.
    async fn send(
        use_case: MockPatchAlbumTrackUseCase,
        caller_id: NumericID,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_patch_album_track()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn PatchAlbumTrackUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
            .uri("/api/v1/library/music/album/7/medium/1/track/1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
                patch(patch_album_track),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The album holding one `CD` medium with the recording at index 1.
    fn album() -> Result<MusicAlbum, Box<dyn Error>> {
        let recording = MusicRecording::try_new(
            5,
            Some("So What".to_owned()),
            None,
            1,
            2,
            None,
            TrackIndex::MediumIndex(1),
        )?;
        sample_album(
            7,
            vec![MusicMedium::new(MediumIndex::Cd(1), vec![recording])],
        )
    }

    #[tokio::test]
    async fn patch_album_track_renames_the_recording() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = album()?;
        let mut use_case = MockPatchAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.name() == &FieldUpdate::Set("So What (Remastered)".to_owned()))
            .return_once(move |_| {
                let response = PatchAlbumTrackResponse::new(stored.clone(), 1, 1);
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, r#"{"name":"So What (Remastered)"}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("recording_id"), Some(&json!(5i64)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_track_null_clears_the_isrc() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = album()?;
        let mut use_case = MockPatchAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.isrc_code() == &FieldUpdate::Clear)
            .return_once(move |_| {
                let response = PatchAlbumTrackResponse::new(stored.clone(), 1, 1);
                Box::pin(async move { Ok(response) })
            });

        // Act
        let response = send(use_case, 1, r#"{"isrc_code":null}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::duplicate_isrc(
        PatchAlbumTrackError::IsrcAlreadyExists,
        StatusCode::CONFLICT,
        "a music recording with this ISRC code already exists"
    )]
    #[case::invalid_isrc(
        PatchAlbumTrackError::InvalidIsrc,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the ISRC code must be exactly twelve characters long"
    )]
    #[case::unknown_track(
        PatchAlbumTrackError::NoSuchTrack,
        StatusCode::NOT_FOUND,
        "a music track with this index does not exist"
    )]
    #[tokio::test]
    async fn patch_album_track_error_maps_to_status(
        #[case] error: PatchAlbumTrackError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPatchAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, r#"{"name":"Renamed"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_track_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PATCH")
            .uri("/music/album/7/medium/1/track/1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"name":"Renamed"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
