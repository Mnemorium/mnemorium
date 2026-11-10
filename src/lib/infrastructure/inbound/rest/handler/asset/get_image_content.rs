use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method};
use axum::response::Response;

use crate::application::port::AssetTarget;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::{
    ContentAccess, asset_content_response, content_command, parse_asset_id,
};
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Read the bytes of a typed image.
///
/// The image is authorized through the resource that owns it before any byte is
/// read, or the request is authorized by a playback credential scoped to its
/// backing file. An unreadable image answers `404 Not Found`.
#[utoipa::path(
    get,
    operation_id = "get_image_content",
    path = "/image/{image_id}/content",
    tag = "asset",
    params(
        ("image_id" = NumericID, Path, description = "Identifier of the image whose content is read"),
        ("Range" = Option<String>, Header, description = "Requested byte range, e.g. `bytes=0-99`"),
        ("If-Range" = Option<String>, Header, description = "Validator bounding a conditional range"),
        ("If-None-Match" = Option<String>, Header, description = "Validator suppressing unchanged content"),
    ),
    responses(
        (
            status = OK,
            body = Vec<u8>,
            content_type = "*/*",
            description = "Stored bytes of the image",
            headers(
                ("Accept-Ranges" = String, description = "Always `bytes`"),
                ("ETag" = String, description = "Validator from the stored digest, when present"),
                ("Last-Modified" = String, description = "Modification instant of the content"),
            )
        ),
        (
            status = PARTIAL_CONTENT,
            body = Vec<u8>,
            content_type = "*/*",
            description = "The requested byte range"
        ),
        (
            status = RANGE_NOT_SATISFIABLE,
            description = "The requested range cannot be satisfied"
        ),
        (
            status = NOT_MODIFIED,
            description = "The content matches the presented validator"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid image identifier or ambiguous credential"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing, invalid or expired credential"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or unreadable image"
        ),
        (
            status = SERVICE_UNAVAILABLE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The library root is unavailable"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error or absent file bytes"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Read the content of a typed image"
)]
pub(crate) async fn get_image_content(
    Path(image_id_value): Path<String>,
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    access: ContentAccess,
) -> Result<Response, ApiError> {
    let image_id = parse_asset_id(&image_id_value, "image_id", "invalid image identifier")?;
    let command = content_command(access, AssetTarget::Image(image_id), &headers, &method);
    let response = state
        .asset_use_case_factory()
        .get_asset_content()
        .execute(command)
        .await?;
    Ok(asset_content_response(response))
}
