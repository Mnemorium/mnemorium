use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::GetPlaylistCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{PlaylistResponse, parse_playlist_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Fetch one music playlist through the collection alias.
///
/// This is a literal alias of `GET /library/music/playlist/{playlist_id}`: it
/// returns the same playlist representation with its tracks embedded, and
/// `_links.self` keeps naming the canonical playlist URI. The playlist's
/// `tracks` link points here.
#[utoipa::path(
    get,
    operation_id = "get_playlist_track_list",
    path = "/music/playlist/{playlist_id}/track",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist found with its tracks embedded"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid playlist identifier"
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
            description = "Unknown or invisible playlist"
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
    summary = "Fetch a music playlist's tracks"
)]
pub(crate) async fn get_playlist_track_list(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_playlist()
        .execute(GetPlaylistCommand::new(caller.user_id(), playlist_id))
        .await?;
    Ok(hal_json(PlaylistResponse::new(response.playlist())))
}
