pub mod get_upload;
pub mod post_upload;
pub mod post_upload_complete;
pub mod put_upload_chunk;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::get;
use axum::routing::post;
use axum::routing::put;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::handler::asset::get_upload::get_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::post_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_complete::post_upload_complete;
use crate::infrastructure::inbound::rest::handler::asset::put_upload_chunk::put_upload_chunk;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// HAL links exposed by an upload-session representation.
///
/// The `self` link addresses the session itself; the templated `chunk` link
/// lets clients discover where to store a chunk instead of building the URL
/// from the identifiers (`API-037`); the `complete` link lets clients discover
/// how to finish the session.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct UploadSessionLinks {
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
    pub fn for_upload(upload_id: NumericID) -> Self {
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
pub struct UploadSessionResponse {
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
    pub fn new(
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

/// Routes of the asset bounded context.
///
/// Every route requires an authenticated caller.
pub fn asset_routes(state: &AppState) -> Router {
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

    let upload: Router<AppState> = Router::new()
        .route("/upload", post(post_upload))
        .route("/upload/{upload_id}/complete", post(post_upload_complete))
        .route("/upload/{upload_id}", get(get_upload))
        .route(
            "/upload/{upload_id}/chunk/{chunk_number}",
            put(put_upload_chunk).layer(DefaultBodyLimit::max(chunk_body_limit)),
        )
        .with_state(state.clone());

    upload
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use tower::ServiceExt as _;

    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::complete_upload::CompleteUploadResponse;
    use crate::application::port::complete_upload::CompleteUploadUseCase;
    use crate::application::port::complete_upload::MockCompleteUploadUseCase;
    use crate::domain::model::configuration::DEFAULT_CHUNK_SIZE_BYTES;
    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
    use crate::test_helpers::TEST_JWT_SECRET;
    use crate::test_helpers::app_state_with_asset;
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
        Ok(asset_routes(&state))
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
