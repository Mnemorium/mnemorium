use axum::{body::Body, http::Request, middleware::Next, response::Response};
use tracing::info;

pub async fn tracing(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();

    info!("--> {} {}", method, uri);

    let response = next.run(req).await;

    info!("<-- {} {}", response.status(), uri);

    response
}
