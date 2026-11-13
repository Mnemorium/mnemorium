/// The cancel-upload endpoint.
mod delete_upload;
/// The get-audio endpoint.
mod get_audio;
/// The get-audio content endpoint.
mod get_audio_content;
/// The get-audio file alias endpoint.
mod get_audio_file;
/// The get-file endpoint.
mod get_file;
/// The get-file content endpoint.
mod get_file_content;
/// The get-image endpoint.
mod get_image;
/// The get-image content endpoint.
mod get_image_content;
/// The get-image file alias endpoint.
mod get_image_file;
/// The get-upload endpoint.
mod get_upload;
/// The get-video endpoint.
mod get_video;
/// The get-video content endpoint.
mod get_video_content;
/// The get-video file alias endpoint.
mod get_video_file;
/// The begin-upload endpoint.
mod post_upload;
/// The complete-upload endpoint.
mod post_upload_complete;
/// The store-chunk endpoint.
mod put_upload_chunk;

use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse as _, Response};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use tracing::warn;
use utoipa::ToSchema;
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouterExt as _};
use utoipa_axum::routes;

use crate::application::port::{
    AssetTarget, ContentOutcome, GetAssetContentCommand, GetAssetContentError,
    GetAssetContentResponse,
};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::middleware::authenticate;
use crate::infrastructure::inbound::rest::{ApiError, AppState, Link};
use crate::infrastructure::outbound::jwt::JwtTokenProvider;

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

/// Map an asset-content error to its API error.
impl From<GetAssetContentError> for ApiError {
    fn from(err: GetAssetContentError) -> Self {
        match err {
            GetAssetContentError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetAssetContentError::NoSuchAsset => Self::NotFound(err.to_string()),
            GetAssetContentError::Forbidden => Self::Forbidden(err.to_string()),
            GetAssetContentError::LibraryUnavailable => Self::ServiceUnavailable(err.to_string()),
            GetAssetContentError::BytesMissing | GetAssetContentError::Unknown(_) => {
                Self::InternalServerError
            }
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

/// Canonical URI reference of the typed video identified by `video_id`.
#[must_use]
pub(crate) fn asset_video_href(video_id: NumericID) -> String {
    format!("/api/v1/asset/video/{video_id}")
}

/// Compose the content link of the typed asset `kind` addressed by `id`.
#[must_use]
pub(crate) fn content_href(kind: &str, id: NumericID) -> Link {
    Link::new(&format!("/api/v1/asset/{kind}/{id}/content"))
}

/// Render `value` with the representation's date format.
fn format_date(value: NaiveDate) -> String {
    value.format(DATE_FORMAT).to_string()
}

/// Build the content command from the request's caller and headers.
///
/// The authentication layer has already resolved the session caller; this only
/// carries the range and conditional values into the use case, which owns their
/// classification.
pub(crate) fn content_command(
    caller_id: NumericID,
    target: AssetTarget,
    headers: &HeaderMap,
    method: &Method,
) -> GetAssetContentCommand {
    GetAssetContentCommand::new(caller_id, target)
        .with_head(*method == Method::HEAD)
        .with_range(
            headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        )
        .with_if_none_match(
            headers
                .get(header::IF_NONE_MATCH)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        )
        .with_if_range(
            headers
                .get(header::IF_RANGE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
        )
}

/// Build the response carrying an authorized asset's bytes.
///
/// The body streams from the opened reader in bounded chunks; the headers name
/// the range, the validators and the satisfiability of the request. A `HEAD`,
/// `304` or `416` carries no reader, so only headers are written. A save
/// surface's name reaches `Content-Disposition` with RFC 6266 encoding.
pub(crate) fn asset_content_response(data: GetAssetContentResponse) -> Response {
    let outcome = data.outcome();
    let status = match outcome {
        ContentOutcome::Full => StatusCode::OK,
        ContentOutcome::Partial => StatusCode::PARTIAL_CONTENT,
        ContentOutcome::NotModified => StatusCode::NOT_MODIFIED,
        ContentOutcome::Unsatisfiable => StatusCode::RANGE_NOT_SATISFIABLE,
    };
    let mut response = status.into_response();
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("accept-ranges"),
        HeaderValue::from_static("bytes"),
    );
    let content_type = HeaderValue::from_str(data.mime_type_id())
        .unwrap_or_else(|_error| HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, content_type);
    if let Some(etag) = data.etag()
        && let Ok(value) = HeaderValue::from_str(etag)
    {
        headers.insert(HeaderName::from_static("etag"), value);
    }
    let modified_at = data.modified_at().format("%a, %d %b %Y %H:%M:%S GMT");
    if let Ok(value) = HeaderValue::from_str(&modified_at.to_string()) {
        headers.insert(HeaderName::from_static("last-modified"), value);
    }
    match outcome {
        ContentOutcome::Full => {
            headers.insert(
                HeaderName::from_static("content-length"),
                HeaderValue::from_str(&data.size_bytes().to_string())
                    .unwrap_or_else(|_error| HeaderValue::from_static("0")),
            );
        }
        ContentOutcome::Partial => {
            if let Some((start, end)) = data.selected_range() {
                headers.insert(
                    HeaderName::from_static("content-range"),
                    HeaderValue::from_str(&format!("bytes {start}-{end}/{}", data.size_bytes()))
                        .unwrap_or_else(|_error| HeaderValue::from_static("bytes */0")),
                );
                headers.insert(
                    HeaderName::from_static("content-length"),
                    HeaderValue::from_str(&end.saturating_sub(start).saturating_add(1).to_string())
                        .unwrap_or_else(|_error| HeaderValue::from_static("0")),
                );
            }
        }
        ContentOutcome::Unsatisfiable => {
            headers.insert(
                HeaderName::from_static("content-range"),
                HeaderValue::from_str(&format!("bytes */{}", data.size_bytes()))
                    .unwrap_or_else(|_error| HeaderValue::from_static("bytes */0")),
            );
        }
        ContentOutcome::NotModified => {}
    }
    if let Some(reader) = data.into_reader() {
        *response.body_mut() = Body::from_stream(ReaderStream::new(reader));
    }
    response
}

/// Build the `Content-Disposition` value naming a saved file.
///
/// A name that a quoted string can carry is emitted as `filename=`; any other
/// name — one carrying a quote, a separator, a control character or a
/// non-ASCII character — is emitted as an RFC 5987 `filename*=` with a
/// sanitized ASCII `filename=` fallback, so a CR/LF attempt can never start a
/// header line.
pub(crate) fn content_disposition(name: &str) -> HeaderValue {
    if name.is_empty() {
        return HeaderValue::from_static("attachment; filename=\"download\"");
    }
    if is_plain(name) {
        return HeaderValue::from_str(&format!("attachment; filename=\"{name}\""))
            .unwrap_or_else(|_error| HeaderValue::from_static("attachment"));
    }
    let encoded = rfc5987(name);
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{encoded}",
        sanitized(name)
    ))
    .unwrap_or_else(|_error| HeaderValue::from_static("attachment"))
}

/// Return whether `name` can be written inside a quoted `filename=` value.
fn is_plain(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || " ._-".contains(character))
}

