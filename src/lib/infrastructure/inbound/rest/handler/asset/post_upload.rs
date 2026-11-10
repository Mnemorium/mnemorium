use axum::Json;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::{
    BeginUploadCommand, BeginUploadError, BeginUploadResponse as BeginUploadResponseData,
};
use crate::domain::NumericID;
use crate::domain::model::{DIGEST_HEX_LENGTH, IntegrityHash};
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::asset::UploadSessionLinks;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, HAL_CONTENT_TYPE};

/// Payload initializing an upload session.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PostUploadRequest {
    /// Declared media type of the file content.
    #[schema(
        example = json!("video/mp4"),
        pattern = "^[A-Za-z0-9!#$&^_.+-]+/[A-Za-z0-9!#$&^_.+-]+$"
    )]
    pub content_type: String,
    /// Name of the file being uploaded.
    #[schema(min_length = 1, example = json!("clip.mp4"))]
    pub file_name: String,
    /// Total size of the file being uploaded, in bytes.
    #[schema(minimum = 1, example = json!(1_048_576))]
    pub file_size: u64,
    /// Integrity hash of the complete file, as a 64-character lowercase
    /// hexadecimal SHA-256 string.
    #[schema(
        min_length = 64,
        max_length = 64,
        pattern = "^[0-9a-f]{64}$",
        example = json!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    )]
    pub integrity_hash: String,
}

/// Upload session created by a successful initialization.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct PostUploadResponse {
    /// Fixed size of every chunk except the last, in bytes.
    pub chunk_size: u64,
    /// Date and time at which the upload session expires, as
    /// `YYYY-MM-DDTHH:MM:SS`.
    #[schema(example = json!("2026-01-01T12:00:00"))]
    pub expires_at: String,
    /// Links to the created upload session.
    #[serde(rename = "_links")]
    pub links: UploadSessionLinks,
    /// Unique identifier of the upload session.
    pub upload_id: NumericID,
}

/// Map the begin-upload response onto its HTTP representation.
impl From<BeginUploadResponseData> for PostUploadResponse {
    fn from(response: BeginUploadResponseData) -> Self {
        let upload_id = response.upload_id();
        Self {
            chunk_size: response.chunk_size(),
            expires_at: response
                .expires_at()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
            links: UploadSessionLinks::for_upload(upload_id),
            upload_id,
        }
    }
}

/// Map a begin-upload error to its API error.
impl From<BeginUploadError> for ApiError {
    fn from(err: BeginUploadError) -> Self {
        match err {
            BeginUploadError::InvalidFileName | BeginUploadError::InvalidFileSize => {
                Self::UnprocessableEntity(err.to_string())
            }
            BeginUploadError::FileTooLarge => Self::PayloadTooLarge(err.to_string()),
            BeginUploadError::LibraryUnavailable => Self::ServiceUnavailable(err.to_string()),
            BeginUploadError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            BeginUploadError::Unknown(_) => Self::InternalServerError,
            BeginUploadError::UnsupportedMediaType => Self::UnsupportedMediaType(err.to_string()),
        }
    }
}

/// Initialize an upload session.
///
/// Validates the declared content type, file name, size and integrity hash,
/// creates the upload session and its staging file, and returns the session
/// identifier, the fixed chunk size and the expiry instant.
#[utoipa::path(
    post,
    operation_id = "post_upload",
    path = "/upload",
    tag = "asset",
    request_body(
        content_type = "application/json",
        content = PostUploadRequest,
    ),
    responses(
        (
            status = CREATED,
            body = PostUploadResponse,
            content_type = "application/hal+json",
            headers(
                ("Location" = String, description = "Canonical URI of the created upload session"),
            ),
            description = "Upload session created"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Malformed request body or invalid integrity hash"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = METHOD_NOT_ALLOWED,
            body = ErrorBody,
            content_type = "application/hal+json",
            headers(
                ("Allow" = String, description = "HTTP methods accepted by this path"),
            ),
            description = "Method not allowed"
        ),
        (
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The declared file size or the request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The declared content type is not a supported media type"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body does not match the expected schema, or the file name or size fails validation"
        ),
        (
            status = SERVICE_UNAVAILABLE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The media library root is absent"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Initialize an upload session"
)]
pub(crate) async fn post_upload(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PostUploadRequest>,
) -> Result<Response, ApiError> {
    let JsonObject(request) = payload;
    // The handler is where the wire form is parsed into the domain value object:
    // a malformed digest never reaches the use case.
    let integrity_hash = IntegrityHash::<DIGEST_HEX_LENGTH>::try_new(request.integrity_hash)
        .map_err(|_| ApiError::BadRequest("the integrity hash is invalid".to_owned()))?;
    let response = state
        .asset_use_case_factory()
        .begin_upload()
        .execute(BeginUploadCommand::new(
            request.file_name,
            request.file_size,
            request.content_type,
            integrity_hash,
            caller.user_id(),
        ))
        .await?;
    let body = PostUploadResponse::from(response);
    let location = body.links.self_link.href.clone();
    let mut http_response = Json(body).into_response();
    *http_response.status_mut() = StatusCode::CREATED;
    let headers = http_response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(HAL_CONTENT_TYPE),
    );
    headers.insert(
        header::LOCATION,
        HeaderValue::from_str(&location).map_err(|_| ApiError::InternalServerError)?,
    );
    Ok(http_response)
}
