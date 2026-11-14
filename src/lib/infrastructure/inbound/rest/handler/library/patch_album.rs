use axum::extract::{Path, State};
use axum::response::Response;
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::FieldUpdate;
use crate::application::port::UpdateMusicAlbumCommand;
use crate::domain::NumericID;
use crate::domain::model::ProductionType;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    AlbumResponse, DATE_FORMAT, ProductionTypeRequest, deserialize_field_update, parse_album_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

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
    #[schema(min_length = 1)]
    pub genres: Option<Vec<String>>,
    /// Identifier of the music group the album belongs to.
    #[schema(example = 1)]
    pub music_group_id: Option<NumericID>,
    /// Name of the album.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
    /// Production type of the album; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<ProductionTypeRequest>, nullable = true)]
    pub production_type: FieldUpdate<ProductionTypeRequest>,
    /// Date of the release in `YYYY-MM-DD` form; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true, format = "date")]
    pub release_date: FieldUpdate<String>,
}

/// Update a music album partially.
///
/// Updating an album requires `View` and then `EditMetadata` on it.
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
    payload: JsonObject<PatchAlbumRequest>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let JsonObject(request) = payload;
    let production_type = match request.production_type {
        FieldUpdate::Absent => FieldUpdate::Absent,
        FieldUpdate::Clear => FieldUpdate::Clear,
        FieldUpdate::Set(value) => FieldUpdate::Set(ProductionType::from(value)),
    };
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
