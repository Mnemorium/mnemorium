use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method};
use axum::response::Response;

use crate::application::port::AssetTarget;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::{
    asset_content_response, content_command, content_disposition, parse_asset_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};
use axum::http::header;

/// Read the bytes of a stored file.
///
/// The request is authenticated and the file's `Download` action is decided
/// before any byte is read. An asset the caller cannot discover answers
/// `404 Not Found`; a discoverable one whose action refuses answers `403
/// Forbidden`; neither reads content. The response is an attachment naming the
/// file's original name when a typed media row stores one.
#[utoipa::path(
    get,
    operation_id = "get_file_content",
    path = "/file/{file_id}/content",
    tag = "asset",
    params(
        ("file_id" = NumericID, Path, description = "Identifier of the file whose content is read"),
        ("Range" = Option<String>, Header, description = "Requested byte range, e.g. `bytes=0-99`"),
        ("If-Range" = Option<String>, Header, description = "Validator bounding a conditional range"),
        ("If-None-Match" = Option<String>, Header, description = "Validator suppressing unchanged content"),
    ),
    responses(
        (
            status = OK,
            body = Vec<u8>,
            content_type = "*/*",
            description = "Stored bytes of the file",
            headers(
                ("Accept-Ranges" = String, description = "Always `bytes`"),
                ("Content-Disposition" = String, description = "Attachment naming the file's original name"),
                ("Content-Range" = String, description = "Served span and total size"),
                ("Content-Length" = String, description = "Length of the served span"),
                ("ETag" = String, description = "Validator from the stored digest, when present"),
                ("Last-Modified" = String, description = "Modification instant of the content"),
            )
        ),
        (
            status = PARTIAL_CONTENT,
            body = Vec<u8>,
            content_type = "*/*",
            description = "The requested byte range",
            headers(
                ("Content-Disposition" = String, description = "Attachment naming the file's original name"),
                ("Content-Range" = String, description = "Served span and total size"),
            )
        ),
        (
            status = RANGE_NOT_SATISFIABLE,
            description = "The requested range cannot be satisfied",
            headers(("Content-Range" = String, description = "`bytes */<size>`"))
        ),
        (
            status = NOT_MODIFIED,
            description = "The content matches the presented validator"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing, invalid or expired session token"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller can discover the file but may not download it"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or undiscoverable file"
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
    summary = "Read the content of a stored file"
)]
pub(crate) async fn get_file_content(
    Path(file_id_value): Path<String>,
    State(state): State<AppState>,
    method: Method,
    headers: HeaderMap,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let file_id = parse_asset_id(&file_id_value, "file_id", "invalid file identifier")?;
    let command = content_command(
        caller.user_id(),
        AssetTarget::File(file_id),
        &headers,
        &method,
    );
    let response = state
        .asset_use_case_factory()
        .get_asset_content()
        .execute(command)
        .await?;
    let disposition_name = response.file_name().map(str::to_owned);
    let mut rendered = asset_content_response(response);
    if let Some(name) = disposition_name {
        rendered
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, content_disposition(&name));
    }
    Ok(rendered)
}
