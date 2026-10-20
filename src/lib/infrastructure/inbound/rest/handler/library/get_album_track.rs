use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_album_track::GetAlbumTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::TrackResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_album_id;
use crate::infrastructure::inbound::rest::handler::library::parse_index;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one track of one medium of a music album by its indexes.
///
/// Any authenticated caller may read a track; the album is not owned and carries
/// no visibility flag. The track is scoped to the album and medium named in the
/// path, so a track of another album or medium is reported as missing.
#[utoipa::path(
    get,
    operation_id = "get_album_track",
    path = "/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album the track belongs to"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium the track is placed in"),
        ("track_index" = u8, Path, description = "Position of the track within its medium"),
    ),
    responses(
        (
            status = OK,
            body = TrackResponse,
            content_type = "application/hal+json",
            description = "Track found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music album, medium or track index"
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
            description = "Unknown music album, medium, or track of the medium"
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
    summary = "Fetch a music track"
)]
pub async fn get_album_track(
    Path((album_id_value, medium_index_value, track_index_value)): Path<(String, String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let track_index = parse_index(
        &track_index_value,
        "track_index",
        "invalid music track index",
    )?;
    let response = state
        .library_use_case_factory()
        .get_album_track()
        .execute(GetAlbumTrackCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            track_index,
        ))
        .await?;
    Ok(hal_json(TrackResponse::from(response)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::Response;
    use axum::routing::get;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_album_track;
    use crate::application::port::get_album_track::GetAlbumTrackError;
    use crate::application::port::get_album_track::GetAlbumTrackResponse;
    use crate::application::port::get_album_track::GetAlbumTrackUseCase;
    use crate::application::port::get_album_track::MockGetAlbumTrackUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::album_release_date;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a GET to
    /// `/api/v1/library/music/album/{album}/medium/{medium}/track/{track}` on
    /// behalf of `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockGetAlbumTrackUseCase,
        caller_id: NumericID,
        album_id: &str,
        medium_index: &str,
        track_index: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_album_track()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetAlbumTrackUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
                get(get_album_track),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_album_track_returns_the_track() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.album_id() == 7
                    && command.medium_index() == 2
                    && command.track_index() == 1
            })
            .return_once(|_| {
                Box::pin(async {
                    Ok(GetAlbumTrackResponse::new(
                        7,
                        2,
                        1,
                        30,
                        201,
                        Some("So What".to_owned()),
                        Some("USRC17607839".to_owned()),
                        album_release_date(),
                    ))
                })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", "2", "1").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": {
                        "href": "/api/v1/library/music/album/7/medium/2/track/1"
                    },
                    "album": { "href": "/api/v1/library/music/album/7" },
                    "medium": { "href": "/api/v1/library/music/album/7/medium/2" }
                },
                "id": 1u8,
                "album_id": 7i64,
                "medium_index": 2u8,
                "recording_id": 30i64,
                "file_id": 201i64,
                "name": "So What",
                "isrc_code": "USRC17607839",
                "first_release_date": "1959-08-17"
            })
        );
        Ok(())
    }

    #[rstest]
    #[case::album("abc", "2", "1", "invalid music album identifier")]
    #[case::medium("7", "abc", "1", "invalid music medium index")]
    #[case::track("7", "2", "abc", "invalid music track index")]
    #[case::track_out_of_range("7", "2", "256", "invalid music track index")]
    #[tokio::test]
    async fn get_album_track_malformed_identifier_returns_bad_request(
        #[case] album_id: &str,
        #[case] medium_index: &str,
        #[case] track_index: &str,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetAlbumTrackUseCase::new();

        // Act
        let response = send(use_case, 1, album_id, medium_index, track_index).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[rstest]
    #[case::unknown_album(GetAlbumTrackError::NoSuchAlbum)]
    #[case::unknown_medium(GetAlbumTrackError::NoSuchMedium)]
    #[case::unknown_track(GetAlbumTrackError::NoSuchTrack)]
    #[tokio::test]
    async fn get_album_track_not_found_returns_not_found(
        #[case] error: GetAlbumTrackError,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async move { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", "2", "1").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetAlbumTrackError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "7", "2", "1").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/album/7/medium/2/track/1")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
