/// The asset context endpoints.
/// The cancel-upload endpoint.
pub(crate) mod delete_upload;
/// The get-file endpoint.
pub(crate) mod get_file;
/// The asset context endpoints.
pub(crate) mod get_file_content;
/// The asset context endpoints.
pub(crate) mod get_image;
/// The asset context endpoints.
pub(crate) mod get_image_content;
/// The asset context endpoints.
pub(crate) mod get_image_file;
/// The asset context endpoints.
pub(crate) mod get_upload;
/// The asset context endpoints.
pub(crate) mod post_upload;
/// The asset context endpoints.
pub(crate) mod post_upload_complete;
/// The asset context endpoints.
pub(crate) mod put_upload_chunk;

use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse as _, Response};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouterExt as _};
use utoipa_axum::routes;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Date and time format of the asset representations.
const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

/// Date format of the asset representations.
const DATE_FORMAT: &str = "%Y-%m-%d";

/// HAL links exposed by an upload-session representation.
///
/// The `self` link addresses the session itself; the templated `chunk` link
/// lets clients discover where to store a chunk instead of building the URL
/// from the identifiers; the `complete` link lets clients discover how to
/// finish the session.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct UploadSessionLinks {
    /// URI template for storing one chunk of the upload session.
    pub chunk: Link,
    /// Link completing the upload session.
    pub complete: Link,
    /// Link to the upload-session resource itself.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl UploadSessionLinks {
    /// Build the links of the upload session identified by `upload_id`.
    #[must_use]
    pub(crate) fn for_upload(upload_id: NumericID) -> Self {
        Self {
            chunk: Link::templated(&format!(
                "/api/v1/asset/upload/{upload_id}/chunk/{{chunk_number}}"
            )),
            complete: Link::new(&format!("/api/v1/asset/upload/{upload_id}/complete")),
            self_link: Link::new(&upload_self_href(upload_id)),
        }
    }
}

/// State of an upload session returned by a successful lookup or chunk store.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct UploadSessionResponse {
    /// One character per chunk, `1` when received and `0` otherwise.
    #[schema(example = json!("110"))]
    pub bitmap: String,
    /// Date and time at which the upload session expires, as
    /// `YYYY-MM-DDTHH:MM:SS`.
    #[schema(example = json!("2026-01-01T12:00:00"))]
    pub expires_at: String,
    /// Unique identifier of the caller's file for the session digest, when one
    /// exists.
    pub file_id: Option<NumericID>,
    /// Whether the upload session has been finished.
    pub is_finished: bool,
    /// Links to the upload session, its chunk endpoint and its completion.
    #[serde(rename = "_links")]
    pub links: UploadSessionLinks,
    /// Total number of chunks the upload is split into.
    pub total_chunks: usize,
}

impl UploadSessionResponse {
    /// Map the state of the upload session identified by `upload_id` onto its
    /// HTTP representation.
    #[must_use]
    pub(crate) fn new(
        upload_id: NumericID,
        bitmap: String,
        total_chunks: usize,
        expires_at: NaiveDateTime,
        is_finished: bool,
        file_id: Option<NumericID>,
    ) -> Self {
        Self {
            bitmap,
            expires_at: expires_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
            file_id,
            is_finished,
            links: UploadSessionLinks::for_upload(upload_id),
            total_chunks,
        }
    }
}

/// Canonical URI reference of the upload session identified by `upload_id`.
///
/// Shared by the upload-session representations and the completion response so
/// the `_links.self` target cannot drift (`API-032`).
#[must_use]
pub(crate) fn upload_self_href(upload_id: NumericID) -> String {
    format!("/api/v1/asset/upload/{upload_id}")
}

/// Canonical URI reference of the stored file identified by `file_id`.
#[must_use]
pub(crate) fn asset_file_href(file_id: NumericID) -> String {
    format!("/api/v1/asset/file/{file_id}")
}

/// Canonical URI reference of the typed image identified by `image_id`.
#[must_use]
pub(crate) fn asset_image_href(image_id: NumericID) -> String {
    format!("/api/v1/asset/image/{image_id}")
}

/// Canonical URI reference of the typed audio identified by `audio_id`.
#[must_use]
pub(crate) fn asset_audio_href(audio_id: NumericID) -> String {
    format!("/api/v1/asset/audio/{audio_id}")
}

/// Render `value` with the representation's date format.
fn format_date(value: NaiveDate) -> String {
    value.format(DATE_FORMAT).to_string()
}

/// Parse an asset identifier from a path segment.
///
/// The malformed value and the parser detail never reach the log (`OBS-003`);
/// only the classification and its declared fields do (`OBS-006`).
pub(crate) fn parse_asset_id(
    value: &str,
    field: &'static str,
    message: &'static str,
) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_error| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = field,
            reason = "invalid_identifier",
            "rejected a malformed asset identifier"
        );
        ApiError::BadRequest(message.to_owned())
    })
}

/// Build the response carrying an authorized asset's bytes.
///
/// The body is served whole with the stored media type; a later change can add
/// bounded streaming (`design.md`, "Content is a sub-resource").
pub(crate) fn asset_content_response(mime_type_id: &str, bytes: Vec<u8>) -> Response {
    let content_type = HeaderValue::from_str(mime_type_id)
        .unwrap_or_else(|_error| HeaderValue::from_static("application/octet-stream"));
    let mut response = StatusCode::OK.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    *response.body_mut() = Body::from(bytes);
    response
}

/// Register the routes of the Asset bounded context.
///
/// The handlers carry their own paths, so this only groups them. The
/// state-dependent chunk body limit is passed in because the `OpenAPI` document
/// does not need it (`None`).
pub(crate) fn asset_router(chunk_body_limit: Option<usize>) -> OpenApiRouter<AppState> {
    let chunk = chunk_body_limit.map_or_else(
        || OpenApiRouter::new().routes(routes!(put_upload_chunk::put_upload_chunk)),
        |limit| {
            OpenApiRouter::new().routes(
                routes!(put_upload_chunk::put_upload_chunk).layer(DefaultBodyLimit::max(limit)),
            )
        },
    );

    OpenApiRouter::new()
        .routes(routes!(post_upload::post_upload))
        .routes(routes!(post_upload_complete::post_upload_complete))
        .routes(routes!(delete_upload::delete_upload))
        .routes(routes!(get_upload::get_upload))
        .merge(chunk)
        .routes(routes!(get_file::get_file))
        .routes(routes!(get_file_content::get_file_content))
        .routes(routes!(get_image::get_image))
        .routes(routes!(get_image_file::get_image_file))
        .routes(routes!(get_image_content::get_image_content))
}

/// Routes of the asset bounded context.
///
/// Every route requires an authenticated caller.
pub(crate) fn asset_routes(state: &AppState) -> OpenApiRouter<()> {
    // The chunk endpoint receives a full chunk (up to `chunk_size_bytes`), which
    // exceeds axum's 2 MiB default request body limit, so that route's limit is
    // raised to the configured chunk size; every other route keeps the default.
    // The limit is fixed when the router is built, so
    // `asset.upload.chunk_size_bytes` is a startup-only setting
    // (`STY-RUST-076`): a change applies after a restart.
    let chunk_body_limit =
        usize::try_from(state.configuration().asset().upload().chunk_size_bytes())
            .unwrap_or(usize::MAX);

    asset_router(Some(chunk_body_limit))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}
