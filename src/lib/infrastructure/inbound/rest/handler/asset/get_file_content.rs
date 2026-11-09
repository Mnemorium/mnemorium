use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method};
use axum::response::Response;

use crate::application::port::get_asset_content::{AssetTarget, GetAssetContentError};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::{
    ContentAccess, asset_content_response, content_command, parse_asset_id,
};

/// Map an asset-content error to its API error.
impl From<GetAssetContentError> for ApiError {
    fn from(err: GetAssetContentError) -> Self {
        match err {
            GetAssetContentError::NoSuchCaller | GetAssetContentError::InvalidCredential => {
                Self::Unauthorized(err.to_string())
            }
            GetAssetContentError::NoSuchAsset => Self::NotFound(err.to_string()),
            GetAssetContentError::LibraryUnavailable => Self::ServiceUnavailable(err.to_string()),
            GetAssetContentError::BytesMissing | GetAssetContentError::Unknown(_) => {
                Self::InternalServerError
            }
        }
    }
}

/// Read the bytes of a stored file.
///
/// The file is authorized before any byte is read, or the request is
/// authorized by a file-scoped playback credential. An unreadable file answers
/// `404 Not Found` with no content, exactly as an unknown one does.
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
            headers(("Content-Range" = String, description = "Served span and total size"))
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
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid file identifier or ambiguous credential"
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
            description = "Unknown or unreadable file"
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
    access: ContentAccess,
) -> Result<Response, ApiError> {
    let file_id = parse_asset_id(&file_id_value, "file_id", "invalid file identifier")?;
    let command = content_command(access, AssetTarget::File(file_id), &headers, &method);
    let response = state
        .asset_use_case_factory()
        .get_asset_content()
        .execute(command)
        .await?;
    Ok(asset_content_response(response))
}
