use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_asset_content::{
    AssetTarget, GetAssetContentCommand, GetAssetContentError,
};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::{
    asset_content_response, parse_asset_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map an asset-content error to its API error.
impl From<GetAssetContentError> for ApiError {
    fn from(err: GetAssetContentError) -> Self {
        match err {
            GetAssetContentError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAssetContentError::NoSuchAsset => Self::NotFound(err.to_string()),
            GetAssetContentError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Read the bytes of a stored file.
///
/// The file is authorized before any byte is read, so an unreadable file
/// answers `404 Not Found` with no content, exactly as an unknown one does.
#[utoipa::path(
    get,
    operation_id = "get_file_content",
    path = "/file/{file_id}/content",
    tag = "asset",
    params(
        ("file_id" = NumericID, Path, description = "Identifier of the file whose content is read"),
    ),
    responses(
        (
            status = OK,
            body = Vec<u8>,
            content_type = "*/*",
            description = "Stored bytes of the file"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid file identifier"
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
            description = "Unknown or unreadable file"
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
    summary = "Read the content of a stored file"
)]
pub(crate) async fn get_file_content(
    Path(file_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let file_id = parse_asset_id(&file_id_value, "file_id", "invalid file identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_asset_content()
        .execute(GetAssetContentCommand::new(
            caller.user_id(),
            AssetTarget::File(file_id),
        ))
        .await?;
    Ok(asset_content_response(
        response.mime_type_id(),
        response.bytes().to_vec(),
    ))
}
