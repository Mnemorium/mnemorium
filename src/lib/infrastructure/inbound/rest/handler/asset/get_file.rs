use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::get_file::{
    GetFileCommand, GetFileError, GetFileResponse as GetFileResponseData,
};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{Link, hal_json};
use crate::infrastructure::inbound::rest::handler::asset::{
    TIMESTAMP_FORMAT, asset_audio_href, asset_file_href, asset_image_href, asset_video_href,
    content_href, format_date, parse_asset_id,
};
use crate::infrastructure::inbound::rest::handler::user::user_self_href;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// HAL links exposed by a stored-file representation.
///
/// The `self` link addresses the file; `owner` names the account that owns it,
/// and `image` and `audio` name the typed medium the file backs when one exists.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct FileLinks {
    /// Link to the typed audio backed by the file, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<Link>,
    /// Link to the file's playable content.
    pub content: Link,
    /// Link to the typed image backed by the file, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Link>,
    /// Link to the account owning the file.
    pub owner: Link,
    /// Link to the stored file itself.
    #[serde(rename = "self")]
    pub self_link: Link,
    /// Link to the typed video backed by the file, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<Link>,
}

/// Representation of a stored file.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct FileResponse {
    /// Unique identifier of the stored file.
    pub id: NumericID,
    /// Whether the file is marked public.
    pub is_public: bool,
    /// Links to the file and the resources related to it.
    #[serde(rename = "_links")]
    pub links: FileLinks,
    /// Media type of the file.
    #[schema(example = json!("image/png"))]
    pub mime_type: String,
    /// Instant at which the content was last modified, as
    /// `YYYY-MM-DDTHH:MM:SS`.
    #[schema(example = json!("2026-01-01T12:00:00"))]
    pub modified_at: String,
    /// Origin of the file's bytes.
    #[schema(example = json!("UPLOADED"))]
    pub origin: String,
    /// Path of the file relative to the library root.
    #[schema(example = json!("12_cover.png"))]
    pub path: String,
    /// Size of the file content, in bytes.
    pub size_bytes: i64,
    /// Date at which the file was uploaded, as `YYYY-MM-DD`.
    #[schema(example = json!("2026-01-01"))]
    pub uploaded_at: String,
}

impl FileResponse {
    /// Map a fetched file onto its HTTP representation.
    #[must_use]
    pub(crate) fn from_response(response: &GetFileResponseData) -> Self {
        Self {
            id: response.id(),
            is_public: response.is_public(),
            links: FileLinks {
                audio: response
                    .audio_id()
                    .map(|id| Link::new(&asset_audio_href(id))),
                content: Link::new(&content_href(
                    &format!("/api/v1/asset/file/{}/content", response.id()),
                    response.content_link(),
                )),
                image: response
                    .image_id()
                    .map(|id| Link::new(&asset_image_href(id))),
                owner: Link::new(&user_self_href(response.owner_id())),
                self_link: Link::new(&asset_file_href(response.id())),
                video: response
                    .video_id()
                    .map(|id| Link::new(&asset_video_href(id))),
            },
            mime_type: response.mime_type_id().to_owned(),
            modified_at: response.modified_at().format(TIMESTAMP_FORMAT).to_string(),
            origin: response.origin().as_str().to_owned(),
            path: response.path().to_owned(),
            size_bytes: response.size_bytes(),
            uploaded_at: format_date(response.uploaded_at()),
        }
    }
}

/// Map a fetch-file error to its API error.
impl From<GetFileError> for ApiError {
    fn from(err: GetFileError) -> Self {
        match err {
            GetFileError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetFileError::NoSuchFile => Self::NotFound(err.to_string()),
            GetFileError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch one stored file by its identifier.
///
/// Any authenticated caller may read a file it is allowed to read: a catalogue
/// asset follows `library.read`, a gallery asset its gallery and a playlist
/// cover its playlist. An unknown file and an unreadable file answer the same
/// `404 Not Found`.
#[utoipa::path(
    get,
    operation_id = "get_file",
    path = "/file/{file_id}",
    tag = "asset",
    params(
        ("file_id" = NumericID, Path, description = "Identifier of the file to fetch"),
    ),
    responses(
        (
            status = OK,
            body = FileResponse,
            content_type = "application/hal+json",
            description = "File found",
            headers(("Cache-Control" = String, description = "Always `no-store`"))
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid file identifier"
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
            description = "Unknown or unreadable file"
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
    summary = "Fetch a stored file"
)]
pub(crate) async fn get_file(
    Path(file_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let file_id = parse_asset_id(&file_id_value, "file_id", "invalid file identifier")?;
    let response = state
        .asset_use_case_factory()
        .get_file()
        .execute(GetFileCommand::new(caller.user_id(), file_id))
        .await?;
    let mut representation = hal_json(FileResponse::from_response(&response));
    representation
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(representation)
}
