use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::{GetGalleryCommand, GetGalleryError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::library::{GalleryResponse, parse_gallery_id};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Map a fetch-gallery error to its API error.
impl From<GetGalleryError> for ApiError {
    fn from(err: GetGalleryError) -> Self {
        match err {
            GetGalleryError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetGalleryError::NoSuchGallery => Self::NotFound(err.to_string()),
            GetGalleryError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch a gallery and its items by its identifier.
///
/// An authenticated caller reads a gallery its `gallery.read` permission
/// covers. A gallery the caller cannot read answers `404 Not Found`,
/// indistinguishable from a missing one. The representation carries the
/// gallery and its items, ordered by position.
#[utoipa::path(
    get,
    operation_id = "get_gallery",
    path = "/gallery/{id}",
    tag = "library",
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery to fetch"),
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
    summary = "Fetch a gallery by identifier"
)]
pub(crate) async fn get_gallery(
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
