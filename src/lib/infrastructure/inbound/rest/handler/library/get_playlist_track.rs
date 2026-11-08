use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_playlist_track::GetPlaylistTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    PlaylistTrackResponse, parse_playlist_id, parse_playlist_track_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one track of one music playlist by its zero-based index.
///
/// Any authenticated caller who can read the playlist may read a track. The
/// track is scoped to the playlist named in the path, so a track of another
/// playlist is reported as missing.
#[utoipa::path(
    get,
    operation_id = "get_playlist_track",
    path = "/music/playlist/{playlist_id}/track/{track_index}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist the track belongs to"),
        ("track_index" = u64, Path, description = "Zero-based index of the track within the playlist", maximum = 4_294_967_295u64),
    ),
    responses(
        (
            status = OK,
            body = PlaylistTrackResponse,
            content_type = "application/hal+json",
            description = "Track found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier or track index"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The track index is outside the 32-bit range"
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
            description = "Unknown or invisible playlist, or unknown track"
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
    summary = "Fetch one track of a music playlist"
)]
pub(crate) async fn get_playlist_track(
    Path((playlist_id_value, track_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let track_index = parse_playlist_track_index(&track_index_value)?;
    let response = state
        .library_use_case_factory()
        .get_playlist_track()
        .execute(GetPlaylistTrackCommand::new(
            caller.user_id(),
            playlist_id,
            track_index,
        ))
        .await?;
    Ok(hal_json(PlaylistTrackResponse::new(
        response.playlist_id(),
        response.track(),
    )))
}
