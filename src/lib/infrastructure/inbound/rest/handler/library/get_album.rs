use axum::extract::Path;
use axum::extract::State;
use axum::response::Response;

use crate::application::port::get_album::GetAlbumCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::AlbumResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_album_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one music album by its identifier.
///
/// Any authenticated caller may read an album; the album is not owned and
/// carries no visibility flag. The representation carries the album's mediums
/// so a client can navigate to each medium and its track collection.
#[utoipa::path(
    get,
    operation_id = "get_album",
    path = "/library/music/album/{album_id}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album to fetch"),
    ),
    responses(
        (
            status = OK,
            body = AlbumResponse,
            content_type = "application/hal+json",
            description = "Album found"
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
            description = "Unknown or inaccessible music album"
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
    summary = "Fetch a music album"
)]
pub async fn get_album(
    Path(album_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_album()
        .execute(GetAlbumCommand::new(caller.user_id(), album_id))
        .await?;
    Ok(hal_json(AlbumResponse::from(&response)))
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
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_album;
    use crate::application::port::get_album::GetAlbumError;
    use crate::application::port::get_album::GetAlbumMediumItem;
    use crate::application::port::get_album::GetAlbumResponse;
    use crate::application::port::get_album::GetAlbumUseCase;
    use crate::application::port::get_album::MockGetAlbumUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_album::ProductionType;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::album_release_date;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;

    /// Send a GET to `/api/v1/library/music/album/{album}` on behalf of
    /// `caller_id`, injecting the caller identifier the way the auth middleware
    /// does.
    async fn send(
        use_case: MockGetAlbumUseCase,
        caller_id: NumericID,
        album_id: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_get_album()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn GetAlbumUseCase>);

        let mut request = Request::builder()
            .method("GET")
            .uri(format!("/api/v1/library/music/album/{album_id}"))
            .body(Body::empty())?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/album/{album_id}", get(get_album))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The album representation of a studio album holding one jazz medium.
    #[expect(
        clippy::single_call_fn,
        reason = "the album fixture names the case it builds"
    )]
    fn album() -> GetAlbumResponse {
        GetAlbumResponse::new(
            7,
            "Kind of Blue".to_owned(),
            album_release_date(),
            Some(ProductionType::Studio),
            vec!["JAZZ".to_owned()],
            5,
            None,
            vec![GetAlbumMediumItem::new(1, "CD".to_owned(), 5)],
        )
    }

    #[tokio::test]
    async fn get_album_returns_the_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.caller_id() == 1 && command.album_id() == 7)
            .return_once(|_| Box::pin(async { Ok(album()) }));

        // Act
        let (status, content_type, payload) = into_parts(send(use_case, 1, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(
            payload,
            json!({
                "_links": {
                    "self": { "href": "/api/v1/library/music/album/7" }
                },
                "cover_file_id": null,
                "id": 7i64,
                "name": "Kind of Blue",
                "release_date": "1959-08-17",
                "production_type": "STUDIO",
                "genres": ["JAZZ"],
                "track_count": 5i64,
                "mediums": [{
                    "_links": {
                        "self": { "href": "/api/v1/library/music/album/7/medium/1" },
                        "album": { "href": "/api/v1/library/music/album/7" },
                        "tracks": { "href": "/api/v1/library/music/album/7/medium/1/track" }
                    },
                    "index": 1u8,
                    "type": "CD",
                    "track_count": 5i64
                }]
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_with_artwork_exposes_the_cover_link() -> Result<(), Box<dyn Error>> {
        // Arrange: the album's cover is file 19.
        let response = GetAlbumResponse::new(
            7,
            "Kind of Blue".to_owned(),
            album_release_date(),
            None,
            Vec::new(),
            0,
            Some(19),
            Vec::new(),
        );
        let mut use_case = MockGetAlbumUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let copy = response.clone();
            Box::pin(async move { Ok(copy) })
        });

        // Act
        let (status, _, payload) = into_parts(send(use_case, 1, "7").await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("cover_file_id"), Some(&json!(19i64)));
        assert_eq!(
            payload.pointer("/_links/cover/href"),
            Some(&json!("/api/v1/asset/image/19"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_malformed_identifier_returns_bad_request() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockGetAlbumUseCase::new();

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
    async fn get_album_unknown_album_returns_not_found() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetAlbumError::NoSuchAlbum) }));

        // Act
        let response = send(use_case, 1, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        Ok(())
    }

    #[tokio::test]
    async fn get_album_unknown_caller_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockGetAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(|_| Box::pin(async { Err(GetAlbumError::NoSuchCaller) }));

        // Act
        let response = send(use_case, 999, "7").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the authenticated user does not exist")
        );
        Ok(())
    }

    #[tokio::test]
    async fn get_album_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("GET")
            .uri("/music/album/7")
            .body(Body::empty())?;

        // Act
        let response: Response = library_routes(&state).oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
