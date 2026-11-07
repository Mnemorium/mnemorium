use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::add_gallery_item::{AddGalleryItemCommand, AddGalleryItemError};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::{
    GalleryItemDetailResponse, GalleryItemType, parse_gallery_id,
};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to add one of the caller's media to a gallery.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostGalleryItemRequest {
    /// Identifier of the backing file of the medium to add.
    ///
    /// The upload-completion response (`POST /asset/upload/{id}/complete`)
    /// exposes this backing file identifier. The created item's response
    /// reports the resolved medium's own identifier separately.
    pub media_id: NumericID,
    /// Kind of the medium to add.
    // The raw wire value is validated by the handler so a value outside the
    // discrete list is classified as `enum_validation_failed` (`OBS-006`)
    // rather than a generic deserialization failure.
    #[serde(rename = "type")]
    #[schema(value_type = GalleryItemType)]
    pub media_type: String,
}

/// Map an add-gallery-item error to its API error.
///
/// The missing and the unowned medium are one client-visible outcome: a medium
/// the caller does not own is indistinguishable from one that does not exist
/// (`API-044`). Both fail the `media_id` body value and answer `422` with one
/// stable, server-authored message (`API-040`, `API-041`); `404` stays for the
/// URI-addressed gallery and `403` for the gallery-permission outcome.
impl From<AddGalleryItemError> for ApiError {
    fn from(err: AddGalleryItemError) -> Self {
        match err {
            AddGalleryItemError::AlreadyAssigned | AddGalleryItemError::ScopeConflict => {
                Self::Conflict(err.to_string())
            }
            AddGalleryItemError::Forbidden => Self::Forbidden(err.to_string()),
            AddGalleryItemError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            AddGalleryItemError::NoSuchGallery => Self::NotFound(err.to_string()),
            AddGalleryItemError::NoSuchMedia | AddGalleryItemError::NotOwnedMedia => {
                Self::UnprocessableEntity(
                    "the medium does not exist or does not belong to the caller".to_owned(),
                )
            }
            AddGalleryItemError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Add one of the caller's media to a gallery.
///
/// Any authenticated caller may add an item to a public gallery; a private
/// gallery accepts items from its owner and the Root Admin only. A caller may
/// only add media it owns, unless it is the Root Admin. The request carries the
/// medium's backing file identifier, as exposed by upload completion; the item
/// response reports the resolved medium identifier separately. A medium already
/// in a gallery is rejected with `409`.
#[utoipa::path(
    post,
    operation_id = "post_gallery_item",
    path = "/gallery/{id}/item",
    tag = "library",
    request_body = PostGalleryItemRequest,
    params(
        ("id" = NumericID, Path, description = "Identifier of the gallery the media is added to"),
    ),
    responses(
        (
            status = CREATED,
            body = GalleryItemDetailResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly added item"),
            ),
            description = "Item added",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid gallery identifier or malformed request body"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema, or the medium does not exist or belongs to another caller"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller may not add items to the gallery"
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
            description = "The medium already belongs to a gallery"
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
    summary = "Add an item to a gallery"
)]
pub(crate) async fn post_gallery_item(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
    payload: Result<Json<PostGalleryItemRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let gallery_id = parse_gallery_id(&id)?;
    let Json(request) = payload.map_err(ApiError::from)?;
    let media_type = GalleryItemType::from_wire(&request.media_type)
        .ok_or_else(|| ApiError::reject_enum("type"))?;
    let media = media_type.media(request.media_id);
    let response = state
        .library_use_case_factory()
        .add_gallery_item()
        .execute(AddGalleryItemCommand::new(
            caller.user_id(),
            gallery_id,
            media,
        ))
        .await?;
    let body = GalleryItemDetailResponse::from(response);
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}
