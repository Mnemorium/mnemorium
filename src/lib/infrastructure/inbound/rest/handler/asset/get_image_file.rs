use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse as _, Response};

use crate::application::port::GetImageCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::asset::{asset_file_href, parse_asset_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Redirect the typed image to its backing file.
///
/// The alias authorizes exactly like the image representation and then answers
/// `303 See Other`, so the client continues under the canonical file resource
/// (`design.md`, "One file resource, typed views over it").
#[utoipa::path(
    get,
    operation_id = "get_image_file",
    path = "/image/{image_id}/file",
    tag = "asset",
    params(
        ("image_id" = NumericID, Path, description = "Identifier of the image whose file is addressed"),
    ),
    responses(
        (
            status = SEE_OTHER,
            description = "Redirect to the stored file resource",
            headers(
                ("Location" = String, description = "Canonical URI of the stored file"),
            )
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
    summary = "Redirect an image to its stored file"
)]
pub(crate) async fn get_image_file(
    Path(image_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let image_id = parse_asset_id(&image_id_value, "image_id", "invalid image identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_image()
        .execute(GetImageCommand::new(caller.user_id(), image_id))
        .await?;
    Ok((
        StatusCode::SEE_OTHER,
        [(header::LOCATION, asset_file_href(response.file_id()))],
    )
        .into_response())
}
