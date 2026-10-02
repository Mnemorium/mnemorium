//! Middleware wrapping every error response in the HAL error envelope.

use std::str::from_utf8;

use axum::body::to_bytes;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::http::header;
use axum::http::response::Parts;
use axum::middleware::Next;
use axum::response::Response;
use serde_json::Value;

use crate::infrastructure::inbound::rest::api_error::ApiErrorMessage;
use crate::infrastructure::inbound::rest::api_error::error_response;

/// Rewrap every `4xx`/`5xx` response as the HAL error envelope (`API-039`).
///
/// The envelope carries the extracted message at the root and a `_links.self`
/// pointing at the request URI, including any query string. The original status
/// is preserved; every original header except `Content-Type` and
/// `Content-Length` is kept, so `Allow`, `Location` and `WWW-Authenticate`
/// survive the rewrite.
pub async fn hal_errors(request: Request, next: Next) -> Response {
    let path = request.uri().to_string();
    let response = next.run(request).await;
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }

    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, usize::MAX).await.unwrap_or_default();
    let mut wrapped = error_response(status, error_message(&parts, status, &bytes), &path);

    let mut headers = parts.headers;
    headers.remove(header::CONTENT_TYPE);
    headers.remove(header::CONTENT_LENGTH);
    wrapped.headers_mut().extend(headers);

    wrapped
}

/// Extract the human-readable message of a failed response.
///
/// The message attached by [`ApiError`](crate::infrastructure::inbound::rest::api_error::ApiError)
/// takes precedence; otherwise a JSON body with a string `error` field is
/// reused as-is, then a non-empty UTF-8 body is used trimmed, and finally the
/// status reason phrase is synthesized, because the default `404`/`405` bodies
/// are empty or textual.
#[expect(
    clippy::single_call_fn,
    reason = "the message extraction order is named for readability"
)]
fn error_message(parts: &Parts, status: StatusCode, body: &[u8]) -> String {
    if let Some(message) = parts.extensions.get::<ApiErrorMessage>() {
        return message.0.clone();
    }
    if let Ok(value) = serde_json::from_slice::<Value>(body)
        && let Some(message) = value.get("error").and_then(Value::as_str)
    {
        return message.to_owned();
    }
    if let Ok(text) = from_utf8(body) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    status.canonical_reason().unwrap_or("error").to_owned()
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use axum::Json;
    use axum::Router;
    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::middleware;
    use axum::response::Response;
    use axum::routing::get;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::hal_errors;
    use crate::infrastructure::inbound::rest::api_error::ApiError;

    /// Router exercising the middleware over an `ApiError`, a raw JSON error, a
    /// textual error, a healthy route and a method restriction.
    fn router() -> Router {
        Router::new()
            .route(
                "/broken",
                get(|| async { ApiError::BadRequest("bad request".to_owned()) }),
            )
            .route(
                "/raw-json",
                get(|| async {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": "raw json" })),
                    )
                }),
            )
            .route(
                "/plain",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "plain failure") }),
            )
            .route("/only", get(|| async { "ok" }))
            .layer(middleware::from_fn(hal_errors))
    }

    /// Send a body-less request through `router`.
    async fn send(router: Router, method: &str, uri: &str) -> Result<Response, Box<dyn Error>> {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())?;
        Ok(router.oneshot(request).await?)
    }

    /// Split `response` into its status and decoded JSON body.
    async fn into_parts(response: Response) -> Result<(StatusCode, Value), Box<dyn Error>> {
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        Ok((status, serde_json::from_slice(&bytes)?))
    }

    /// Return the media type declared by `response`.
    fn content_type(response: &Response) -> Option<&str> {
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
    }

    #[tokio::test]
    async fn hal_errors_wraps_an_api_error() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/broken").await?;

        // Assert
        assert_eq!(content_type(&response), Some("application/hal+json"));
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({
                "error": "bad request",
                "_links": { "self": { "href": "/broken" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_reuses_a_handler_json_error() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/raw-json").await?;

        // Assert
        assert_eq!(content_type(&response), Some("application/hal+json"));
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({
                "error": "raw json",
                "_links": { "self": { "href": "/raw-json" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_keeps_the_query_string_in_the_self_link() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/broken?page=2&size=10").await?;

        // Assert
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({
                "error": "bad request",
                "_links": { "self": { "href": "/broken?page=2&size=10" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_reuses_a_plain_text_error() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/plain").await?;

        // Assert
        assert_eq!(content_type(&response), Some("application/hal+json"));
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            payload,
            json!({
                "error": "plain failure",
                "_links": { "self": { "href": "/plain" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_synthesizes_an_empty_error_from_the_status_reason()
    -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/missing").await?;

        // Assert
        assert_eq!(content_type(&response), Some("application/hal+json"));
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            payload,
            json!({
                "error": "Not Found",
                "_links": { "self": { "href": "/missing" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_wraps_a_method_not_allowed_and_keeps_allow() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "POST", "/only").await?;

        // Assert
        assert_eq!(content_type(&response), Some("application/hal+json"));
        assert!(
            response.headers().get(header::ALLOW).is_some(),
            "the Allow header must survive the rewrite"
        );
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            payload,
            json!({
                "error": "Method Not Allowed",
                "_links": { "self": { "href": "/only" } },
            })
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_leaves_a_success_response_unchanged() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/only").await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(content_type(&response), Some("text/plain; charset=utf-8"));
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        assert_eq!(bytes.as_ref(), b"ok");
        Ok(())
    }
}
