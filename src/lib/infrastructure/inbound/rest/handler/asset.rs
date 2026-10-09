pub mod get_upload;
pub mod links;
pub mod post_upload;
pub mod post_upload_complete;
pub mod put_upload_chunk;
pub mod upload_session;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::get;
use axum::routing::post;
use axum::routing::put;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::get_upload::get_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::post_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_complete::post_upload_complete;
use crate::infrastructure::inbound::rest::handler::asset::put_upload_chunk::put_upload_chunk;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

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
    let chunk_body_limit = usize::try_from(
        state
            .configuration()
            .load()
            .asset()
            .upload()
            .chunk_size_bytes(),
    )
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
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use tower::ServiceExt as _;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::complete_upload::CompleteUploadResponse;
    use crate::application::port::complete_upload::CompleteUploadUseCase;
    use crate::application::port::complete_upload::MockCompleteUploadUseCase;
    use crate::domain::model::asset::DEFAULT_CHUNK_SIZE_BYTES;
    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
    use crate::test_helpers::TEST_JWT_SECRET;
    use crate::test_helpers::app_state_with_asset;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "only the raw `write` and `flush` are meaningful for an in-memory capture buffer"
    )]
    impl stdio::Write for CaptureWriter {
        fn flush(&mut self) -> stdio::Result<()> {
            Ok(())
        }

        fn write(&mut self, buf: &[u8]) -> stdio::Result<usize> {
            let mut buffer = self.buffer.lock().unwrap_or_else(PoisonError::into_inner);
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    /// Hand every formatted event to a fresh clone of the shared buffer.
    #[expect(
        clippy::missing_trait_methods,
        reason = "the default `make_writer_for` already routes through `make_writer`"
    )]
    impl<'writer> fmt::MakeWriter<'writer> for CaptureWriter {
        type Writer = Self;

        fn make_writer(&'writer self) -> Self::Writer {
            self.clone()
        }
    }

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    #[expect(
        clippy::single_call_fn,
        reason = "the capture harness is named for readability"
    )]
    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(
            fmt::layer().with_ansi(false).with_writer(CaptureWriter {
                buffer: Arc::clone(&buffer),
            }),
        );
        let guard = set_default(subscriber);
        (buffer, guard)
    }

    /// Read the captured bytes back as a lossy UTF-8 string.
    #[expect(
        clippy::single_call_fn,
        reason = "the capture reader is named for readability"
    )]
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

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
    async fn asset_chunk_route_oversized_body_returns_payload_too_large_once()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let (buffer, _capture) = capture_logs();
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
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("event=\"limit_exceeded\"").count(),
            1,
            "an oversized body must emit exactly one limit_exceeded event: {logs}"
        );
        assert!(
            logs.contains("source=\"request_body\""),
            "the event must classify the source as the request body: {logs}"
        );
        assert!(
            logs.contains("reason=\"size_limit_exceeded\""),
            "the event must classify the reason as the size limit: {logs}"
        );
        assert!(
            logs.contains("security"),
            "the event must target the reserved security target: {logs}"
        );
        Ok(())
    }
}
