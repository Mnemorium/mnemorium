use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::application::port::{DeleteGalleryItemCommand, DeleteGalleryItemError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{parse_gallery_id, parse_item_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Map a delete-gallery-item error to its API error.
impl From<DeleteGalleryItemError> for ApiError {
    fn from(err: DeleteGalleryItemError) -> Self {
        match err {
            DeleteGalleryItemError::Forbidden => Self::Forbidden(err.to_string()),
            DeleteGalleryItemError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            DeleteGalleryItemError::NoSuchGallery | DeleteGalleryItemError::NoSuchItem => {
                Self::NotFound(err.to_string())
            }
            DeleteGalleryItemError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Delete one item of a gallery by its identifiers.
///
/// Removing an item requires the `Delete` action on the gallery, which its owner
/// and the Root Admin hold. The item is scoped to the gallery named in the path.
/// The item and its media row are removed; the backing file row is removed only
/// where the caller may view the file, and the stored bytes are never touched.
#[utoipa::path(
    delete,
    operation_id = "delete_gallery_item",
    path = "/gallery/{id}/item/{item_id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery the item belongs to"),
        ("item_id" = NumericID, Path, description = "Identifier of the item to delete"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Item deleted",
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller can read the gallery but may not delete its items"
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
    summary = "Delete a gallery item"
)]
pub(crate) async fn delete_gallery_item(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path((gallery_id_value, item_id_value)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let gallery_id = parse_gallery_id(&gallery_id_value)?;
    let item_id = parse_item_id(&item_id_value)?;
    state
        .library_use_case_factory()
        .delete_gallery_item()
        .execute(DeleteGalleryItemCommand::new(
            caller.user_id(),
            gallery_id,
            item_id,
        ))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
