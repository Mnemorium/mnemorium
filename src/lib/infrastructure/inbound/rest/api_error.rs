use axum::Json;
use axum::extract::path::ErrorKind;
use axum::extract::rejection::BytesRejection;
use axum::extract::rejection::JsonRejection;
use axum::extract::rejection::PathRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::response::Response;
use tracing::error;
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
    #[expect(
        clippy::cognitive_complexity,
        reason = "each rejection arm emits its catalogued event inline so the field sets stay aligned with OBS-006"
    )]
    fn from(rejection: JsonRejection) -> Self {
        // The client receives a stable, server-authored message only
        // (`API-040`): the framework rejection text carries deserialization and
        // parser detail and the offending value, so only its classification is
        // logged (`OBS-003`) and it is never echoed (`API-039`, § 2.5). Each
        // catalog event carries its declared field set (`OBS-006`): `field` and
        // `reason` for `input_validation_failed`, `source` and `reason` for
        // `deserialization_failed`.
        match rejection {
            JsonRejection::BytesRejection(bytes_rejection) => {
                Self::from_bytes_rejection(&bytes_rejection)
            }
            JsonRejection::MissingJsonContentType(_) => {
                warn!(
                    target: "security",
                    event = "input_validation_failed",
                    field = "content_type",
                    reason = "missing_content_type",
                    "rejected a request body"
                );
                Self::UnsupportedMediaType(
                    "the request body must use Content-Type: application/json".to_owned(),
                )
            }
            JsonRejection::JsonDataError(_) => {
                warn!(
                    target: "security",
                    event = "deserialization_failed",
                    source = "json_body",
                    reason = "schema_mismatch",
                    "rejected a request body"
                );
                Self::UnprocessableEntity(
                    "the request body does not match the expected schema".to_owned(),
                )
            }
            JsonRejection::JsonSyntaxError(_) => {
                warn!(
                    target: "security",
                    event = "deserialization_failed",
                    source = "json_body",
                    reason = "syntax",
                    "rejected a request body"
                );
                Self::BadRequest("the request body is not valid JSON".to_owned())
            }
            _unknown => {
                warn!(
                    target: "security",
                    event = "deserialization_failed",
                    source = "json_body",
                    reason = "unknown",
                    "rejected a request body"
                );
                Self::BadRequest("the request body is invalid".to_owned())
            }
        }
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        // The client receives a stable, server-authored message only
        // (`API-040`): axum's path rejection text embeds the raw rejected value
        // and the expected type name, so only its classification is logged
        // (`OBS-003`) and it is never echoed. A parse or UTF-8 failure is a
        // client `400` (`API-041`); a missing path-parameter extension, an
        // unsupported target type or an unclassified variant is an internal
        // `500` and an operator-actionable `extractor_fault` (`OBS-005`).
        match rejection {
            PathRejection::FailedToDeserializePathParams(failure) => {
                Self::from(failure.into_kind())
            }
            PathRejection::MissingPathParams(_) => {
                Self::reject_misconfigured_path("missing_path_params")
            }
            _unknown => Self::reject_misconfigured_path("unclassified_rejection"),
        }
    }
}

impl From<ErrorKind> for ApiError {
    /// Map a failed path-parameter deserialization to a `400` or a `500`.
    ///
    /// The known [`ErrorKind`] variants are matched so every classification
    /// emits its own catalogued event (`OBS-006`) and the server-authored
    /// message stays free of the raw value and the expected type name
    /// (`API-040`). `ErrorKind` is `#[non_exhaustive]`, so the wildcard is the
    /// `#[non_exhaustive]` forward-compatibility guard; an unclassified variant
    /// is an internal `500` under the § 3 response-code table (`API-041`), not
    /// a client error, and logs `extractor_fault`.
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::ParseError { .. }
            | ErrorKind::ParseErrorAtIndex { .. }
            | ErrorKind::ParseErrorAtKey { .. } => Self::reject_path("parse"),
            ErrorKind::DeserializeError { .. } => Self::reject_path("deserialize"),
            ErrorKind::InvalidUtf8InPathParam { .. } => Self::reject_path("invalid_utf8"),
            ErrorKind::Message(_) => Self::reject_path("custom_message"),
            ErrorKind::WrongNumberOfParameters { .. } => {
                Self::reject_misconfigured_path("wrong_number_of_parameters")
            }
            ErrorKind::UnsupportedType { .. } => {
                Self::reject_misconfigured_path("unsupported_type")
            }
            _unknown => Self::reject_misconfigured_path("unclassified_rejection"),
        }
    }
}

impl ApiError {
    /// Log an operator-actionable path-extraction fault and return a generic
    /// `500`.
    fn reject_misconfigured_path(reason: &'static str) -> Self {
        error!(
            target: "security",
            event = "extractor_fault",
            reason,
            "rejected a misconfigured request path"
        );
        Self::InternalServerError
    }

    /// Log a client path-parameter failure and return a generic `400`.
    fn reject_path(reason: &'static str) -> Self {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "path",
            reason,
            "rejected a request path"
        );
        Self::BadRequest("the request path is invalid".to_owned())
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
            field = "body",
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
    use axum::extract::path::ErrorKind;
    use axum::extract::rejection::JsonRejection;
    use axum::extract::rejection::MissingPathParams;
    use axum::extract::rejection::PathRejection;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use axum::routing::post;
    use rstest::rstest;
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

