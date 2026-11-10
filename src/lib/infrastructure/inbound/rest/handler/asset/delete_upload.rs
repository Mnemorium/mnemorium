use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse as _, Response};

use crate::application::port::{CancelUploadCommand, CancelUploadError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Map a cancel-upload error to its API error.
impl From<CancelUploadError> for ApiError {
    fn from(err: CancelUploadError) -> Self {
        match err {
            CancelUploadError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            CancelUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            CancelUploadError::NotCancellable => Self::Conflict(err.to_string()),
            CancelUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Cancel a pending upload session.
///
/// The upload identifier is carried by the path; there is no request body. The
/// session's staged content and its record are deleted and the endpoint answers
/// `204 No Content`. A finished session's content is already the caller's
/// library content, so cancelling it is a `409 Conflict` and deletes nothing; a
/// session being verified is mid-completion and is a conflict too. An unknown
/// or unowned session answers `404 Not Found`, indistinguishable from each
/// other, and a session already cancelled answers `404` as well.
#[utoipa::path(
    delete,
    operation_id = "delete_upload",
    path = "/upload/{upload_id}",
    tag = "asset",
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "Upload session cancelled"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid upload session identifier"
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
            description = "Unknown or unowned upload session"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The upload session is finished or being verified"
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
    summary = "Cancel an upload session"
)]
pub(crate) async fn delete_upload(
    Path(upload_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let Ok(upload_id) = upload_id_value.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    state
        .asset_use_case_factory()
        .cancel_upload()
        .execute(CancelUploadCommand::new(upload_id, caller.user_id()))
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
