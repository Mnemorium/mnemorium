use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_playlist::GetPlaylistCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::PlaylistResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one music playlist with its tracks.
///
/// Any authenticated caller who can read the playlist may fetch it: a public
/// playlist is readable by all, a private one only by its owner and the Root
/// Admin. A playlist the caller cannot read answers `404`, exactly like a
/// missing one.
#[utoipa::path(
    get,
    operation_id = "get_playlist",
    path = "/library/music/playlist/{playlist_id}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier"
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
            description = "Unknown or invisible playlist"
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
    summary = "Fetch a music playlist"
)]
pub async fn get_playlist(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_playlist()
        .execute(GetPlaylistCommand::new(caller.user_id(), playlist_id))
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
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use chrono::NaiveDateTime;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_playlist;
    use crate::application::port::get_playlist::GetPlaylistError;
    use crate::application::port::get_playlist::GetPlaylistResponse;
    use crate::application::port::get_playlist::GetPlaylistUseCase;
    use crate::application::port::get_playlist::MockGetPlaylistUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_playlist::MusicPlaylist;
    use crate::domain::model::music_recording::MusicRecording;
    use crate::domain::model::music_recording::TrackIndex;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
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

    fn release_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 20).unwrap_or_default()
    }

    fn fetched() -> Result<GetPlaylistResponse, Box<dyn Error>> {
        Ok(GetPlaylistResponse::new(MusicPlaylist::new(
            3,
            "Mix".to_owned(),
            ts(),
            ts(),
            4,
            false,
            vec![MusicRecording::try_new(
                30,
                Some("First".to_owned()),
                release_date(),
                None,
                30,
                201,
                None,
                TrackIndex::PlaylistIndex(0),
            )?],
        )))
    }

    async fn send(
        use_case: MockGetPlaylistUseCase,
        caller_id: NumericID,
        playlist_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_playlist()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetPlaylistUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/playlist/{playlist_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/playlist/{playlist_id}",
                get(get_playlist),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_playlist_returns_playlist_with_tracks() -> Result<(), Box<dyn Error>> {
        // Arrange
        let fetched = fetched()?;
        let mut use_case = MockGetPlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 4 && command.playlist_id() == 3)
            .return_once(move |_| Box::pin(async move { Ok(fetched) }));

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 4, "3").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/music/playlist/3" },
                    "tracks": { "href": "/api/v1/library/music/playlist/3/track" }
                },
                "id": 3i64,
                "name": "Mix",
                "owner_id": 4i64,
                "is_public": false,
                "created_at": "2026-10-06T12:00:00",
                "last_modified_at": "2026-10-06T12:00:00",
                "track_count": 1i64,
                "tracks": [{
                    "id": 0u32,
                    "recording_id": 30i64,
                    "file_id": 201i64,
                    "name": "First",
                    "isrc_code": null,
                    "first_release_date": "2026-10-20",
                    "_links": {
                        "self": { "href": "/api/v1/library/music/playlist/3/track/0" },
                        "playlist": { "href": "/api/v1/library/music/playlist/3" }
                    }
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetPlaylistUseCase::new();

        // Act
        let response = send(use_case, 1, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid music playlist identifier")
        );
        Ok(())
    }

    #[rstest]
    #[case::unknown(GetPlaylistError::NoSuchPlaylist, StatusCode::NOT_FOUND)]
    #[case::unknown_caller(GetPlaylistError::NoSuchCaller, StatusCode::UNAUTHORIZED)]
    #[tokio::test]
    async fn get_playlist_error_maps_to_status(
        #[case] error: GetPlaylistError,
        #[case] status: StatusCode,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetPlaylistUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, "3").await?;

        // Assert
        assert_eq!(response.status(), status);
        Ok(())
    }

    #[tokio::test]
    async fn get_playlist_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/playlist/3")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
