use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to reorder the tracks of a playlist.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct ReorderPlaylistTracksRequest {
    /// Complete permutation of the playlist's current zero-based track indices.
    pub order: Vec<u32>,
}

/// Reorder the tracks of a music playlist.
///
/// Any caller who can read the playlist may reorder it. The body must be a
/// complete permutation of the current track indices; the tracks are reindexed
/// to `0..n-1` in the requested order.
#[utoipa::path(
    put,
    operation_id = "put_playlist_track",
    path = "/library/music/playlist/{playlist_id}/track",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    request_body = ReorderPlaylistTracksRequest,
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist reordered"
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
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible playlist"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The order is not a complete permutation of the current indices"
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
    summary = "Reorder the tracks of a music playlist"
)]
pub async fn put_playlist_track(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<ReorderPlaylistTracksRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .reorder_playlist_tracks()
        .execute(ReorderPlaylistTracksCommand::new(
            caller.user_id(),
            playlist_id,
            request.order,
        ))
        .await?;
    Ok(hal_json(PlaylistResponse::new(response.playlist())))
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
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::put;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::put_playlist_track;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::reorder_playlist_tracks::MockReorderPlaylistTracksUseCase;
    use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksError;
    use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksResponse;
    use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::into_parts;

    fn ts() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 6)
            .unwrap_or_default()
            .and_hms_opt(12, 0, 0)
            .unwrap_or_default()
    }

    fn release_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 20).unwrap_or_default()
    }

    fn reordered() -> Result<ReorderPlaylistTracksResponse, Box<dyn Error>> {
        Ok(ReorderPlaylistTracksResponse::new(MusicPlaylist::new(
            3,
            "Mix".to_owned(),
            ts(),
            ts(),
            4,
            false,
            None,
            vec![MusicRecording::try_new(
                31,
                Some("Second".to_owned()),
                release_date(),
                None,
                31,
                202,
                None,
                TrackIndex::PlaylistIndex(0),
            )?],
        )))
    }

    async fn send(
        use_case: MockReorderPlaylistTracksUseCase,
        caller_id: NumericID,
        playlist_id: &str,
        body: Body,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_reorder_playlist_tracks()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ReorderPlaylistTracksUseCase>);

        let mut request = Request::builder()
            .method("PUT")
            .uri(format!(
                "/api/v1/library/music/playlist/{playlist_id}/track"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/playlist/{playlist_id}/track",
                put(put_playlist_track),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn put_playlist_track_forwards_the_order() -> Result<(), Box<dyn Error>> {
        // Arrange
        let reordered = reordered()?;
        let mut use_case = MockReorderPlaylistTracksUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 4
                    && command.playlist_id() == 3
                    && command.order() == [1u32, 0u32]
            })
            .return_once(move |_| Box::pin(async move { Ok(reordered) }));

        // Act
        let (status, content_type, payload) = into_parts(
            send(
                use_case,
                4,
                "3",
                Body::from(json!({ "order": [1u32, 0u32] }).to_string()),
            )
            .await?,
        )
        .await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload
                .get("tracks")
                .and_then(|tracks| tracks.get(0))
                .and_then(|track| track.get("id")),
            Some(&json!(0u32))
        );
        Ok(())
    }

    #[rstest]
    #[case::invalid(
        ReorderPlaylistTracksError::InvalidOrder,
        StatusCode::UNPROCESSABLE_ENTITY
    )]
    #[case::invisible(ReorderPlaylistTracksError::NoSuchPlaylist, StatusCode::NOT_FOUND)]
    #[case::no_caller(ReorderPlaylistTracksError::NoSuchCaller, StatusCode::UNAUTHORIZED)]
    #[tokio::test]
    async fn put_playlist_track_error_maps_to_status(
        #[case] error: ReorderPlaylistTracksError,
        #[case] status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockReorderPlaylistTracksUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(
            use_case,
            4,
            "3",
            Body::from(json!({ "order": [1u32, 0u32] }).to_string()),
        )
        .await?;

        // Assert
        assert_eq!(response.status(), status);
        Ok(())
    }
}
