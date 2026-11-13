use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::application::port::{DeleteGalleryCommand, DeleteGalleryError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::parse_gallery_id;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Map a delete-gallery error to its API error.
impl From<DeleteGalleryError> for ApiError {
    fn from(err: DeleteGalleryError) -> Self {
        match err {
            DeleteGalleryError::DefaultGallery => Self::Conflict(err.to_string()),
            DeleteGalleryError::Forbidden => Self::Forbidden(err.to_string()),
            DeleteGalleryError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            DeleteGalleryError::NoSuchGallery => Self::NotFound(err.to_string()),
            DeleteGalleryError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Delete a gallery and its items by its identifier.
///
/// Deleting a gallery requires the `Delete` action on it; the resource's owner
/// and the Root Admin hold it. A caller who can read the gallery but may not
/// delete it answers `403 Forbidden`; one that cannot read it answers `404 Not
/// Found`. The seeded default gallery (identifier `0`) is never deletable and
/// answers `409`. Deletion removes the gallery, its items and their media rows,
/// and the backing file row only where the caller may view the file; the stored
/// bytes are never touched.
#[utoipa::path(
    delete,
    operation_id = "delete_gallery",
    path = "/gallery/{id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery to delete"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Gallery deleted",
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
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller can read the gallery but may not delete it"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown or inaccessible gallery"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The default gallery cannot be deleted"
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
    summary = "Delete a gallery"
)]
pub(crate) async fn delete_gallery(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let gallery_id = parse_gallery_id(&id)?;
    state
        .library_use_case_factory()
        .delete_gallery()
        .execute(DeleteGalleryCommand::new(caller.user_id(), gallery_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
