use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse as _, Response};

use crate::application::port::delete_playlist::DeletePlaylistCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::library::parse_playlist_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Delete a music playlist and its track placements.
///
/// The owner and the Root Admin may delete a playlist. A caller who can read a
/// playlist but may not manage it answers `403 Forbidden`; one that cannot read
/// it answers `404 Not Found`. The shared recordings, audio and files
/// survive.
#[utoipa::path(
    delete,
    operation_id = "delete_playlist",
    path = "/music/playlist/{playlist_id}",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Playlist deleted"
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller can read the playlist but may not manage it"
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
    summary = "Delete a music playlist"
)]
pub(crate) async fn delete_playlist(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    state
        .library_use_case_factory()
        .delete_playlist()
        .execute(DeletePlaylistCommand::new(caller.user_id(), playlist_id))
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
