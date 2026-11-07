use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_album_track::GetAlbumTrackCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    TrackResponse, parse_album_id, parse_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one track of one medium of a music album by its indexes.
///
/// Any authenticated caller may read a track; the album is not owned and carries
/// no visibility flag. The track is scoped to the album and medium named in the
/// path, so a track of another album or medium is reported as missing.
#[utoipa::path(
    get,
    operation_id = "get_album_track",
    path = "/music/album/{album_id}/medium/{medium_index}/track/{track_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album the track belongs to"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium the track is placed in"),
        ("track_index" = u8, Path, description = "Position of the track within its medium"),
    ),
    responses(
        (
            status = OK,
            body = TrackResponse,
            content_type = "application/hal+json",
            description = "Track found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music album, medium or track index"
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
            description = "Unknown or inaccessible music album, or a medium or track the album does not hold"
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
    summary = "Fetch a music track"
)]
pub(crate) async fn get_album_track(
    Path((album_id_value, medium_index_value, track_index_value)): Path<(String, String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let track_index = parse_index(
        &track_index_value,
        "track_index",
        "invalid music track index",
    )?;
    let response = state
        .library_use_case_factory()
        .get_album_track()
        .execute(GetAlbumTrackCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            track_index,
        ))
        .await?;
    Ok(hal_json(TrackResponse::from(response)))
}
