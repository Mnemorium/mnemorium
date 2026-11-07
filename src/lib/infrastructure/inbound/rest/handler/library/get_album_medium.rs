use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_album_medium::GetAlbumMediumCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    MediumResponse, parse_album_id, parse_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one medium of a music album by its album identifier and medium index.
///
/// Any authenticated caller may read a medium; the album is not owned and
/// carries no visibility flag. The medium is scoped to the album named in the
/// path, so a medium of another album is reported as missing.
#[utoipa::path(
    get,
    operation_id = "get_album_medium",
    path = "/music/album/{album_id}/medium/{medium_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album the medium belongs to"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium to fetch"),
    ),
    responses(
        (
            status = OK,
            body = MediumResponse,
            content_type = "application/hal+json",
            description = "Medium found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music album identifier or medium index"
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
    summary = "Fetch a music medium"
)]
pub(crate) async fn get_album_medium(
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
        .get_album_medium()
        .execute(GetAlbumMediumCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
        ))
        .await?;
    Ok(hal_json(MediumResponse::from(response)))
}
