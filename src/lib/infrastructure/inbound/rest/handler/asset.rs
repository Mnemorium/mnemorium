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

use std::future::Future;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, FromRef as _, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse as _, Response};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use tracing::{error, warn};
use utoipa::ToSchema;
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouterExt as _};
use utoipa_axum::routes;

use crate::application::port::{
    AssetTarget, ContentOutcome, GetAssetContentCommand, GetAssetContentResponse,
};
use crate::domain::NumericID;
use crate::domain::port::{
    ContentLink, ContentLinkProvider as _, TokenProvider as _, TokenProviderError,
};
use crate::infrastructure::inbound::rest::middleware::authenticate;
use crate::infrastructure::inbound::rest::{ApiError, AppState, Link};
use crate::infrastructure::outbound::jwt::{JwtContentLinkProvider, JwtTokenProvider};

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

/// The credential a content request presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentAccess {
    /// A file-scoped playback credential naming `file_id`.
    Link(NumericID),
    /// A session bearer token naming `user_id`.
    Session(NumericID),
}

impl ContentAccess {
    /// Return the file a playback credential names, if this is a link.
    #[must_use]
    pub(crate) fn link_file_id(&self) -> Option<NumericID> {
        match *self {
            Self::Link(file_id) => Some(file_id),
            Self::Session(_) => None,
        }
    }

    /// Return the session caller, if this is a bearer request.
    #[must_use]
    pub(crate) fn session_user_id(&self) -> Option<NumericID> {
        match *self {
            Self::Session(user_id) => Some(user_id),
            Self::Link(_) => None,
        }
    }
}

