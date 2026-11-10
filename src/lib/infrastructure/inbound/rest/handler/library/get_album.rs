use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::GetAlbumCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{AlbumResponse, parse_album_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Fetch one music album by its identifier.
///
/// Any authenticated caller may read an album; the album is not owned and
/// carries no visibility flag. The representation carries the album's mediums
/// so a client can navigate to each medium and its track collection.
#[utoipa::path(
    get,
    operation_id = "get_album",
    path = "/music/album/{album_id}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album to fetch"),
    ),
    responses(
        (
            status = OK,
            body = AlbumResponse,
            content_type = "application/hal+json",
            description = "Album found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music album identifier"
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
            description = "Unknown or inaccessible music album"
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
    summary = "Fetch a music album"
)]
pub(crate) async fn get_album(
    Path(album_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_album()
        .execute(GetAlbumCommand::new(caller.user_id(), album_id))
        .await?;
    Ok(hal_json(AlbumResponse::from(&response)))
}
