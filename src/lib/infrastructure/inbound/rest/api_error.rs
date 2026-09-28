use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;
use serde_json::json;

/// Standard error payload returned by every failed request.
#[derive(Debug, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[non_exhaustive]
pub struct ErrorBody {
    /// Human-readable description of the error.
    #[schema(example = json!("An error message"))]
    pub error: String,
}

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
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::BadRequest(rejection.body_text())
    }
}
