use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::{GetGalleryItemCommand, GetGalleryItemError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{
    GalleryItemDetailResponse, parse_gallery_id, parse_item_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Map a fetch-gallery-item error to its API error.
impl From<GetGalleryItemError> for ApiError {
    fn from(err: GetGalleryItemError) -> Self {
        match err {
            GetGalleryItemError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetGalleryItemError::NoSuchGallery | GetGalleryItemError::NoSuchItem => {
                Self::NotFound(err.to_string())
            }
            GetGalleryItemError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one item of a gallery by its identifiers.
///
/// A caller who may `View` the gallery reads one of its items; an item of a
/// gallery the caller cannot read answers `404 Not Found`. The item is scoped
/// to the gallery named in the path.
#[utoipa::path(
    get,
    operation_id = "get_gallery_item",
    path = "/gallery/{id}/item/{item_id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery the item belongs to"),
        ("item_id" = NumericID, Path, description = "Identifier of the item to fetch"),
    ),
    responses(
        (
            status = OK,
            body = GalleryItemDetailResponse,
            content_type = "application/hal+json",
            description = "Item found"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery or item identifier"
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
            description = "Unknown or inaccessible gallery, or unknown item"
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
    summary = "Fetch a gallery item"
)]
pub(crate) async fn get_gallery_item(
    Path((gallery_id_value, item_id_value)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let gallery_id = parse_gallery_id(&gallery_id_value)?;
    let item_id = parse_item_id(&item_id_value)?;
    let response = state
        .library_use_case_factory()
        .get_gallery_item()
        .execute(GetGalleryItemCommand::new(
            caller.user_id(),
            gallery_id,
            item_id,
        ))
        .await?;
    Ok(hal_json(GalleryItemDetailResponse::from(response)))
}
