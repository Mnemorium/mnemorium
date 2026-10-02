use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::response::Response;

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
        // A body larger than the configured limit is refused by the `Json`
        // extractor before deserialization. `JsonRejection::status` reports the
        // underlying rejection's status, so the `413` is recovered here instead
        // of collapsing into a misleading `400`.
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            return Self::PayloadTooLarge(
                "the request body exceeds the maximum allowed size".to_owned(),
            );
        }
        Self::BadRequest(rejection.body_text())
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
