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
use crate::infrastructure::inbound::rest::handler::library::parse_index;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// List the tracks of one medium of a music album.
///
/// Any authenticated caller may read a medium's tracks; the album is not owned
/// and carries no visibility flag. The collection is returned in play order,
/// ordered by ascending track index.
#[utoipa::path(
    get,
    operation_id = "get_album_track_list",
    path = "/library/music/album/{album_id}/medium/{medium_index}/track",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album whose medium's tracks are listed"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium whose tracks are listed"),
    ),
    responses(
        (
            status = OK,
            body = TrackListResponse,
            content_type = "application/hal+json",
            description = "Tracks of the album medium"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music album identifier or medium index"
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
            description = "Unknown music album, or medium of the album"
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
    summary = "List the tracks of a music album medium"
)]
pub async fn get_album_track_list(
    Path((album_id_value, medium_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let response = state
        .library_use_case_factory()
        .list_album_tracks()
        .execute(ListAlbumTracksCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
        ))
        .await?;
    let body = TrackListResponse {
        items: response.tracks().iter().map(TrackResponse::from).collect(),
        links: SelfLinks::new(&format!(
            "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track"
        )),
    };
    Ok(hal_json(body))
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
    use crate::test_helpers::album_release_date;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a GET to
    /// `/api/v1/library/music/album/{album}/medium/{medium}/track` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth middleware
    /// does.
    async fn send(
        use_case: MockListAlbumTracksUseCase,
        caller_id: NumericID,
        album_id: &str,
        medium_index: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_list_album_tracks()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn ListAlbumTracksUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track"
            ))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track",
                get(get_album_track_list),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn get_album_track_list_returns_the_collection() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumTracksUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1 && command.album_id() == 7 && command.medium_index() == 2
            })
            .return_once(|_| {
                let item = ListAlbumTracksItem::new(
                    7,
                    2,
                    1,
                    30,
                    201,
                    Some("So What".to_owned()),
                    None,
                    album_release_date(),
                );
                Box::pin(async move { Ok(ListAlbumTracksResponse::new(vec![item])) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, "7", "2").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": {
                        "href": "/api/v1/library/music/album/7/medium/2/track"
                    }
                },
                "items": [{
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
                    "isrc_code": null,
                    "first_release_date": "1959-08-17"
                }]
            })
        );
        Ok(())
    }

    #[rstest]
    #[case::album("abc", "2", "invalid music album identifier")]
    #[case::medium("7", "abc", "invalid music medium index")]
    #[tokio::test]
    async fn get_album_track_list_malformed_identifier_returns_bad_request(
        #[case] album_id: &str,
        #[case] medium_index: &str,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockListAlbumTracksUseCase::new();

        // Act
        let response = send(use_case, 1, album_id, medium_index).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[rstest]
    #[case::unknown_album(ListAlbumTracksError::NoSuchAlbum)]
    #[case::unknown_medium(ListAlbumTracksError::NoSuchMedium)]
    #[tokio::test]
    async fn get_album_track_list_not_found_returns_not_found(
        #[case] error: ListAlbumTracksError,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockListAlbumTracksUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async move { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", "2").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
            .uri("/music/album/7/medium/2/track")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
