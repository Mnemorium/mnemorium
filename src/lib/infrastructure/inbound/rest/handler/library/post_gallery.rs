use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::create_gallery::{CreateGalleryCommand, CreateGalleryError};
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::library::GalleryResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload to create a gallery owned by the caller.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostGalleryRequest {
    /// Whether any authenticated user may read and moderate the gallery.
    #[serde(default)]
    pub is_public: bool,
    /// Name of the gallery.
    #[schema(min_length = 1, max_length = 100)]
    pub name: String,
}

/// Map a create-gallery error to its API error.
impl From<CreateGalleryError> for ApiError {
    fn from(err: CreateGalleryError) -> Self {
        match err {
            CreateGalleryError::InvalidName | CreateGalleryError::NameTooLong => {
                Self::UnprocessableEntity(err.to_string())
            }
            CreateGalleryError::NameAlreadyExists => Self::Conflict(err.to_string()),
            CreateGalleryError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CreateGalleryError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Create a gallery owned by the caller.
///
/// Any authenticated caller may create a gallery. The name is required, trimmed
/// and at most 100 characters long; `is_public` defaults to `false`.
#[utoipa::path(
    post,
    operation_id = "post_gallery",
    path = "/gallery",
    tag = "library",
    request_body = PostGalleryRequest,
    responses(
        (
            status = CREATED,
            body = GalleryResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "URI of the newly created gallery"),
            ),
            description = "Gallery created",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body"
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
            description = "The gallery name is empty or too long"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The caller already owns a gallery with this name"
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
    summary = "Create a gallery"
)]
pub(crate) async fn post_gallery(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: Result<Json<PostGalleryRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(request) = payload.map_err(ApiError::from)?;
    let response = state
        .library_use_case_factory()
        .create_gallery()
        .execute(CreateGalleryCommand::new(
            caller.user_id(),
            request.name,
            request.is_public,
        ))
        .await?;
    let body = GalleryResponse::from(response);
    let location = body.links.self_link.href.clone();
    let mut http_response = hal_json(body);
    *http_response.status_mut() = StatusCode::CREATED;
    http_response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}
