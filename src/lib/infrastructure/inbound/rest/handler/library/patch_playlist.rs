use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::update_playlist::UpdatePlaylistCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{PlaylistResponse, parse_playlist_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// A recording to append to a playlist.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct AddPlaylistTrackRequest {
    /// Identifier of the shared recording to append.
    pub recording_id: NumericID,
}

/// Payload to update a playlist's metadata and/or append a recording.
///
/// Every field is optional. `name` and `is_public` require `playlist.manage`,
/// which the role presets grant to the owner and the Root Admin; `add_track`
/// requires `playlist.moderate`. A request mixing the two from a caller that
/// may moderate but not manage is rejected as a whole.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchPlaylistRequest {
    /// Recording to append to the playlist, when requested.
    pub add_track: Option<AddPlaylistTrackRequest>,
    /// New visibility, when requested.
    pub is_public: Option<bool>,
    /// New name, when requested.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
}

/// Update a playlist's metadata and/or append a recording.
///
/// Omitted fields stay unchanged. `name` and `is_public` require
/// `playlist.manage`; adding a track requires `playlist.moderate`. The request
/// is atomic: a rejected field applies nothing.
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
    payload: Result<Json<PatchPlaylistRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .update_playlist()
        .execute(UpdatePlaylistCommand::new(
            caller.user_id(),
            playlist_id,
            request.name,
            request.is_public,
            request.add_track.map(|add_track| add_track.recording_id),
        ))
        .await?;
    Ok(hal_json(PlaylistResponse::new(response.playlist())))
}
