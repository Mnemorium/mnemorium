use axum::Json;
use axum::extract::rejection::BytesRejection;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::response::Response;
use tracing::warn;

use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::hal::SelfLinks;

/// Standard error payload returned by every failed request.
///
/// The envelope is uniform across every error the server emits, including
/// routing fallbacks and extractor rejections (`API-039`). The `error_response`
/// helper builds it, and the root `hal_errors` middleware applies it to every
/// `4xx`/`5xx` response.
#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[non_exhaustive]
pub struct ErrorBody {
    /// Human-readable description of the error.
    #[schema(example = "An error message")]
    pub error: String,
    /// Link to the request that produced the error.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
}

/// Internal carrier for an [`ApiError`] message.
///
/// `ApiError` cannot build the HAL error envelope itself: [`IntoResponse`] has
/// no access to the request, so it cannot fill `_links.self` (`API-039`).
/// It attaches only its message, and the root `hal_errors` middleware builds
/// the one [`ErrorBody`] envelope from that message (`API-039`).
#[derive(Clone, Debug)]
pub(crate) struct ApiErrorMessage(pub String);

/// HTTP error mapped to a status code and the standard error body.
#[derive(Debug)]
#[non_exhaustive]
pub enum ApiError {
    /// The request is invalid (`400`).
    BadRequest(String),
    /// The request conflicts with the current state of the resource (`409`).
    Conflict(String),
    /// The caller is not allowed to perform the request (`403`).
    Forbidden(String),
    /// The requested resource is no longer available (`410`).
    Gone(String),
    /// An unexpected error occurred (`500`).
    InternalServerError,
    /// The requested resource does not exist (`404`).
    NotFound(String),
    /// The request payload is larger than the server allows (`413`).
    PayloadTooLarge(String),
    /// The caller sent too many requests in a given time window (`429`).
    TooManyRequests(String),
    /// Authentication is required or the credentials are invalid (`401`).
    Unauthorized(String),
    /// The request is well-formed but the server cannot process it (`422`).
    UnprocessableEntity(String),
    /// The request payload media type is not supported (`415`).
    UnsupportedMediaType(String),
}

impl ApiError {
    /// Return the HTTP status and the message to send.
    #[must_use]
    fn status_and_message(self) -> (StatusCode, String) {
        match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Conflict(message) => (StatusCode::CONFLICT, message),
            Self::Forbidden(message) => (StatusCode::FORBIDDEN, message),
            Self::Gone(message) => (StatusCode::GONE, message),
            Self::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "an unexpected error occurred".to_owned(),
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::PayloadTooLarge(message) => (StatusCode::PAYLOAD_TOO_LARGE, message),
            Self::TooManyRequests(message) => (StatusCode::TOO_MANY_REQUESTS, message),
            Self::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message),
            Self::UnprocessableEntity(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            Self::UnsupportedMediaType(message) => (StatusCode::UNSUPPORTED_MEDIA_TYPE, message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = self.status_and_message();
        let mut response = status.into_response();
        response.extensions_mut().insert(ApiErrorMessage(message));
        response
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        // The client receives a stable, server-authored message only
        // (`API-040`): the framework rejection text carries deserialization and
        // parser detail and the offending value, so only its classification is
        // logged (`OBS-003`) and it is never echoed (`API-039`, § 2.5).
        let (event, reason, mapped) = match rejection {
            JsonRejection::BytesRejection(bytes_rejection) => {
                return Self::from_bytes_rejection(&bytes_rejection);
            }
            JsonRejection::MissingJsonContentType(_) => (
                "input_validation_failed",
                "missing_content_type",
                Self::UnsupportedMediaType(
                    "the request body must use Content-Type: application/json".to_owned(),
                ),
            ),
            JsonRejection::JsonDataError(_) => (
                "deserialization_failed",
                "schema_mismatch",
                Self::UnprocessableEntity(
                    "the request body does not match the expected schema".to_owned(),
                ),
            ),
            JsonRejection::JsonSyntaxError(_) => (
                "deserialization_failed",
                "syntax",
                Self::BadRequest("the request body is not valid JSON".to_owned()),
            ),
            _unknown => (
                "deserialization_failed",
                "unknown",
                Self::BadRequest("the request body is invalid".to_owned()),
            ),
        };
        warn!(
            target: "security",
            event = event,
            source = "json_body",
            reason = reason,
            "rejected a request body"
        );
        mapped
    }
}

impl ApiError {
    /// Map a failed body buffer to a `413` or a generic `400`.
    #[expect(
        clippy::single_call_fn,
        reason = "the bytes-rejection mapping is named for readability"
    )]
    #[must_use]
    fn from_bytes_rejection(rejection: &BytesRejection) -> Self {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            warn!(
                target: "security",
                event = "limit_exceeded",
                source = "request_body",
                reason = "size_limit_exceeded",
                "rejected an oversized request body"
            );
            return Self::PayloadTooLarge(
                "the request body exceeds the maximum allowed size".to_owned(),
            );
        }
        warn!(
            target: "security",
            event = "input_validation_failed",
            source = "request_body",
            reason = "buffer_failed",
            "failed to buffer the request body"
        );
        Self::BadRequest("the request body could not be read".to_owned())
    }
}

