use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::field_update::FieldUpdate;
use crate::application::port::update_music_album::UpdateMusicAlbumCommand;
use crate::domain::alias::NumericID;
use crate::domain::model::music_album::ProductionType;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::AlbumResponse;
use crate::infrastructure::inbound::rest::handler::library::DATE_FORMAT;
use crate::infrastructure::inbound::rest::handler::library::deserialize_field_update;
use crate::infrastructure::inbound::rest::handler::library::parse_album_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a music album partially.
///
/// An absent field keeps its value; a `null` clears an optional value.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchAlbumRequest {
    /// Identifier of the image holding the album's artwork; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<NumericID>, nullable = true)]
    pub cover_image_id: FieldUpdate<NumericID>,
    /// Genres of the album; omit to keep them.
    pub genres: Option<Vec<String>>,
    /// Identifier of the music group the album belongs to.
    #[schema(example = 1)]
    pub music_group_id: Option<NumericID>,
    /// Name of the album.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
    /// Production type of the album; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub production_type: FieldUpdate<String>,
    /// Date of the release in `YYYY-MM-DD` form; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true)]
    pub release_date: FieldUpdate<String>,
}

/// Update a music album partially.
///
/// Only a caller whose `library.manage` scope covers the album may update it.
/// An absent field keeps its value and a `null` clears an optional value.
#[utoipa::path(
    patch,
    operation_id = "patch_album",
    path = "/music/album/{album_id}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album to update"),
    ),
    request_body = PatchAlbumRequest,
    responses(
        (
            status = OK,
            body = AlbumResponse,
            content_type = "application/hal+json",
            description = "Album updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or malformed body"
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
            description = "The identity duplicates another album, or the cover is not a library image"
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
    summary = "Update a music album"
)]
pub(crate) async fn patch_album(
    Path(album_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchAlbumRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let production_type = request.production_type.try_map(|value| {
        ProductionType::from_wire(&value).ok_or_else(|| ApiError::reject_enum("production_type"))
    })?;
    let release_date = request.release_date.try_map(|value| {
        NaiveDate::parse_from_str(&value, DATE_FORMAT).map_err(|_| {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "release_date",
                reason = "invalid_date",
                "rejected a malformed release date"
            );
            ApiError::UnprocessableEntity("the release date must use YYYY-MM-DD".to_owned())
        })
    })?;
    let response = state
        .library_use_case_factory()
        .update_music_album()
        .execute(UpdateMusicAlbumCommand::new(
            caller.user_id(),
            album_id,
            request.name,
            request.music_group_id,
            release_date,
            production_type,
            request.genres,
            request.cover_image_id,
        ))
        .await?;
    Ok(hal_json(AlbumResponse::from(response.album())))
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
    use axum::routing::patch;
    use rstest::rstest;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::patch_album;
    use crate::application::field_update::FieldUpdate;
    use crate::application::port::library_use_case_factory::MockLibraryUseCaseFactory;
    use crate::application::port::update_music_album::MockUpdateMusicAlbumUseCase;
    use crate::application::port::update_music_album::UpdateMusicAlbumError;
    use crate::application::port::update_music_album::UpdateMusicAlbumResponse;
    use crate::application::port::update_music_album::UpdateMusicAlbumUseCase;
    use crate::domain::alias::NumericID;
    use crate::infrastructure::inbound::rest::handler::library::library_routes;
    use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;
    use crate::test_helpers::app_state_with_library;
    use crate::test_helpers::error_message_of;
    use crate::test_helpers::into_parts;
    use crate::test_helpers::sample_album;

    /// Send a PATCH to `/api/v1/library/music/album/{album}` on behalf of
    /// `caller_id`.
    async fn send(
        use_case: MockUpdateMusicAlbumUseCase,
        caller_id: NumericID,
        album_id: &str,
        body: &str,
    ) -> Result<Response, Box<dyn Error>> {
        let mut factory = MockLibraryUseCaseFactory::new();
        factory
            .expect_update_music_album()
            .times(0..=1)
            .return_once(move || Arc::new(use_case) as Arc<dyn UpdateMusicAlbumUseCase>);

        let mut request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/v1/library/music/album/{album_id}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        request
            .extensions_mut()
            .insert(AuthenticatedUser::from(caller_id));

        let router = axum::Router::new()
            .route("/api/v1/library/music/album/{album_id}", patch(patch_album))
            .with_state(app_state_with_library(Arc::new(factory))?);
        Ok(router.oneshot(request).await?)
    }

    #[tokio::test]
    async fn patch_album_updates_the_album() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let mut use_case = MockUpdateMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| {
                command.caller_id() == 1
                    && command.album_id() == 7
                    && command.name() == Some("Kind of Blue")
            })
            .return_once(move |_| {
                let stored = UpdateMusicAlbumResponse::new(album.clone());
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let (status, _, payload) =
            into_parts(send(use_case, 1, "7", r#"{"name":"Kind of Blue"}"#).await?).await?;

        // Assert
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.get("id"), Some(&json!(7i64)));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_null_clears_the_release_date() -> Result<(), Box<dyn Error>> {
        // Arrange
        let album = sample_album(7, Vec::new())?;
        let mut use_case = MockUpdateMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .withf(|command| command.release_date() == FieldUpdate::Clear)
            .return_once(move |_| {
                let stored = UpdateMusicAlbumResponse::new(album.clone());
                Box::pin(async move { Ok(stored) })
            });

        // Act
        let response = send(use_case, 1, "7", r#"{"release_date":null}"#).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[rstest]
    #[case::conflict(
        UpdateMusicAlbumError::Conflict,
        StatusCode::CONFLICT,
        "a music album with this name and group already exists"
    )]
    #[case::unknown(
        UpdateMusicAlbumError::NoSuchAlbum,
        StatusCode::NOT_FOUND,
        "a music album with this identifier does not exist"
    )]
    #[case::invalid(
        UpdateMusicAlbumError::InvalidName,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the name must not be empty"
    )]
    #[tokio::test]
    async fn patch_album_error_maps_to_status(
        #[case] error: UpdateMusicAlbumError,
        #[case] status: StatusCode,
        #[case] message: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockUpdateMusicAlbumUseCase::new();
        use_case
            .expect_execute()
            .times(1)
            .return_once(move |_| Box::pin(async { Err(error) }));

        // Act
        let response = send(use_case, 1, "7", r#"{"name":"Kind of Blue"}"#).await?;

        // Assert
        assert_eq!(response.status(), status);
        assert_eq!(error_message_of(&response).as_deref(), Some(message));
        Ok(())
    }

    #[tokio::test]
    async fn patch_album_missing_credentials_returns_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange: the real router, wrapped in its `authenticate` layer, with no
        // authorization header.
        let state = app_state_with_library(Arc::new(MockLibraryUseCaseFactory::new()))?;
        let request = Request::builder()
            .method("PATCH")
            .uri("/music/album/7")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"name":"Kind of Blue"}"#))?;

        // Act
        let response: Response = axum::Router::from(library_routes(&state))
            .oneshot(request)
            .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }
}