/// Extract the bearer session or the file-scoped playback credential.
///
/// Exactly one credential is accepted: a session bearer token resolves the
/// caller, a link credential names one file, and both together are ambiguous
/// and rejected with `400 Bad Request`. A malformed, invalid or expired
/// credential answers `401 Unauthorized`, and the query string never reaches a
/// log sink (`OBS-003`).
impl FromRequestParts<AppState> for ContentAccess {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let authorization = parts.headers.get(header::AUTHORIZATION).cloned();
        let mut expires_at = None;
        let mut signature = None;
        for pair in parts.uri.query().unwrap_or_default().split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            match key {
                "exp" => expires_at = value.parse::<u64>().ok(),
                "sig" if !value.is_empty() => signature = Some(value.to_owned()),
                _ => {}
            }
        }
        let token_provider = Arc::<JwtTokenProvider>::from_ref(state);
        let content_link = Arc::<JwtContentLinkProvider>::from_ref(state);

        async move {
            if authorization.is_some() && (expires_at.is_some() || signature.is_some()) {
                warn!(
                    target: "security",
                    event = "session_validation_failed",
                    reason = "ambiguous_content_credential",
                    "rejected a request carrying both a bearer token and a playback credential"
                );
                return Err(ApiError::BadRequest(
                    "the request must carry either an authorization header or a playback credential"
                        .to_owned(),
                ));
            }

            if let Some(value) = authorization {
                let Some(token) = value
                    .to_str()
                    .ok()
                    .and_then(|raw| raw.split_once(' '))
                    .and_then(|(scheme, token)| {
                        scheme.eq_ignore_ascii_case("bearer").then_some(token)
                    })
                else {
                    warn!(
                        target: "security",
                        event = "session_validation_failed",
                        reason = "missing_authorization_header",
                        "rejected a request without a bearer token"
                    );
                    return Err(ApiError::Unauthorized(
                        "missing or malformed authorization header".to_owned(),
                    ));
                };
                let user_id = token_provider.validate(token).await.map_err(|error| {
                    if matches!(
                        error,
                        TokenProviderError::InvalidClaims
                            | TokenProviderError::InvalidToken
                            | TokenProviderError::TokenExpired
                    ) {
                        warn!(
                            target: "security",
                            event = "session_validation_failed",
                            reason = "invalid_or_expired_token",
                            "rejected an invalid or expired bearer token"
                        );
                        ApiError::Unauthorized("invalid or expired token".to_owned())
                    } else {
                        error!(
                            target: "security",
                            event = "token_provider_fault",
                            kind = "validation_failed",
                            "the token provider failed to validate a token"
                        );
                        ApiError::InternalServerError
                    }
                })?;
                return Ok(Self::Session(user_id));
            }

            let (Some(exp), Some(sig)) = (expires_at, signature) else {
                warn!(
                    target: "security",
                    event = "session_validation_failed",
                    reason = "missing_authorization_header",
                    "rejected a request without a bearer token or playback credential"
                );
                return Err(ApiError::Unauthorized(
                    "missing or malformed authorization header".to_owned(),
                ));
            };
            let file_id = content_link.verify(exp, &sig).await.map_err(|_error| {
                warn!(
                    target: "security",
                    event = "session_validation_failed",
                    reason = "invalid_content_credential",
                    "rejected an invalid or expired playback credential"
                );
                ApiError::Unauthorized("invalid or expired playback credential".to_owned())
            })?;
            Ok(Self::Link(file_id))
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

/// Compose a content link carrying a playback credential.
#[must_use]
pub(crate) fn content_href(base: &str, link: &ContentLink) -> String {
    format!("{base}?exp={}&sig={}", link.expires_at(), link.signature())
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

/// Build the content command from the request's credential and headers.
///
/// The extractor has already validated the session token or the playback
/// credential; this only carries the range and conditional values into the use
/// case, which owns their classification.
pub(crate) fn content_command(
    access: ContentAccess,
    target: AssetTarget,
    headers: &HeaderMap,
    method: &Method,
) -> GetAssetContentCommand {
    let command = match (access.session_user_id(), access.link_file_id()) {
        (Some(caller_id), _) => GetAssetContentCommand::new(caller_id, target),
        (None, Some(file_id)) => GetAssetContentCommand::credentialed(target, file_id),
        (None, None) => GetAssetContentCommand::credentialed(target, 0),
    };
    command
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
/// `304` or `416` carries no reader, so only headers are written.
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

/// The content routes, authorized by a session token or a playback credential.
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
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .merge(content_asset_router())
        .with_state(state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::inbound::rest::hal_json;

    use crate::test_helpers::{TEST_JWT_SECRET, app_state};
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use std::error::Error;
    use tower::ServiceExt as _;
    use tracing_test::traced_test;

    /// Echo the credential the extractor recovered.
    async fn stub(access: ContentAccess) -> Response {
        hal_json(serde_json::json!({
            "link": access.link_file_id(),
            "session": access.session_user_id(),
        }))
    }

    /// A router guarded by the content extractor.
    fn router() -> Result<Router, Box<dyn Error>> {
        Ok(Router::new()
            .route("/content", get(stub))
            .with_state(app_state()?))
    }

    /// Mint a session bearer token for `user_id`.
    async fn bearer(user_id: NumericID) -> Result<String, Box<dyn Error>> {
        let provider = JwtTokenProvider::new(TEST_JWT_SECRET.to_owned(), 3600);
        Ok(provider.issue(user_id).await?.value().to_owned())
    }

    /// Mint a playback credential for `file_id`.
    async fn credential(file_id: NumericID) -> Result<(u64, String), Box<dyn Error>> {
        let provider = JwtContentLinkProvider::new(TEST_JWT_SECRET.to_owned());
        let link = provider.mint(file_id).await?;
        Ok((link.expires_at(), link.signature().to_owned()))
    }

    #[tokio::test]
    async fn a_bearer_header_resolves_the_session() -> Result<(), Box<dyn Error>> {
        // Arrange
        let token = bearer(42).await?;
        let request = Request::builder()
            .uri("/content")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())?;

        // Act
        let response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn a_playback_credential_resolves_the_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (exp, sig) = credential(7).await?;
        let request = Request::builder()
            .uri(format!("/content?exp={exp}&sig={sig}"))
            .body(Body::empty())?;

        // Act
        let response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn both_credentials_are_ambiguous() -> Result<(), Box<dyn Error>> {
        // Arrange
        let token = bearer(42).await?;
        let (exp, sig) = credential(7).await?;
        let request = Request::builder()
            .uri(format!("/content?exp={exp}&sig={sig}"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())?;

        // Act
        let response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }

    #[tokio::test]
    async fn neither_credential_is_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder().uri("/content").body(Body::empty())?;

        // Act
        let response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn a_tampered_credential_is_unauthorized() -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder()
            .uri("/content?exp=4102444800&sig=tampered")
            .body(Body::empty())?;

        // Act
        let response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn the_query_string_never_reaches_a_log_sink() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (exp, sig) = credential(7).await?;
        let request = Request::builder()
            .uri(format!("/content?exp={exp}&sig={sig}&LEAK_SENTINEL=1"))
            .body(Body::empty())?;

        // Act
        let response = router()?.oneshot(request).await?;

        // Assert: the handler answers, and the credential never reaches a sink.
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !logs_contain(&sig),
            "the credential must never reach a log sink"
        );
        assert!(
            !logs_contain("LEAK_SENTINEL"),
            "the query string must never reach a log sink"
        );
        Ok(())
    }
}
