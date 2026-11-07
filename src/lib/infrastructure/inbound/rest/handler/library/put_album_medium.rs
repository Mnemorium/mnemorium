use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::put_album_medium::PutAlbumMediumCommand;
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    MediumResponse, parse_album_id, parse_write_index,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create or retype a music medium.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutAlbumMediumRequest {
    /// Medium type (`CD` or `DIGITAL`).
    #[schema(example = "CD")]
    #[serde(rename = "type")]
    pub medium_type: String,
}

/// Create or retype a music medium.
///
/// Only a caller whose `library.manage` scope covers the album may write it.
/// The write is idempotent by `(album, index)`: an existing medium keeps its
/// tracks and changes type.
#[utoipa::path(
    put,
    operation_id = "put_album_medium",
    path = "/music/album/{album_id}/medium/{medium_index}",
    tag = "library",
    params(
        ("album_id" = NumericID, Path, description = "Identifier of the album"),
        ("medium_index" = u8, Path, description = "Album-scoped index of the medium"),
    ),
    request_body = PutAlbumMediumRequest,
    responses(
        (
            status = CREATED,
            body = MediumResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created medium"),
            ),
            description = "Medium created",
        ),
        (
            status = OK,
            body = MediumResponse,
            content_type = "application/hal+json",
            description = "Medium retyped"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid album identifier or medium index"
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
            description = "Unknown or invisible album"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The stored medium refuses the write"
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
            description = "The type is not CD or DIGITAL, the index is out of range, or the album is the default one"
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
    summary = "Create or retype a music medium"
)]
pub(crate) async fn put_album_medium(
    Path((album_id_value, medium_index_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PutAlbumMediumRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let album_id = parse_album_id(&album_id_value)?;
    let medium_index = parse_write_index(
        &medium_index_value,
        "medium_index",
        "invalid music medium index",
    )?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .put_album_medium()
        .execute(PutAlbumMediumCommand::new(
            caller.user_id(),
            album_id,
            medium_index,
            request.medium_type,
        ))
        .await?;
    let body = MediumResponse::from_album(response.album(), response.medium_index())
        .ok_or(ApiError::InternalServerError)?;
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    if response.created() {
        *http_response.status_mut() = StatusCode::CREATED;
        http_response.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
        );
    }
    Ok(http_response)
}
