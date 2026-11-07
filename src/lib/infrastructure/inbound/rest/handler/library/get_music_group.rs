use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_music_group::GetMusicGroupCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{MusicGroupResponse, parse_group_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Fetch one music group by its identifier.
///
/// Any authenticated caller with library access may read a group; the group is
/// not owned and carries no visibility flag. A caller whose library access is
/// `NONE` sees the same `404` as an unknown identifier.
#[utoipa::path(
    get,
    operation_id = "get_music_group",
    path = "/music/group/{group_id}",
    tag = "library",
    params(
        ("group_id" = NumericID, Path, description = "Identifier of the group to fetch"),
    ),
    responses(
        (
            status = OK,
            body = MusicGroupResponse,
            content_type = "application/hal+json",
            description = "Group found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music group identifier"
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
            description = "Unknown or inaccessible music group"
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
    summary = "Fetch a music group"
)]
pub(crate) async fn get_music_group(
    Path(group_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let group_id = parse_group_id(&group_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_music_group()
        .execute(GetMusicGroupCommand::new(caller.user_id(), group_id))
        .await?;
    Ok(hal_json(MusicGroupResponse::from(&response)))
}
