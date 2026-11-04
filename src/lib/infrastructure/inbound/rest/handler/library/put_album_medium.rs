use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::put_album_medium::PutAlbumMediumCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::MediumResponse;
use crate::infrastructure::inbound::rest::handler::library::parse_album_id;
use crate::infrastructure::inbound::rest::handler::library::parse_write_index;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create or retype a music medium.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PutAlbumMediumRequest {
    /// Medium type (`CD` or `DIGITAL`).
    #[schema(example = "CD")]
    #[serde(rename = "type")]
    pub medium_type: String,
}

/// Create or retype a music medium.
///
/// Only a caller whose `library.manage` scope covers the album may write it.
/// The write is idempotent by `(album, index)`: an existing medium keeps its
/// tracks and changes type.
#[utoipa::path(
    put,
    operation_id = "put_album_medium",
    path = "/music/album/{album_id}/medium/{medium_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium"),
    ),
    request_body = PutAlbumMediumRequest,
    responses(
        (
            status = CREATED,
            body = MediumResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created medium"),
            ),
            description = "Medium created",
        ),
        (
            status = OK,
            body = MediumResponse,
            content_type = "application/hal+json",
            description = "Medium retyped"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or medium index"
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
            description = "Unknown or invisible album"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The stored medium refuses the write"
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
            description = "The type is not CD or DIGITAL, the index is out of range, or the album is the default one"
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
    summary = "Create or retype a music medium"
)]
pub async fn put_album_medium(
    Path((album_id_value, medium_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PutAlbumMediumRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_write_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .put_album_medium()
        .execute(PutAlbumMediumCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            request.medium_type,
        ))
        .await?;
    let body = MediumResponse::from_album(response.album(), response.medium_index())
        .ok_or(ApiError::InternalServerError)?;
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
    use axum::routing::put;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::put_album_medium;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::put_album_medium::MockPutAlbumMediumUseCase;
    use crate::application::port::put_album_medium::PutAlbumMediumError;
    use crate::application::port::put_album_medium::PutAlbumMediumResponse;
    use crate::application::port::put_album_medium::PutAlbumMediumUseCase;
    use crate::domain::alias::NumericID;
    use crate::domain::model::music_album::MusicAlbum;
    use crate::domain::model::music_medium::MediumIndex;
    use crate::domain::model::music_medium::MusicMedium;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_album;

    /// Send a PUT to the medium route on behalf of `caller_id`.
    async fn send(
        use_case: MockPutAlbumMediumUseCase,
        caller_id: NumericID,
        album_id: &str,
        medium_index: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_put_album_medium()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn PutAlbumMediumUseCase>);

        let mut request = Request::builder()
            .method("PUT")
            .uri(format!(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route(
                "/api/v1/library/music/album/{album_id}/medium/{medium_index}",
                put(put_album_medium),
            )
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    /// The album holding one empty `CD` medium at index 1.
    fn album() -> Result<MusicAlbum, Box<dyn Error>> {
        sample_album(7, vec![MusicMedium::new(MediumIndex::Cd(1), Vec::new())])
    }

    #[tokio::test]
    async fn put_album_medium_creates_the_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = album()?;
        let mut use_case = MockPutAlbumMediumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.album_id() == 7
                    && command.medium_index() == 1
                    && command.medium_type() == "CD"
            })
            .return_once(move |_| {
                let response = PutAlbumMediumResponse::new(stored.clone(), 1, true);
                Box::pin(async move { Ok(response) })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "7", "1", r#"{"type":"CD"}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(payload.get("type"), Some(&json!("CD")));
        assert_eq!(
            payload.pointer("/_links/self/href"),
            Some(&json!("/api/v1/library/music/album/7/medium/1"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_retypes_the_medium() -> Result<(), Box<dyn Error>> {
        // Arrange
        let stored = album()?;
        let mut use_case = MockPutAlbumMediumUseCase::new();
        use_case.expect_execute().times(1).return_once(move |_| {
            let response = PutAlbumMediumResponse::new(stored.clone(), 1, false);
            Box::pin(async move { Ok(response) })
        });

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"type":"CD"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::invalid_type(
        PutAlbumMediumError::InvalidMediumType,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the medium type must be CD or DIGITAL"
    )]
    #[case::default_album(
        PutAlbumMediumError::DefaultAlbum,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the default music album cannot be written"
    )]
    #[case::unknown_album(
        PutAlbumMediumError::NoSuchAlbum,
        StatusCode::NOT_FOUND,
        "a music album with this identifier does not exist"
    )]
    #[case::conflict(
        PutAlbumMediumError::Conflict,
        StatusCode::CONFLICT,
        "the music medium cannot be written"
    )]
    #[tokio::test]
    async fn put_album_medium_error_maps_to_status(
        #[case] error: PutAlbumMediumError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockPutAlbumMediumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", "1", r#"{"type":"VINYL"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_out_of_range_index_is_unprocessable() -> Result<(), Box<dyn Error>> {
        // Arrange
        let use_case = MockPutAlbumMediumUseCase::new();

        // Act
        let response = send(use_case, 1, "7", "256", r#"{"type":"CD"}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        Ok(())
    }

    #[tokio::test]
    async fn put_album_medium_missing_credentials_returns_unauthorized()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PUT")
            .uri("/music/album/7/medium/1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"type":"CD"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
