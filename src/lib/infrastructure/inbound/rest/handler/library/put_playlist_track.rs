use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::reorder_playlist_tracks::ReorderPlaylistTracksCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::{PlaylistResponse, parse_playlist_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to reorder the tracks of a playlist.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ReorderPlaylistTracksRequest {
    /// Complete permutation of the playlist's current zero-based track indices.
    pub order: Vec<u32>,
}

/// Reorder the tracks of a music playlist.
///
/// Any caller who can read the playlist may reorder it. The body must be a
/// complete permutation of the current track indices; the tracks are reindexed
/// to `0..n-1` in the requested order.
#[utoipa::path(
    put,
    operation_id = "put_playlist_track",
    path = "/music/playlist/{playlist_id}/track",
    tag = "library",
    params(
        ("playlist_id" = NumericID, Path, description = "Identifier of the playlist"),
    ),
    request_body = ReorderPlaylistTracksRequest,
    responses(
        (
            status = OK,
            body = PlaylistResponse,
            content_type = "application/hal+json",
            description = "Playlist reordered"
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
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or invisible playlist"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The order is not a complete permutation of the current indices"
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
    summary = "Reorder the tracks of a music playlist"
)]
pub(crate) async fn put_playlist_track(
    Path(playlist_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<ReorderPlaylistTracksRequest>,
) -> Result<Response, ApiError> {
    let playlist_id = parse_playlist_id(&playlist_id_value)?;
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .reorder_playlist_tracks()
        .execute(ReorderPlaylistTracksCommand::new(
            caller.user_id(),
            playlist_id,
            request.order,
        ))
        .await?;
    Ok(hal_json(PlaylistResponse::new(response.playlist())))
}
