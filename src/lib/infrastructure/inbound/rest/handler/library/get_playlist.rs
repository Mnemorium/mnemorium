use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_playlist::GetPlaylistCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{PlaylistResponse, parse_playlist_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one music playlist with its tracks.
///
/// Any authenticated caller who can read the playlist may fetch it: a public
/// playlist is readable by all, a private one only by its owner and the Root
/// Admin. A playlist the caller cannot read answers `404`, exactly like a
/// missing one.
#[utoipa::path(
    get,
    operation_id = "get_playlist",
    path = "/music/playlist/{playlist_id}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist found"
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
    summary = "Fetch a music playlist"
)]
pub(crate) async fn get_playlist(
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
