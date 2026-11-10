use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::{
    GetVideoCommand, GetVideoError, GetVideoResponse as GetVideoResponseData,
};
use crate::domain::NumericID;
use crate::domain::model::ScanType;
use crate::infrastructure::inbound::rest::handler::asset::{
    asset_file_href, asset_video_href, content_href, parse_asset_id,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, Link, hal_json};

/// HAL links exposed by a typed-video representation.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct AssetVideoLinks {
    /// Link to the video's playable content.
    pub content: Link,
    /// Link to the stored file backing the video.
    pub file: Link,
    /// Link to the video resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

/// Representation of a typed video.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct AssetVideoResponse {
    /// Codec of the video.
    pub codec: String,
    /// Identifier of the colour space.
    pub color_id: String,
    /// Duration of the video, in milliseconds.
    pub duration_ms: f64,
    /// Identifier of the backing file.
    pub file_id: NumericID,
    /// Number of frames.
    pub frame_count: i64,
    /// Height in pixels.
    pub height: i64,
    /// Unique identifier of the video.
    pub id: NumericID,
    /// Links to the video, its backing file and its content.
    #[serde(rename = "_links")]
    pub links: AssetVideoLinks,
    /// Scan type.
    pub scan_type: ScanType,
    /// Width in pixels.
    pub width: i64,
}

impl AssetVideoResponse {
    /// Map a fetched video onto its HTTP representation.
    #[must_use]
    fn from_response(response: &GetVideoResponseData) -> Self {
        Self {
            codec: response.codec().to_owned(),
            color_id: response.color_id().to_owned(),
            duration_ms: response.duration_ms(),
            file_id: response.file_id(),
            frame_count: response.frame_count(),
            height: response.height(),
            id: response.id(),
            links: AssetVideoLinks {
                content: Link::new(&content_href(
                    &format!("/api/v1/asset/video/{}/content", response.id()),
                    response.content_link(),
                )),
                file: Link::new(&asset_file_href(response.file_id())),
                self_link: Link::new(&asset_video_href(response.id())),
            },
            scan_type: response.scan_type(),
            width: response.width(),
        }
    }
}

/// Map a fetch-video error to its API error.
impl From<GetVideoError> for ApiError {
    fn from(err: GetVideoError) -> Self {
        match err {
            GetVideoError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetVideoError::NoSuchVideo => Self::NotFound(err.to_string()),
            GetVideoError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one typed video by its identifier.
#[utoipa::path(
    get,
    operation_id = "get_video",
    path = "/video/{video_id}",
    tag = "asset",
    params(
        ("video_id" = NumericID, Path, description = "Identifier of the video to fetch"),
    ),
    responses(
        (
            status = OK,
            body = AssetVideoResponse,
            content_type = "application/hal+json",
            description = "Video found",
            headers(("Cache-Control" = String, description = "Always `no-store`"))
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid video identifier"
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
            description = "Unknown or unreadable video"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(("bearer_auth" = [])),
    summary = "Fetch a typed video"
)]
pub(crate) async fn get_video(
    Path(video_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let video_id = parse_asset_id(&video_id_value, "video_id", "invalid video identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_video()
        .execute(GetVideoCommand::new(caller.user_id(), video_id))
        .await?;
    let mut representation = hal_json(AssetVideoResponse::from_response(&response));
    representation
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(representation)
}