/// Reduce a name to an ASCII fallback that can always be quoted.
fn sanitized(name: &str) -> String {
    let fallback: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || " ._-".contains(character) {
                character
            } else {
                '_'
            }
        })
        .collect();
    if fallback.trim_matches(['.', ' ']).is_empty() {
        "download".to_owned()
    } else {
        fallback
    }
}

/// Percent-encode a name for the RFC 5987 `filename*` parameter.
fn rfc5987(name: &str) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
            )
        {
            encoded.push(char::from(byte));
        } else {
            let _ignored = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
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

/// Register the routes of the Asset bounded context.
///
/// The handlers carry their own paths, so this only groups them. The
/// state-dependent chunk body limit is passed in because the `OpenAPI` document
/// does not need it (`None`).
#[cfg(any(feature = "api-docs", test))]
pub(crate) fn asset_router(chunk_body_limit: Option<usize>) -> OpenApiRouter<AppState> {
    authenticated_asset_router(chunk_body_limit).merge(content_asset_router())
}

/// The authenticated routes of the Asset context.
fn authenticated_asset_router(chunk_body_limit: Option<usize>) -> OpenApiRouter<AppState> {
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
        .routes(routes!(get_image::get_image))
        .routes(routes!(get_image_file::get_image_file))
        .routes(routes!(get_audio::get_audio))
        .routes(routes!(get_audio_file::get_audio_file))
        .routes(routes!(get_video::get_video))
        .routes(routes!(get_video_file::get_video_file))
}

/// The content routes, authenticated by the session token.
fn content_asset_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_file_content::get_file_content))
        .routes(routes!(get_image_content::get_image_content))
        .routes(routes!(get_audio_content::get_audio_content))
        .routes(routes!(get_video_content::get_video_content))
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

    authenticated_asset_router(Some(chunk_body_limit))
        .merge(content_asset_router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_name_is_a_quoted_filename() {
        // Act
        let value = content_disposition("holiday.png");

        // Assert
        assert_eq!(value, "attachment; filename=\"holiday.png\"");
    }

    #[test]
    fn a_quote_and_a_separator_are_encoded() {
        // Act
        let value = content_disposition("a\"b/c.png");

        // Assert
        assert_eq!(
            value,
            "attachment; filename=\"a_b_c.png\"; filename*=UTF-8''a%22b%2Fc.png"
        );
    }

    #[test]
    fn a_non_ascii_name_round_trips_through_the_encoded_parameter() {
        // Act
        let value = content_disposition("café.png");

        // Assert
        assert!(
            value
                .to_str()
                .is_ok_and(|text| text.contains("filename*=UTF-8''caf%C3%A9.png"))
        );
    }

    #[test]
    fn a_crlf_attempt_never_reaches_the_header() {
        // Act
        let value = content_disposition("a\r\nSet-Cookie: x.png");

        // Assert
        assert!(!value.to_str().unwrap_or_default().contains('\r'));
        assert!(!value.to_str().unwrap_or_default().contains('\n'));
        assert!(value.to_str().is_ok_and(|text| text.contains("%0D%0A")));
    }

    #[test]
    fn a_name_without_a_usable_fallback_degrades_to_download() {
        // Act
        let value = content_disposition("");

        // Assert
        assert_eq!(value, "attachment; filename=\"download\"");
    }

    #[test]
    fn the_content_command_carries_the_caller_and_request_values() {
        // Arrange
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=0-9"));
        headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static("etag"));
        headers.insert(header::IF_RANGE, HeaderValue::from_static("if-range"));

        // Act
        let command = content_command(9, AssetTarget::File(100), &headers, &Method::HEAD);

        // Assert
        assert_eq!(command.caller_id(), 9);
        assert!(command.head());
        assert_eq!(command.range(), Some("bytes=0-9"));
        assert_eq!(command.if_none_match(), Some("etag"));
        assert_eq!(command.if_range(), Some("if-range"));
    }
}
