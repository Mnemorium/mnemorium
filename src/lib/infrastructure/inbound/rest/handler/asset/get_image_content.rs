use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_asset_content::{AssetTarget, GetAssetContentCommand};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::{
    asset_content_response, parse_asset_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Read the bytes of a typed image.
///
/// The image is authorized through the resource that owns it before any byte is
/// read; an unreadable image answers `404 Not Found` with no content.
#[utoipa::path(
    get,
    operation_id = "get_image_content",
    path = "/image/{image_id}/content",
    tag = "asset",
    params(
        ("image_id" = NumericID, Path, description = "Identifier of the image whose content is read"),
    ),
    responses(
        (
            status = OK,
            body = Vec<u8>,
            content_type = "*/*",
            description = "Stored bytes of the image"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid image identifier"
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
            description = "Unknown or unreadable image"
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
    summary = "Read the content of a typed image"
)]
pub(crate) async fn get_image_content(
    Path(image_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let image_id = parse_asset_id(&image_id_value, "image_id", "invalid image identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_asset_content()
        .execute(GetAssetContentCommand::new(
            caller.user_id(),
            AssetTarget::Image(image_id),
        ))
        .await?;
    Ok(asset_content_response(
        response.mime_type_id(),
        response.bytes().to_vec(),
    ))
}
