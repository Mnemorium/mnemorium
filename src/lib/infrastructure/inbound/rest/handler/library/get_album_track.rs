use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;
use tracing::warn;

use crate::application::port::get_album_track::GetAlbumTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::TrackResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_album_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one track of a music album by its identifiers.
///
/// Any authenticated caller may read a track; the album is not owned and carries
/// no visibility flag. The track is scoped to the album named in the path, so a
/// track of another album is reported as missing.
#[utoipa::path(
    get,
    operation_id = "get_album_track",
    path = "/library/music/album/{album_id}/track/{track_id}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album the track belongs to"),
        ("track_id" = NumericID, Path, description = "Identifier of the track to fetch"),
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
            description = "Invalid music album or track identifier"
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
            description = "Unknown music album, or unknown track of the album"
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
    Path((album_id_value, track_id_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let track_id = track_id_value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "track_id",
            reason = "invalid_identifier",
            "rejected a malformed music track identifier"
        );
        ApiError::BadRequest("invalid music track identifier".to_owned())
    })?;
    let response = state
        .library_use_case_factory()
        .get_album_track()
        .execute(GetAlbumTrackCommand::new(
            caller.user_id(),
            album_id,
            track_id,
        ))
        .await?;
    Ok(hal_json(TrackResponse::from(response)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::get;
    use chrono::NaiveDate;
    use rstest::rstest;
    use serde_json::Value;
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
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;

    /// Build the release date the fixtures use.
    #[expect(
        clippy::single_call_fn,
        reason = "the release date keeps the fixture and its assertion in step"
    )]
    fn released() -> NaiveDate {
        NaiveDate::from_ymd_opt(1959, 8, 17).unwrap_or_default()
    }

    /// The response of a successful track lookup.
    #[expect(
        clippy::single_call_fn,
        reason = "the track fixture keeps the use-case stub and the assertion in step"
    )]
    fn track_response() -> GetAlbumTrackResponse {
        GetAlbumTrackResponse::new(
            101,
            7,
            1,
            1,
            201,
            Some("So What".to_owned()),
            Some("USRC17607839".to_owned()),
            released(),
        )
    }

    /// Send a GET to `/api/v1/library/music/album/{album}/track/{track}` on
    /// behalf of `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockGetAlbumTrackUseCase,
        caller_id: NumericID,
        album_id: &str,
        track_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_album_track()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetAlbumTrackUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/music/album/{album_id}/track/{track_id}"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/track/{track_id}",
                get(get_album_track),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status, `Content-Type` header and decoded
    /// JSON body.
    #[expect(
        clippy::single_call_fn,
        reason = "the response splitter keeps the handler test readable"
    )]
    async fn into_parts(
        response: Response,
    ) -> Result<(StatusCode, Option<String>, Value), Box<dyn Error>> {
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body = serde_json::from_slice(&bytes)?;
        Ok((status, content_type, body))
    }

    #[tokio::test]
    async fn get_album_track_returns_the_track() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1 && command.album_id() == 7 && command.track_id() == 101
            })
            .return_once(|_| Box::pin(async { Ok(track_response()) }));

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", "101").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/music/album/7/track/101" },
                    "album": { "href": "/api/v1/library/music/album/7" }
                },
                "id": 101i64,
                "album_id": 7i64,
                "medium_index": 1u32,
                "track_index": 1i64,
                "file_id": 201i64,
                "name": "So What",
                "isrc_code": "USRC17607839",
                "first_release_date": "1959-08-17"
            })
        );
        Ok(())
    }

    #[rstest]
    #[case::album("abc", "101", "invalid music album identifier")]
    #[case::track("7", "abc", "invalid music track identifier")]
    #[tokio::test]
    async fn get_album_track_malformed_identifier_returns_bad_request(
        #[case] album_id: &str,
        #[case] track_id: &str,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetAlbumTrackUseCase::new();

        // Act
        let response = send(use_case, 1, album_id, track_id).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[rstest]
    #[case::unknown_album(GetAlbumTrackError::NoSuchAlbum, "999")]
    #[case::unknown_track(GetAlbumTrackError::NoSuchTrack, "555")]
    #[tokio::test]
    async fn get_album_track_not_found_returns_not_found(
        #[case] error: GetAlbumTrackError,
        #[case] track_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumTrackUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async move { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", track_id).await?;

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
        let response = send(use_case, 999, "7", "101").await?;

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
            .uri("/music/album/7/track/101")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
