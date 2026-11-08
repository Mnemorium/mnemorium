use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse as _, Response};

use crate::application::port::get_upload::{GetUploadCommand, GetUploadError};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::HAL_CONTENT_TYPE;
use crate::infrastructure::inbound::rest::handler::asset::UploadSessionResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a get-upload error to its API error.
impl From<GetUploadError> for ApiError {
    fn from(err: GetUploadError) -> Self {
        match err {
            GetUploadError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetUploadError::NoSuchUpload => Self::NotFound(err.to_string()),
            GetUploadError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch the state of an upload session.
///
/// Returns the received-chunk bitmap, the total number of chunks, the expiry
/// instant, whether the upload is finished and the identifier of the caller's
/// file for the session digest, when one exists. The file identifier is
/// reported whether or not the session is finished: a caller-scoped duplicate
/// completion leaves the session open and returns this same identifier.
///
/// An expired session is reported as `404`, exactly like an unknown one, so a
/// retried read never changes the result. As a side effect the read lazily
/// deletes the expired session's row and staged file before answering.
///
/// There is no collection route: a client resumes an interrupted upload by
/// retaining the session identifier it received when the session began and
/// reading this session's bitmap, then sending only the chunks the bitmap does
/// not report as received.
#[utoipa::path(
    get,
    operation_id = "get_upload",
    path = "/upload/{upload_id}",
    tag = "asset",
    params(
        ("upload_id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (
            status = OK,
            body = UploadSessionResponse,
            content_type = "application/hal+json",
            description = "Upload session found"
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
            description = "Unknown or expired upload session"
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
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Fetch the state of an upload session"
)]
pub(crate) async fn get_upload(
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
        .get_upload()
        .execute(GetUploadCommand::new(upload_id, caller.user_id()))
        .await?;
    let body = UploadSessionResponse::new(
        upload_id,
        response.bitmap().to_owned(),
        response.total_chunks(),
        response.expires_at(),
        response.is_finished(),
        response.file_id(),
    );
    let mut http_response = Json(body).into_response();
    http_response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(HAL_CONTENT_TYPE),
    );
    Ok(http_response)
}
