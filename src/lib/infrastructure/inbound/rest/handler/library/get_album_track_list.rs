use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::list_album_tracks::ListAlbumTracksCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{SelfLinks, hal_json};
use crate::infrastructure::inbound::rest::handler::library::{
    TrackListResponse, TrackResponse, parse_album_id, parse_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// List the tracks of one medium of a music album.
///
/// Any authenticated caller may read a medium's tracks; the album is not owned
/// and carries no visibility flag. The collection is returned in play order,
/// ordered by ascending track index.
#[utoipa::path(
    get,
    operation_id = "get_album_track_list",
    path = "/music/album/{album_id}/medium/{medium_index}/track",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album whose medium's tracks are listed"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium whose tracks are listed", maximum = 255u8),
    ),
    responses(
        (
            status = OK,
            body = TrackListResponse,
            content_type = "application/hal+json",
            description = "Tracks of the album medium"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music album identifier or medium index"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The medium index is outside the 8-bit range"
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
            description = "Unknown or inaccessible music album, or a medium the album does not hold"
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
    summary = "List the tracks of a music album medium"
)]
pub(crate) async fn get_album_track_list(
    Path((album_id_value, medium_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let response = state
        .library_use_case_factory()
        .list_album_tracks()
        .execute(ListAlbumTracksCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
        ))
        .await?;
    let body = TrackListResponse {
        items: response.tracks().iter().map(TrackResponse::from).collect(),
        links: SelfLinks::new(&format!(
            "/api/v1/library/music/album/{album_id}/medium/{medium_index}/track"
        )),
    };
    Ok(hal_json(body))
}
