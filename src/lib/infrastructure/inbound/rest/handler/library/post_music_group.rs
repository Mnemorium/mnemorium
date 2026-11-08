use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::create_music_group::CreateMusicGroupCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::library::MusicGroupResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a music group.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostMusicGroupRequest {
    /// Identifier of the image holding the group's artwork, when set.
    #[schema(example = 1)]
    pub cover_image_id: Option<NumericID>,
    /// Stage name of the group.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
}

/// Create a music group.
///
/// Only a caller whose `library.manage` covers the catalogue may create a
/// group. The name is trimmed and must be unique; the optional cover must be a
/// `LIBRARY` image.
#[utoipa::path(
    post,
    operation_id = "post_music_group",
    path = "/music/group",
    tag = "library",
    request_body = PostMusicGroupRequest,
    responses(
        (
            status = CREATED,
            body = MusicGroupResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created group"),
            ),
            description = "Group created",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
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
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The name is taken, or the cover is not a library image"
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
    summary = "Create a music group"
)]
pub(crate) async fn post_music_group(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PostMusicGroupRequest>,
) -> Result<Response, ApiError> {
    let JsonObject(request) = payload;
    let response = state
        .library_use_case_factory()
        .create_music_group()
        .execute(CreateMusicGroupCommand::new(
            caller.user_id(),
            request.name,
            request.cover_image_id,
        ))
        .await?;
    let group = response.group();
    let body = MusicGroupResponse::new(
        group.id(),
        group.stage_name().to_owned(),
        group.cover_image_id(),
    );
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}
