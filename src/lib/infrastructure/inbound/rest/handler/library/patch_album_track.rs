use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::FieldUpdate;
use crate::application::port::PatchAlbumTrackCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    TrackResponse, deserialize_field_update, parse_album_id, parse_write_index,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Payload to edit the shared recording behind a track address.
///
/// An absent field keeps its value; a `null` clears an optional value.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchAlbumTrackRequest {
    /// ISRC code of the recording; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true, min_length = 12, max_length = 12)]
    pub isrc_code: FieldUpdate<String>,
    /// Name of the recording; `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<String>, nullable = true, min_length = 1)]
    pub name: FieldUpdate<String>,
}

/// Edit the shared recording behind a track address.
///
/// Editing a track's metadata requires `View` and then `EditMetadata` on the
/// album. The recording is shared, so the change is visible in every playlist
/// that references it.
#[utoipa::path(
    patch,
    operation_id = "patch_album_track",
    path = "/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium", maximum = 255u8),
        ("track_index" = u8, Path, description = "Index of the track within the medium", maximum = 255u8),
    ),
    request_body = PatchAlbumTrackRequest,
    responses(
        (
            status = OK,
            body = TrackResponse,
            content_type = "application/hal+json",
            description = "Recording updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or track address"
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
            description = "Unknown or invisible album, medium or track"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The ISRC code is used by another recording"
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
            description = "The name is empty, the ISRC code is not twelve characters, an index is out of range, or the album is the default one"
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
    summary = "Edit the shared recording behind a track"
)]
pub(crate) async fn patch_album_track(
    Path((album_id_value, medium_index_value, track_index_value)): Path<(String, String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PatchAlbumTrackRequest>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_write_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let track_index = parse_write_index(
        &track_index_value,
        "track_index",
        "invalid music track index",
    )?;
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .patch_album_track()
        .execute(PatchAlbumTrackCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            track_index,
            request.name,
            request.isrc_code,
        ))
        .await?;
    let body = TrackResponse::from_album(
        response.album(),
        response.medium_index(),
        response.track_index(),
    )
    .ok_or(ApiError::InternalServerError)?;
    Ok(hal_json(body))
}
