/// The asset context endpoints.
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
    //
    // TODO(hot-reload): the limit is fixed when the router is built, so a runtime
    // change to `asset.upload.chunk_size_bytes` only takes effect after a
    // restart.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::complete_upload::{
        CompleteUploadResponse, CompleteUploadUseCase, MockCompleteUploadUseCase,
    };
    use crate::domain::model::configuration::DEFAULT_CHUNK_SIZE_BYTES;
    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
    use crate::test_helpers::*;
    use axum::extract::Request;
    use std::error::Error;
    use std::sync::Arc;
    use tower::ServiceExt as _;
    use tracing_test::traced_test;

    /// Build the asset router whose complete-upload use case succeeds.
    fn router() -> Result<axum::Router, Box<dyn Error>> {
        let mut complete_upload = MockCompleteUploadUseCase::new();
        complete_upload
            .expect_execute()
            .times(0..=1)
            .return_once(|_| Box::pin(async { Ok(CompleteUploadResponse::new(11, true)) }));

        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_complete_upload()
            .times(0..=1)
            .return_once(move || Arc::new(complete_upload) as Arc<dyn CompleteUploadUseCase>);

        let state = app_state_with_asset(Arc::new(factory))?;
        Ok(asset_routes(&state).into())
    }

    /// Mint a bearer token the test token provider accepts.
    async fn bearer() -> Result<String, Box<dyn Error>> {
        let provider = JwtTokenProvider::new(TEST_JWT_SECRET.to_owned(), 3600);
        Ok(provider.issue(3).await?.value().to_owned())
    }

    #[tokio::test]
    async fn asset_routes_complete_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder()
            .method("POST")
            .uri("/upload/7/complete")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn asset_routes_method_not_allowed_keeps_the_allow_header() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let request = Request::builder()
            .method("DELETE")
            .uri("/upload/7")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert!(
            response.headers().get(header::ALLOW).is_some(),
            "the 405 response must advertise the allowed method"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn asset_chunk_route_oversized_body_returns_payload_too_large_once()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let oversized = usize::try_from(DEFAULT_CHUNK_SIZE_BYTES)?.saturating_add(1);
        // No `Content-Range` or `Content-Digest`: an oversized body must be
        // rejected by the body limit, never shadowed by a header parse
        // (`OBS-006`, `API-041`).
        let request = Request::builder()
            .method("PUT")
            .uri("/upload/7/chunk/0")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::from(vec![0u8; oversized]))?;

        // Act
        let response: Response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"limit_exceeded\""))
                .count();
            (count == 1).then_some(()).ok_or_else(|| {
                format!(
                    "an oversized body must emit exactly one limit_exceeded event (found {count})"
                )
            })
        });
        assert!(
            logs_contain("source=\"request_body\""),
            "the event must classify the source as the request body"
        );
        assert!(
            logs_contain("reason=\"size_limit_exceeded\""),
            "the event must classify the reason as the size limit"
        );
        assert!(
            logs_contain("security"),
            "the event must target the reserved security target"
        );
        Ok(())
    }
}
