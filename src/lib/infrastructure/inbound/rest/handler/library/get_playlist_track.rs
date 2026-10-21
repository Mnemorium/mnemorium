use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_playlist_track::GetPlaylistTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::PlaylistTrackResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_track_index;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one track of one music playlist by its zero-based index.
///
/// Any authenticated caller who can read the playlist may read a track. The
/// track is scoped to the playlist named in the path, so a track of another
/// playlist is reported as missing.
#[utoipa::path(
    get,
    operation_id = "get_playlist_track",
    path = "/library/music/playlist/{playlist_id}/track/{track_index}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist the track belongs to"),
        ("track_index" = u32, Path, description = "Zero-based index of the track within the playlist"),
    ),
    responses(
        (
            status = OK,
            body = PlaylistTrackResponse,
            content_type = "application/hal+json",
            description = "Track found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier or track index"
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
            description = "Unknown or invisible playlist, or unknown track"
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
    summary = "Fetch one track of a music playlist"
)]
pub async fn get_playlist_track(
    Path((playlist_id_value, track_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let track_index = parse_playlist_track_index(&track_index_value)?;
    let response = state
        .library_use_case_factory()
        .get_playlist_track()
        .execute(GetPlaylistTrackCommand::new(
            caller.user_id(),
            playlist_id,
            track_index,
        ))
        .await?;
    Ok(hal_json(PlaylistTrackResponse::new(
        response.playlist_id(),
        response.track(),
    )))
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
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_playlist_track;
    use crate::application::port::get_playlist_track::GetPlaylistTrackError;
    use crate::application::port::get_playlist_track::GetPlaylistTrackResponse;
    use crate::application::port::get_playlist_track::GetPlaylistTrackUseCase;
    use crate::application::port::get_playlist_track::MockGetPlaylistTrackUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    fn release_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 20).unwrap_or_default()
    }

    fn fetched() -> Result<GetPlaylistTrackResponse, Box<dyn Error>> {
        Ok(GetPlaylistTrackResponse::new(
            3,
            MusicRecording::try_new(
                30,
                Some("Third".to_owned()),
                release_date(),
                Some("USRC17607839".to_owned()),
                30,
                201,
                None,
                TrackIndex::PlaylistIndex(2),
            )?,
        ))
    }

    async fn send(
        use_case: MockGetPlaylistTrackUseCase,
        caller_id: NumericID,
        playlist_id: &str,
        track_index: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_playlist_track()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetPlaylistTrackUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/music/playlist/{playlist_id}/track/{track_index}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/playlist/{playlist_id}/track/{track_index}",
                get(get_playlist_track),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_playlist_track_returns_the_track() -> Result<(), Box<dyn Error>> {
        // Arrange
        let fetched = fetched()?;
        let mut use_case = MockGetPlaylistTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 4 && command.playlist_id() == 3 && command.track_index() == 2
            })
            .return_once(move |_| Box::pin(async move { Ok(fetched) }));

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 4, "3", "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/music/playlist/3/track/2" },
                    "playlist": { "href": "/api/v1/library/music/playlist/3" }
                },
                "id": 2u32,
                "recording_id": 30i64,
                "file_id": 201i64,
                "name": "Third",
                "isrc_code": "USRC17607839",
                "first_release_date": "2026-10-20"
            })
        );
        Ok(())
    }

    #[rstest]
    #[case::playlist("abc", "0", "invalid music playlist identifier")]
    #[case::index("3", "abc", "invalid playlist track index")]
    #[tokio::test]
    async fn get_playlist_track_malformed_identifier_returns_bad_request(
        #[case] playlist_id: &str,
        #[case] track_index: &str,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetPlaylistTrackUseCase::new();

        // Act
        let response = send(use_case, 1, playlist_id, track_index).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[rstest]
    #[case::unknown_playlist(GetPlaylistTrackError::NoSuchPlaylist)]
    #[case::unknown_track(GetPlaylistTrackError::NoSuchTrack)]
    #[tokio::test]
    async fn get_playlist_track_not_found_returns_not_found(
        #[case] error: GetPlaylistTrackError,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetPlaylistTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async move { Err(error) }));

        // Act
        let response = send(use_case, 4, "3", "2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_track_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let mut use_case = MockGetPlaylistTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetPlaylistTrackError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "3", "2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
