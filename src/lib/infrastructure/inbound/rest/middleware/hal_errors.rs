//! Middleware wrapping every error response in the HAL error envelope.

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
/// pointing at the request path, never the query string (`API-043`). The
/// original status is preserved; every original header except `Content-Type`
/// and `Content-Length` is kept, so `Allow`, `Location` and `WWW-Authenticate`
/// survive the rewrite.
pub async fn hal_errors(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
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
/// takes precedence; otherwise a JSON body the application authored — served
/// as `application/json` or `application/hal+json` and carrying `error` as its
/// only top-level key — is reused, and finally the status reason phrase is
/// synthesized, because the default `404`/`405` bodies are empty or textual.
///
/// A body that is neither is never reused verbatim: an unverified plain-text
/// or JSON body may carry extractor or framework wording, an expected type
/// name, or a rejected value, which `API-040` forbids. Such a body falls
/// through to the status reason phrase (`API-040`, § 2.5).
#[expect(
    clippy::single_call_fn,
    reason = "the message extraction order is named for readability"
)]
fn error_message(parts: &Parts, status: StatusCode, body: &[u8]) -> String {
    if let Some(message) = parts.extensions.get::<ApiErrorMessage>() {
        return message.0.clone();
    }
    let authored_json = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|media_type| {
            matches!(
                media_type.split(';').next().map(str::trim),
                Some("application/json" | "application/hal+json")
            )
        });
    if authored_json
        && let Ok(object) = serde_json::from_slice::<serde_json::Map<String, Value>>(body)
        && object.len() == 1
        && let Some(message) = object.get("error").and_then(Value::as_str)
    {
        return message.to_owned();
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
    use axum::extract::Path;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::middleware;
    use axum::response::Response;
    use axum::routing::get;
    use rstest::rstest;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::hal_errors;
    use crate::infrastructure::inbound::rest::api_error::ApiError;

    /// Router exercising the middleware over an `ApiError`, a raw JSON error, a
    /// textual error, a framework path rejection, a healthy route and a method
    /// restriction.
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
                "/multi-key",
                get(|| async {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": "raw json", "detail": "extra" })),
                    )
                }),
            )
            .route(
                "/plain-json",
                get(|| async { (StatusCode::BAD_REQUEST, r#"{"error": "sneaky"}"#) }),
            )
            .route(
                "/plain",
                get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "plain failure") }),
            )
            .route(
                "/item/{id}",
                get(|Path(id): Path<i64>| async move { id.to_string() }),
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
    async fn hal_errors_synthesizes_when_a_json_error_carries_extra_keys()
    -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/multi-key").await?;

        // Assert
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({
                "error": "Bad Request",
                "_links": { "self": { "href": "/multi-key" } },
            }),
            "an aggregate JSON body the application did not author must not be reused (`API-040`)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_synthesizes_when_a_json_shaped_body_is_plain_text()
    -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/plain-json").await?;

        // Assert
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({
                "error": "Bad Request",
                "_links": { "self": { "href": "/plain-json" } },
            }),
            "a JSON-shaped body served as text/plain must not be reused (`API-040`)"
        );
        assert!(
            !payload.to_string().contains("sneaky"),
            "an unverified plain-text body must never appear in the envelope (`API-040`)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_drops_the_query_string_from_the_self_link() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/broken?page=2&size=10").await?;

        // Assert
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
    async fn hal_errors_never_echoes_a_query_value() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/broken?token=SECRET&page=2").await?;

        // Assert
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload.pointer("/_links/self/href").and_then(Value::as_str),
            Some("/broken"),
            "the self link must carry the request path only (`API-039`, `API-043`)"
        );
        assert!(
            !payload.to_string().contains("SECRET"),
            "a query value must not appear anywhere in the error envelope"
        );
        Ok(())
    }

    #[rstest]
    #[case::empty_query("/broken?", StatusCode::BAD_REQUEST, "/broken")]
    #[case::repeated_key("/broken?a=1&a=2", StatusCode::BAD_REQUEST, "/broken")]
    #[case::encoded_path("/broken%20x?q=1", StatusCode::NOT_FOUND, "/broken%20x")]
    #[tokio::test]
    async fn hal_errors_keeps_the_path_and_drops_the_query(
        #[case] uri: &str,
        #[case] expected_status: StatusCode,
        #[case] expected_href: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", uri).await?;

        // Assert
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, expected_status);
        assert_eq!(
            payload.pointer("/_links/self/href").and_then(Value::as_str),
            Some(expected_href)
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_does_not_reuse_a_plain_text_error() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/plain").await?;

        // Assert
        assert_eq!(content_type(&response), Some("application/hal+json"));
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            payload,
            json!({
                "error": "Internal Server Error",
                "_links": { "self": { "href": "/plain" } },
            })
        );
        assert!(
            !payload.to_string().contains("plain failure"),
            "an unverified plain-text body must never appear in the envelope (`API-040`)"
        );
        Ok(())
    }

    #[tokio::test]
    async fn hal_errors_does_not_reuse_a_framework_path_rejection() -> Result<(), Box<dyn Error>> {
        // Act
        let response = send(router(), "GET", "/item/abc").await?;

        // Assert
        assert_eq!(content_type(&response), Some("application/hal+json"));
        let (status, payload) = into_parts(response).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            payload,
            json!({
                "error": "Bad Request",
                "_links": { "self": { "href": "/item/abc" } },
            }),
            "a framework path rejection must be reduced to the synthesized status reason, \
             never the raw value or the expected type name (`API-040`)"
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
