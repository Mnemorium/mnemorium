use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse as _, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::complete_upload::{
    CompleteUploadCommand, CompleteUploadError,
    CompleteUploadResponse as CompleteUploadResponseData,
};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{HAL_CONTENT_TYPE, Link};
use crate::infrastructure::inbound::rest::handler::asset::{asset_file_href, upload_self_href};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// HAL links exposed by a completed upload.
///
/// The `self` link addresses the upload session and `file` the file the
/// completion finalized, so a client follows `upload -> file -> image` without
/// holding a raw identifier.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
struct UploadCompleteLinks {
    /// Link to the file finalized by the completion.
    pub file: Link,
    /// Link to the completed upload session.
    #[serde(rename = "self")]
    pub self_link: Link,
}

impl UploadCompleteLinks {
    /// Build the links of the upload session identified by `upload_id`.
    #[must_use]
    pub(crate) fn for_upload(upload_id: NumericID, file_id: NumericID) -> Self {
        Self {
            file: Link::new(&asset_file_href(file_id)),
            self_link: Link::new(&upload_self_href(upload_id)),
        }
    }
}

/// File finalized by a successful completion.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct PostUploadCompleteResponse {
    /// Whether the upload session has been finished.
    pub is_finished: bool,
    /// Links to the completed upload session and the finalized file.
    #[serde(rename = "_links")]
    pub links: UploadCompleteLinks,
}

impl PostUploadCompleteResponse {
    /// Map the completed upload onto its HTTP representation.
    #[must_use]
    pub(crate) fn new(upload_id: NumericID, response: &CompleteUploadResponseData) -> Self {
        Self {
            is_finished: response.is_finished(),
            links: UploadCompleteLinks::for_upload(upload_id, response.file_id()),
        }
    }
}

/// Map a complete-upload error to its API error.
impl From<CompleteUploadError> for ApiError {
    fn from(err: CompleteUploadError) -> Self {
        match err {
            CompleteUploadError::Incomplete
            | CompleteUploadError::IntegrityMismatch
            | CompleteUploadError::UnsupportedMedia => Self::UnprocessableEntity(err.to_string()),
            CompleteUploadError::CompletionInProgress => Self::Conflict(err.to_string()),
            CompleteUploadError::Expired => Self::Gone(err.to_string()),
            CompleteUploadError::LibraryUnavailable => Self::ServiceUnavailable(err.to_string()),
            CompleteUploadError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CompleteUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            CompleteUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Complete an upload session.
///
/// The upload identifier is carried by the path; there is no request body. The
/// staged content's integrity hash is recomputed and verified against the hash
/// declared when the upload was initialized. When the caller already owns a
/// file with that hash, no second copy is stored: the existing file's
/// identifier is returned and the upload session is left unfinished. Otherwise
/// the staged file is promoted, its record registered, and the upload marked
/// finished. Completion is idempotent: completing an already finished upload
/// returns the caller's file for the same digest.
#[utoipa::path(
    post,
    operation_id = "post_upload_complete",
    path = "/upload/{upload_id}/complete",
    tag = "asset",
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (
            status = OK,
            body = PostUploadCompleteResponse,
            content_type = "application/hal+json",
            description = "Upload completed"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid upload session identifier"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The upload is incomplete, its integrity does not match, or its content is not supported media"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown upload session"
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
            status = GONE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Upload session expired"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Another completion of the upload session is in progress"
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
    summary = "Complete an upload session"
)]
pub(crate) async fn post_upload_complete(
    Path(upload_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let Ok(upload_id) = upload_id_value.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    let response = state
        .asset_use_case_factory()
        .complete_upload()
        .execute(CompleteUploadCommand::new(upload_id, caller.user_id()))
        .await?;
    let body = PostUploadCompleteResponse::new(upload_id, &response);
    let mut http_response = Json(body).into_response();
    http_response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(HAL_CONTENT_TYPE),
    );
    Ok(http_response)
}
