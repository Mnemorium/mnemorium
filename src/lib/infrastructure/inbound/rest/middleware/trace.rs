use axum::{body::Body, http::Request, middleware::Next, response::Response};
use tracing::debug;

pub async fn tracing(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    // Log the path only (`OBS-003`): the query string is client-controlled and
    // may carry personal data or a token.
    let path = req.uri().path().to_owned();

    debug!(method = %method, path = %path, "request");

    let response = next.run(req).await;

    debug!(status = %response.status(), path = %path, "response");

    response
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::http::StatusCode;
    use axum::middleware;
    use axum::routing::get;
    use tower::ServiceExt as _;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use super::tracing as trace_middleware;

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
    ///
    /// The guard must stay alive for the whole probe: `#[tokio::test]` runs on a
    /// current-thread runtime, so the awaited request is polled on this thread
    /// and sees the scoped default dispatcher.
    #[expect(
        clippy::single_call_fn,
        reason = "the module-local capture helper mirrors the pattern the other middleware suites use"
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
        reason = "the module-local read-back helper mirrors the pattern the other middleware suites use"
    )]
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

    /// Router running the trace middleware over a trivial success route.
    #[expect(
        clippy::single_call_fn,
        reason = "the probe router exists only for the one middleware test"
    )]
    fn router() -> Router {
        Router::new()
            .route("/probe", get(|| async { "ok" }))
            .layer(middleware::from_fn(trace_middleware))
    }

    #[tokio::test]
    async fn tracing_logs_the_path_and_never_the_query_string() -> Result<(), Box<dyn Error>> {
        // Arrange
        let (buffer, _capture) = capture_logs();
        let request = Request::builder()
            .method("GET")
            .uri("/probe?token=LEAK_SENTINEL")
            .body(Body::empty())?;

        // Act
        let response = router().oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        let logs = captured_logs(&buffer);
        assert!(
            logs.contains("/probe"),
            "the request path must reach the sink: {logs}"
        );
        assert!(
            !logs.contains("LEAK_SENTINEL"),
            "the query string must never reach the sink: {logs}"
        );
        Ok(())
    }
}
