use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse as _, Response};

use crate::application::port::remove_playlist_track::RemovePlaylistTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::{
    parse_playlist_id, parse_playlist_track_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Remove one track from a music playlist.
///
/// Any caller who can read the playlist may remove a track. The remaining
/// tracks keep their indices, leaving a gap. The shared recording and its media
/// survive.
#[utoipa::path(
    delete,
    operation_id = "delete_playlist_track",
    path = "/music/playlist/{playlist_id}/track/{track_index}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist the track belongs to"),
        ("track_index" = u32, Path, description = "Zero-based index of the track within the playlist"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Track removed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier or track index"
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
    summary = "Remove one track of a music playlist"
)]
pub(crate) async fn delete_playlist_track(
    Path((playlist_id_value, track_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let track_index = parse_playlist_track_index(&track_index_value)?;
    state
        .library_use_case_factory()
        .remove_playlist_track()
        .execute(RemovePlaylistTrackCommand::new(
            caller.user_id(),
            playlist_id,
            track_index,
        ))
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