    #[rstest]
    #[case::parse(
        ErrorKind::ParseError { value: "abc".to_owned(), expected_type: "i64" },
        StatusCode::BAD_REQUEST
    )]
    #[case::parse_at_index(
        ErrorKind::ParseErrorAtIndex { index: 0, value: "abc".to_owned(), expected_type: "i64" },
        StatusCode::BAD_REQUEST
    )]
    #[case::parse_at_key(
        ErrorKind::ParseErrorAtKey { key: "id".to_owned(), value: "abc".to_owned(), expected_type: "i64" },
        StatusCode::BAD_REQUEST
    )]
    #[case::deserialize(
        ErrorKind::DeserializeError { key: "id".to_owned(), value: "abc".to_owned(), message: "nope".to_owned() },
        StatusCode::BAD_REQUEST
    )]
    #[case::invalid_utf8(
        ErrorKind::InvalidUtf8InPathParam { key: "id".to_owned() },
        StatusCode::BAD_REQUEST
    )]
    #[case::message(ErrorKind::Message("custom".to_owned()), StatusCode::BAD_REQUEST)]
    #[case::wrong_number(
        ErrorKind::WrongNumberOfParameters { got: 0, expected: 1 },
        StatusCode::INTERNAL_SERVER_ERROR
    )]
    #[case::unsupported_type(
        ErrorKind::UnsupportedType { name: "Vec<i64>" },
        StatusCode::INTERNAL_SERVER_ERROR
    )]
    fn path_error_kind_maps_to_its_status(#[case] kind: ErrorKind, #[case] expected: StatusCode) {
        // Act
        let (status, _message) = ApiError::from(kind).status_and_message();

        // Assert
        assert_eq!(status, expected);
    }

    #[test]
    fn missing_path_params_maps_to_internal_server_error() {
        // Arrange
        let (buffer, _capture) = capture_logs();
        let rejection = PathRejection::MissingPathParams(MissingPathParams::default());

        // Act
        let (status, _message) = ApiError::from(rejection).status_and_message();

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("event=\"extractor_fault\"").count(),
            1,
            "a missing path-parameter extension must emit exactly one extractor_fault: {logs}"
        );
        assert!(
            logs.contains("reason=\"missing_path_params\""),
            "the event must classify the reason: {logs}"
        );
    }

    #[test]
    fn client_path_rejection_logs_one_classified_event_without_the_value() {
        // Arrange
        let (buffer, _capture) = capture_logs();

        // Act
        let (status, message) = ApiError::from(ErrorKind::ParseError {
            value: "LEAK_SENTINEL".to_owned(),
            expected_type: "i64",
        })
        .status_and_message();

        // Assert
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(message, "the request path is invalid");
        assert!(
            !message.contains("LEAK_SENTINEL"),
            "the client message must not echo the rejected value (`API-040`)"
        );
        assert!(
            !message.contains("i64"),
            "the client message must not echo the expected type name (`API-040`)"
        );
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("WARN").count(),
            1,
            "a client path rejection must emit exactly one warn event: {logs}"
        );
        assert_eq!(
            logs.matches("event=\"input_validation_failed\"").count(),
            1,
            "a client path rejection must emit exactly one input_validation_failed event: {logs}"
        );
        assert!(
            logs.contains("field=\"path\""),
            "the event must identify the path field: {logs}"
        );
        assert!(
            logs.contains("reason=\"parse\""),
            "the event must classify the reason: {logs}"
        );
        assert!(
            logs.contains("security"),
            "the event must target the reserved security target: {logs}"
        );
        assert!(
            !logs.contains("extractor_fault"),
            "a client path rejection must not emit the server event: {logs}"
        );
        assert!(
            !logs.contains("LEAK_SENTINEL"),
            "the event must not echo the rejected value: {logs}"
        );
        assert!(
            !logs.contains("i64"),
            "the event must not echo the expected type name: {logs}"
        );
    }

    #[test]
    fn misconfigured_path_logs_one_extractor_fault() {
        // Arrange
        let (buffer, _capture) = capture_logs();

        // Act
        let (status, message) = ApiError::from(ErrorKind::WrongNumberOfParameters {
            got: 0,
            expected: 1,
        })
        .status_and_message();

        // Assert
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(message, "an unexpected error occurred");
        let logs = captured_logs(&buffer);
        assert_eq!(
            logs.matches("ERROR").count(),
            1,
            "a server path misconfiguration must emit exactly one error event: {logs}"
        );
        assert_eq!(
            logs.matches("event=\"extractor_fault\"").count(),
            1,
            "a server path misconfiguration must emit exactly one extractor_fault: {logs}"
        );
        assert!(
            logs.contains("reason=\"wrong_number_of_parameters\""),
            "the event must classify the reason: {logs}"
        );
        assert!(
            !logs.contains("input_validation_failed"),
            "a server fault must not be logged as a client-validation event: {logs}"
        );
    }
}
