use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::{
    GetImageCommand, GetImageError, GetImageResponse as GetImageResponseData,
};
use crate::domain::NumericID;
use crate::domain::model::Orientation;
use crate::infrastructure::inbound::rest::handler::asset::{
    asset_file_href, asset_image_href, content_href, format_date, parse_asset_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, Link, hal_json};

/// HAL links exposed by a typed-image representation.
///
/// The `self` link addresses the image and `file` names the stored file it
/// backs.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct ImageLinks {
    /// Link to the image's playable content.
    pub content: Link,
    /// Link to the stored file backing the image.
    pub file: Link,
    /// Link to the image resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

/// Representation of a typed image.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct ImageResponse {
    /// Date at which the image was created, as `YYYY-MM-DD`.
    #[schema(example = json!("2026-01-01"))]
    pub created_at: String,
    /// Identifier of the stored file backing the image.
    pub file_id: NumericID,
    /// Height of the image, in pixels.
    pub height_px: i64,
    /// Unique identifier of the image.
    pub id: NumericID,
    /// Links to the image and its backing file.
    #[serde(rename = "_links")]
    pub links: ImageLinks,
    /// Name of the image.
    #[schema(example = json!("cover.png"))]
    pub name: String,
    /// Orientation of the image.
    pub orientation: Orientation,
    /// Width of the image, in pixels.
    pub width_px: i64,
}

impl ImageResponse {
    /// Map a fetched image onto its HTTP representation.
    #[must_use]
    pub(crate) fn from_response(response: &GetImageResponseData) -> Self {
        Self {
            created_at: format_date(response.created_at()),
            file_id: response.file_id(),
            height_px: response.height_px(),
            id: response.id(),
            links: ImageLinks {
                content: Link::new(&content_href(
                    &format!("/api/v1/asset/image/{}/content", response.id()),
                    response.content_link(),
                )),
                file: Link::new(&asset_file_href(response.file_id())),
                self_link: Link::new(&asset_image_href(response.id())),
            },
            name: response.name().to_owned(),
            orientation: response.orientation(),
            width_px: response.width_px(),
        }
    }
}

/// Map a fetch-image error to its API error.
impl From<GetImageError> for ApiError {
    fn from(err: GetImageError) -> Self {
        match err {
            GetImageError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetImageError::NoSuchImage => Self::NotFound(err.to_string()),
            GetImageError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one typed image by its identifier.
///
/// The image follows the read rule of the resource that owns its file. An
/// unknown image and an unreadable image answer the same `404 Not Found`.
#[utoipa::path(
    get,
    operation_id = "get_image",
    path = "/image/{image_id}",
    tag = "asset",
    params(
        ("image_id" = NumericID, Path, description = "Identifier of the image to fetch"),
    ),
    responses(
        (
            status = OK,
            body = ImageResponse,
            content_type = "application/hal+json",
            description = "Image found",
            headers(("Cache-Control" = String, description = "Always `no-store`"))
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
    summary = "Fetch a typed image"
)]
pub(crate) async fn get_image(
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
    let mut representation = hal_json(ImageResponse::from_response(&response));
    representation
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(representation)
}
