use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::list_album_tracks::ListAlbumTracksCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::TrackListResponse;
use crate::infrastructure::inbound::rest::handler::library::TrackResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_album_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// List the tracks of a music album.
///
/// Any authenticated caller may read an album's tracks; the album is not owned
/// and carries no visibility flag. The collection is returned in play order,
/// ordered by ascending medium index then track index.
#[utoipa::path(
    get,
    operation_id = "get_album_track_list",
    path = "/library/music/album/{album_id}/track",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album whose tracks are listed"),
    ),
    responses(
        (
            status = OK,
            body = TrackListResponse,
            content_type = "application/hal+json",
            description = "Tracks of the album"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music album identifier"
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
            description = "Unknown music album"
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
    summary = "List the tracks of a music album"
)]
pub async fn get_album_track_list(
    Path(album_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let response = state
        .library_use_case_factory()
        .list_album_tracks()
        .execute(ListAlbumTracksCommand::new(caller.user_id(), album_id))
        .await?;
    let body = TrackListResponse {
        items: response.tracks().iter().map(TrackResponse::from).collect(),
        links: SelfLinks::new(&format!("/api/v1/library/music/album/{album_id}/track")),
    };
    Ok(hal_json(body))
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
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_album_track_list;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::list_album_tracks::ListAlbumTracksError;
    use crate::application::port::list_album_tracks::ListAlbumTracksItem;
    use crate::application::port::list_album_tracks::ListAlbumTracksResponse;
    use crate::application::port::list_album_tracks::ListAlbumTracksUseCase;
    use crate::application::port::list_album_tracks::MockListAlbumTracksUseCase;
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

    /// One track of album `7`, without an ISRC code.
    #[expect(
        clippy::single_call_fn,
        reason = "the track fixture keeps the use-case stub and the assertion in step"
    )]
    fn track_item() -> ListAlbumTracksItem {
        ListAlbumTracksItem::new(
            101,
            7,
            1,
            1,
            201,
            Some("So What".to_owned()),
            None,
            released(),
        )
    }

    /// Send a GET to `/api/v1/library/music/album/{id}/track` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth
    /// middleware does.
    async fn send(
        use_case: MockListAlbumTracksUseCase,
        caller_id: NumericID,
        id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_album_tracks()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListAlbumTracksUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/album/{id}/track"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/track",
                get(get_album_track_list),
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
    async fn get_album_track_list_returns_the_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumTracksUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.album_id() == 7)
            .return_once(|_| {
                let item = track_item();
                Box::pin(async move { Ok(ListAlbumTracksResponse::new(vec![item])) })
            });

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": { "self": { "href": "/api/v1/library/music/album/7/track" } },
                "items": [{
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
                    "isrc_code": null,
                    "first_release_date": "1959-08-17"
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_list_malformed_identifier_returns_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListAlbumTracksUseCase::new();

        // Act
        let response = send(use_case, 1, "abc").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("invalid music album identifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_list_unknown_album_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumTracksUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(ListAlbumTracksError::NoSuchAlbum) }));

        // Act
        let response = send(use_case, 1, "999").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("a music album with this identifier does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_track_list_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/album/7/track")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
