use axum::http::StatusCode;
use axum::{body::Body, http::Request, middleware::Next, response::Response};
use tracing::debug;
use tracing::warn;

pub async fn tracing(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    // Log the path only (`OBS-003`): the query string is client-controlled and
    // may carry personal data or a token.
    let path = req.uri().path().to_owned();

    debug!(method = %method, path = %path, "request");

    let response = next.run(req).await;

    // A method the route does not support is a reviewable security event
    // (`OBS-006`): the catalog declares `unexpected_http_method` at `warn` with
    // fields `method` and `path`, and the path only, never the query string
    // (`OBS-003`).
    if response.status() == StatusCode::METHOD_NOT_ALLOWED {
        warn!(
            target: "security",
            event = "unexpected_http_method",
            method = %method,
            path = %path,
            "rejected an unsupported HTTP method"
        );
    }

    debug!(status = %response.status(), path = %path, "response");

    response
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use tracing_test::traced_test;

    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::http::StatusCode;
    use axum::middleware;
    use axum::routing::get;
    use tower::ServiceExt as _;

    use super::tracing as trace_middleware;

    /// Router running the trace middleware over a trivial success route.
    fn router() -> Router {
        Router::new()
            .route("/probe", get(|| async { "ok" }))
            .layer(middleware::from_fn(trace_middleware))
    }

    #[tokio::test]
    #[traced_test]
    async fn tracing_logs_the_path_and_never_the_query_string() -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder()
            .method("GET")
            .uri("/probe?token=LEAK_SENTINEL")
            .body(Body::empty())?;

        // Act
        let response = router().oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            logs_contain("/probe"),
            "the request path must reach the sink"
        );
        assert!(
            !logs_contain("LEAK_SENTINEL"),
            "the query string must never reach the sink"
        );
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn tracing_logs_unexpected_http_method_on_method_not_allowed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder()
            .method("POST")
            .uri("/probe?token=LEAK_SENTINEL")
            .body(Body::empty())?;

        // Act
        let response = router().oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        logs_assert(|lines| {
            let count = lines
                .iter()
                .filter(|line| line.contains("event=\"unexpected_http_method\""))
                .count();
            (count == 1)
                .then_some(())
                .ok_or_else(|| format!(
                    "an unsupported method must emit exactly one unexpected_http_method (found {count})"
                ))
        });
        assert!(logs_contain("WARN"), "the event must be emitted at warn");
        assert!(
            logs_contain("method=POST"),
            "the event must carry the method"
        );
        assert!(logs_contain("path=/probe"), "the event must carry the path");
        assert!(
            !logs_contain("LEAK_SENTINEL"),
            "the query string must never reach the sink"
        );
        Ok(())
    }
}
