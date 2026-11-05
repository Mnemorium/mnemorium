use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::create_music_album::CreateMusicAlbumCommand;
use crate::domain::alias::NumericID;
use crate::domain::model::music_album::ProductionType;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::AlbumResponse;
use crate::infrastructure::inbound::rest::handler::library::DATE_FORMAT;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a music album.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostAlbumRequest {
    /// Identifier of the image holding the album's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Genres of the album; omit to store the `UNKNOWN` genre.
    pub genres: Option<Vec<String>>,
    /// Identifier of the music group the album belongs to.
    #[schema(example = 1)]
    pub music_group_id: NumericID,
    /// Name of the album.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
    /// Production type of the album (`STUDIO`, `LIVE`, …), when set.
    pub production_type: Option<String>,
    /// Date of the release, in `YYYY-MM-DD` form, when known.
    #[schema(example = "1959-08-17")]
    pub release_date: Option<String>,
}

/// Create a music album.
///
/// Only a caller whose `library.manage` scope covers the whole catalogue may
/// create an album. The write is an upsert by `(name, group)`: an album whose
/// identity already exists is returned unchanged with `200 OK`.
#[utoipa::path(
    post,
    operation_id = "post_album",
    path = "/music/album",
    tag = "library",
    request_body = PostAlbumRequest,
    responses(
        (
            status = CREATED,
            body = AlbumResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created album"),
            ),
            description = "Album created",
        ),
        (
            status = OK,
            body = AlbumResponse,
            content_type = "application/hal+json",
            description = "Album already existed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
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
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The cover is not a library image"
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
            description = "The name, group, genre, production type or release date is invalid"
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
    summary = "Create a music album"
)]
pub(crate) async fn post_album(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostAlbumRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let production_type = match request.production_type.as_deref() {
        Some(value) => Some(
            ProductionType::from_wire(value)
                .ok_or_else(|| ApiError::reject_enum("production_type"))?,
        ),
        None => None,
    };
    let release_date = match request.release_date.as_deref() {
        Some(value) => Some(NaiveDate::parse_from_str(value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "release_date",
                reason = "invalid_date",
                "rejected a malformed release date"
            );
            ApiError::UnprocessableEntity("the release date must use YYYY-MM-DD".to_owned())
        })?),
        None => None,
    };
    let response = state
        .library_use_case_factory()
        .create_music_album()
        .execute(CreateMusicAlbumCommand::new(
            caller.user_id(),
            request.name,
            request.music_group_id,
            release_date,
            production_type,
            request.genres.unwrap_or_default(),
            request.cover_image_id,
        ))
        .await?;
    let body = AlbumResponse::from(response.album());
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    if response.created() {
        *http_response.status_mut() = StatusCode::CREATED;
        http_response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
        );
    }
    Ok(http_response)
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
    use axum::routing::post;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::post_album;
    use crate::application::port::create_music_album::CreateMusicAlbumError;
    use crate::application::port::create_music_album::CreateMusicAlbumResponse;
    use crate::application::port::create_music_album::CreateMusicAlbumUseCase;
    use crate::application::port::create_music_album::MockCreateMusicAlbumUseCase;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_album;

    /// Send a POST to `/api/v1/library/music/album` on behalf of `caller_id`.
    async fn send(
        use_case: MockCreateMusicAlbumUseCase,
        caller_id: NumericID,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_create_music_album()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn CreateMusicAlbumUseCase>);

        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/library/music/album")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/album", post(post_album))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn post_album_creates_the_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let mut use_case = MockCreateMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.name() == "Kind of Blue"
                    && command.music_group_id() == 0
            })
            .return_once(move |_| {
                let stored = CreateMusicAlbumResponse::new(album.clone(), true);
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let (status, content_type, payload) =
            into_parts(send(use_case, 1, r#"{"name":"Kind of Blue","music_group_id":0}"#).await?)
                .await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(content_type.as_deref(), Some("application/hal+json"));
        assert_eq!(payload.get("id"), Some(&json!(7i64)));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/music/album/7"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_album_returns_the_location_of_the_new_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let mut use_case = MockCreateMusicAlbumUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let stored = CreateMusicAlbumResponse::new(album.clone(), true);
            Box::pin(async move { Ok(stored) })
        });

        // Act
        let response = send(use_case, 1, r#"{"name":"Kind of Blue","music_group_id":0}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("/api/v1/library/music/album/7")
        );
        Ok(())
    }

    #[tokio::test]
    async fn post_album_existing_identity_returns_ok() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let mut use_case = MockCreateMusicAlbumUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let stored = CreateMusicAlbumResponse::new(album.clone(), false);
            Box::pin(async move { Ok(stored) })
        });

        // Act
        let response = send(use_case, 1, r#"{"name":"Kind of Blue","music_group_id":0}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::LOCATION).is_none());
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        CreateMusicAlbumError::Conflict,
        StatusCode::CONFLICT,
        "the music album cannot be created"
    )]
    #[case::invalid_name(
        CreateMusicAlbumError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[case::missing_group(
        CreateMusicAlbumError::NoSuchGroup,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a music group with this identifier does not exist"
    )]
    #[tokio::test]
    async fn post_album_error_maps_to_status(
        #[case] error: CreateMusicAlbumError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockCreateMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, r#"{"name":"Kind of Blue","music_group_id":0}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn post_album_invalid_date_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockCreateMusicAlbumUseCase::new();

        // Act
        let response = send(
            use_case,
            1,
            r#"{"name":"Kind of Blue","music_group_id":0,"release_date":"1959-13-01"}"#,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn post_album_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("POST")
            .uri("/music/album")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"name":"Kind of Blue","music_group_id":0}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
