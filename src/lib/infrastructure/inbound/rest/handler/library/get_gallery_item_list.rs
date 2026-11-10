use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::GetGalleryCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{GalleryResponse, parse_gallery_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// List the items of a gallery.
///
/// This route is a true alias of `GET /library/gallery/{id}`: it returns the
/// same gallery representation, and the `_links.self` of that representation is
/// the canonical gallery URI, never this alias path. A gallery the caller
/// cannot read answers `404 Not Found`, indistinguishable from a missing one.
#[utoipa::path(
    get,
    operation_id = "get_gallery_item_list",
    path = "/gallery/{id}/item",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery whose items are listed"),
    ),
    responses(
        (
            status = OK,
            body = GalleryResponse,
            content_type = "application/hal+json",
            description = "Gallery found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery identifier"
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
            description = "Unknown or inaccessible gallery"
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
    summary = "List the items of a gallery"
)]
pub(crate) async fn get_gallery_item_list(
    Path(id): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let gallery_id = parse_gallery_id(&id)?;
    let response = state
        .library_use_case_factory()
        .get_gallery()
        .execute(GetGalleryCommand::new(caller.user_id(), gallery_id))
        .await?;
    Ok(hal_json(GalleryResponse::from(response)))
}
