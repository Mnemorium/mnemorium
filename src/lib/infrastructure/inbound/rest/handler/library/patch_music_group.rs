use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::update_music_group::UpdateMusicGroupCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{MusicGroupResponse, parse_group_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to update a music group.
///
/// Every field is optional: an omitted field keeps its stored value.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PatchMusicGroupRequest {
    /// Replacement identifier of the image holding the group's artwork.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// New stage name of the group.
    #[schema(min_length = 1, max_length = 100)]
    pub name: Option<String>,
}

/// Update a music group.
///
/// Only a caller whose `library.manage` covers the group may rename it or
/// replace its artwork. The name is trimmed and must be unique; the cover must
/// be a `LIBRARY` image. The seeded default group is protected and refuses
/// every update with `409 Conflict`.
#[utoipa::path(
    patch,
    operation_id = "patch_music_group",
    path = "/music/group/{group_id}",
    tag = "library",
    params(
        ("group_id" = NumericID, Path, description = "Identifier of the group to update"),
    ),
    request_body = PatchMusicGroupRequest,
    responses(
        (
            status = OK,
            body = MusicGroupResponse,
            content_type = "application/hal+json",
            description = "Group updated"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid music group identifier or malformed request body"
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
            description = "The caller may not manage the library"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or inaccessible music group"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The name is taken, the cover is not a library image, or the group is protected"
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
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The group name is empty or too long"
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
    summary = "Update a music group"
)]
pub(crate) async fn patch_music_group(
    Path(group_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PatchMusicGroupRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let group_id = parse_group_id(&group_id_value)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .update_music_group()
        .execute(UpdateMusicGroupCommand::new(
            caller.user_id(),
            group_id,
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let group = response.group();
    Ok(hal_json(MusicGroupResponse::new(
        group.id(),
        group.stage_name().to_owned(),
        group.cover_image_id(),
    )))
}