/// Build the one HAL error envelope every error response is served as
/// (`API-039`): the message at the root (`API-033`) plus `_links.self` to the
/// request path (`API-039`, `API-043`), served as `application/hal+json`
/// (`API-031`).
///
/// It is the single envelope builder by design: only the root `hal_errors`
/// middleware calls it, and only after it has read the [`ApiErrorMessage`].
#[expect(
    clippy::single_call_fn,
    reason = "centralising the envelope in one builder is the point, even when only the root middleware calls it"
)]
#[must_use]
pub(crate) fn error_response(status: StatusCode, message: String, path: &str) -> Response {
    let body = ErrorBody {
        error: message,
        links: SelfLinks::new(path),
    };
    let mut response = Json(body).into_response();
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(HAL_CONTENT_TYPE),
    );
    response
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io as stdio;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::PoisonError;

    use axum::Json;
    use axum::Router;
    use axum::body::Body;
    use axum::extract::DefaultBodyLimit;
    use axum::extract::Request;
    use axum::extract::State;
    use axum::extract::rejection::JsonRejection;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use tower::ServiceExt as _;
    use tracing::subscriber::DefaultGuard;
    use tracing::subscriber::set_default;
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt as _;

    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::test_helpers::app_state;
    use crate::test_helpers::error_message_of;

    use super::ApiError;

    /// Append-only sink that lets a test read back the lines [`fmt`] emits.
    #[derive(Clone)]
    struct CaptureWriter {
        /// Buffer shared with the test that asserts on the captured output.
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    /// Body schema the probe route deserializes into.
    ///
    /// The field is never read: it exists only so a wrong-typed body yields a
    /// `JsonDataError` rather than a `JsonSyntaxError`.
    #[derive(serde::Deserialize, serde::Serialize)]
    struct Probe {
        count: u64,
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

    /// Echo the probe count, converting any rejection through the impl.
    async fn probe(
        State(_state): State<AppState>,
        payload: Result<Json<Probe>, JsonRejection>,
    ) -> Result<Json<Probe>, ApiError> {
        let Json(probe) = payload.map_err(ApiError::from)?;
        Ok(Json(probe))
    }

    /// Drive the probe route and return the response.
    ///
    /// A `JsonRejection` cannot be constructed directly, so the tests obtain a
    /// real one by routing a request through the `Json` extractor. A
    /// `content_type` of `None` omits the header, exercising the extractor's
    /// missing-content-type rejection.
    async fn rejection_response(
        body: &str,
        content_type: Option<&str>,
        body_limit: Option<usize>,
    ) -> Result<Response, Box<dyn Error>> {
        let handler = match body_limit {
            Some(limit) => post(probe).layer(DefaultBodyLimit::max(limit)),
            None => post(probe),
        };
        let router = Router::new()
            .route("/probe", handler)
            .with_state(app_state()?);
        let mut builder = Request::builder().method("POST").uri("/probe");
        if let Some(value) = content_type {
            builder = builder.header(header::CONTENT_TYPE, value);
        }
        let request = builder.body(Body::from(body.to_owned()))?;
        let response = router.oneshot(request).await?;
        Ok(response)
    }

    /// Install a capturing subscriber on the current thread and return the
    /// shared buffer plus the guard that keeps it active.
    ///
    /// The guard must stay alive for the whole probe: `#[tokio::test]` runs on a
    /// current-thread runtime, so the awaited request is polled on this thread
    /// and sees the scoped default dispatcher.
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
    fn captured_logs(buffer: &Mutex<Vec<u8>>) -> String {
        let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(bytes.as_slice()).into_owned()
    }

    #[tokio::test]
    async fn json_rejection_invalid_syntax_returns_generic_bad_request()
    -> Result<(), Box<dyn Error>> {
        // Act
        let response = rejection_response("not json", Some("application/json"), None).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the request body is not valid JSON")
        );
        Ok(())
    }

    #[tokio::test]
    async fn json_rejection_wrong_type_returns_unprocessable_entity() -> Result<(), Box<dyn Error>>
    {
        // Act
        let response = rejection_response(
            r#"{"count":"LEAK_SENTINEL"}"#,
            Some("application/json"),
            None,
        )
        .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let message = error_message_of(&response)
            .ok_or_else(|| "a 422 must carry an error message".to_owned())?;
        assert!(
            !message.contains("LEAK_SENTINEL"),
            "message echoed the body"
        );
        assert!(
            !message.contains("invalid type"),
            "message leaked serde detail"
        );
        assert!(!message.contains("line"), "message leaked a parse position");
        assert!(
            !message.contains("column"),
            "message leaked a parse position"
        );
        Ok(())
    }

    #[tokio::test]
    async fn json_rejection_missing_content_type_returns_unsupported_media_type()
    -> Result<(), Box<dyn Error>> {
        // Act
        let response = rejection_response(r#"{"count":1}"#, None, None).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        Ok(())
    }

    #[tokio::test]
    async fn json_rejection_oversized_body_returns_payload_too_large() -> Result<(), Box<dyn Error>>
    {
        // Act
        let response =
            rejection_response(r#"{"count":1234567890}"#, Some("application/json"), Some(8))
                .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            error_message_of(&response).as_deref(),
            Some("the request body exceeds the maximum allowed size")
        );
        Ok(())
    }

    #[tokio::test]
    async fn from_bytes_rejection_oversized_body_logs_limit_exceeded_once()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let (buffer, _capture) = capture_logs();

        // Act
        let response =
            rejection_response(r#"{"count":1234567890}"#, Some("application/json"), Some(8))
                .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("WARN").count(),
            1,
            "the oversized-body 413 must emit exactly one warn event: {logs}"
        );
        assert_eq!(
            logs.matches("event=\"limit_exceeded\"").count(),
            1,
            "the oversized-body 413 must emit exactly one limit_exceeded event: {logs}"
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
        assert!(
            !logs.contains("1234567890"),
            "the captured event must not contain the request-body value: {logs}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn from_bytes_rejection_oversized_body_does_not_log_buffer_failed()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let (buffer, _capture) = capture_logs();

        // Act
        let response =
            rejection_response(r#"{"count":1234567890}"#, Some("application/json"), Some(8))
                .await?;

        // Assert
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let logs = captured_logs(&buffer);
        assert!(
            !logs.contains("buffer_failed"),
            "the oversized body must not also log the sibling buffer_failed event: {logs}"
        );
        assert!(
            !logs.contains("input_validation_failed"),
            "the oversized body must not also log the input_validation_failed event: {logs}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn from_bytes_rejection_valid_body_logs_no_limit_exceeded() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let (buffer, _capture) = capture_logs();

        // Act
        let response = rejection_response(r#"{"count":1}"#, Some("application/json"), None).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        let logs = captured_logs(&buffer);
        assert!(
            !logs.contains("limit_exceeded"),
            "a valid body within the limit must not emit a limit_exceeded event: {logs}"
        );
        Ok(())
    }
}
