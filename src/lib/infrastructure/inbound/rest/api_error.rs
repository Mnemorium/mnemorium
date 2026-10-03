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
/// no access to the request URI, so it cannot fill `_links.self` (`API-032`).
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
        // parser detail, so it is logged and never echoed (`API-039`, § 2.5).
        match rejection {
            JsonRejection::MissingJsonContentType(_) => Self::UnsupportedMediaType(
                "the request body must use Content-Type: application/json".to_owned(),
            ),
            JsonRejection::JsonDataError(error) => {
                warn!(
                    error = %error,
                    "rejected a JSON request body that does not match the expected schema"
                );
                Self::UnprocessableEntity(
                    "the request body does not match the expected schema".to_owned(),
                )
            }
            JsonRejection::JsonSyntaxError(error) => {
                warn!(error = %error, "rejected a JSON request body that is not valid JSON");
                Self::BadRequest("the request body is not valid JSON".to_owned())
            }
            JsonRejection::BytesRejection(bytes_rejection) => {
                Self::from_bytes_rejection(&bytes_rejection)
            }
            unknown => {
                warn!(error = %unknown, "rejected a request body");
                Self::BadRequest("the request body is invalid".to_owned())
            }
        }
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
            return Self::PayloadTooLarge(
                "the request body exceeds the maximum allowed size".to_owned(),
            );
        }
        warn!(error = %rejection, "failed to buffer the request body");
        Self::BadRequest("the request body could not be read".to_owned())
    }
}

/// Build the one HAL error envelope every error response is served as
/// (`API-039`): the message at the root (`API-033`) plus `_links.self` to the
/// request URI (`API-032`), served as `application/hal+json` (`API-031`).
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

    use crate::infrastructure::inbound::rest::app_state::AppState;
    use crate::test_helpers::app_state;
    use crate::test_helpers::error_message_of;

    use super::ApiError;

    /// Body schema the probe route deserializes into.
    ///
    /// The field is never read: it exists only so a wrong-typed body yields a
    /// `JsonDataError` rather than a `JsonSyntaxError`.
    #[derive(serde::Deserialize, serde::Serialize)]
    struct Probe {
        count: u64,
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
}
