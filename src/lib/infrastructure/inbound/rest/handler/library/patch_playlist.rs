use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::FieldUpdate;
use crate::application::port::UpdatePlaylistCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{
    PlaylistResponse, deserialize_field_update, parse_playlist_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// A recording to append to a playlist.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct AddPlaylistTrackRequest {
    /// Identifier of the shared recording to append.
    pub recording_id: NumericID,
}

/// Payload to update a playlist's metadata and/or append a recording.
///
/// Every field is optional. `name`, `is_public` and `cover_image_id` require
/// `EditMetadata` on the playlist; `add_track` requires `CreateChild` for a
/// music recording on the playlist and `ViewMetadata` on the recording. A
/// request that fails one check is rejected as a whole.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchPlaylistRequest {
    /// Recording to append to the playlist, when requested.
    pub add_track: Option<AddPlaylistTrackRequest>,
    /// Replacement identifier of the image holding the playlist's artwork;
    /// `null` clears it.
    #[serde(default, deserialize_with = "deserialize_field_update")]
    #[schema(value_type = Option<NumericID>, nullable = true, example = 1)]
    pub cover_image_id: FieldUpdate<NumericID>,
    /// New visibility, when requested.
    pub is_public: Option<bool>,
    /// New name, when requested.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
}

/// Update a playlist's metadata and/or append a recording.
///
/// Omitted fields stay unchanged. `name`, `is_public` and `cover_image_id`
/// require `EditMetadata` on the playlist; adding a track requires
/// `CreateChild` for a music recording on the playlist and `ViewMetadata` on
/// the recording. The request is atomic: a rejected field applies nothing.
#[utoipa::path(
    patch,
    operation_id = "patch_playlist",
    path = "/music/playlist/{playlist_id}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    request_body = PatchPlaylistRequest,
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier or malformed body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller can read the playlist but may not change its metadata or moderate its tracks"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible playlist"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Duplicate playlist name or duplicate recording"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid name or unknown recording"
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
    summary = "Update a music playlist"
)]
pub(crate) async fn patch_playlist(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PatchPlaylistRequest>,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .update_playlist()
        .execute(UpdatePlaylistCommand::new(
            caller.user_id(),
            playlist_id,
            request.name,
            request.is_public,
            request.add_track.map(|add_track| add_track.recording_id),
            request.cover_image_id,
        ))
        .await?;
    Ok(hal_json(PlaylistResponse::from_playlist(
        response.playlist(),
    )))
}
