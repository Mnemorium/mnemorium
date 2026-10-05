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
