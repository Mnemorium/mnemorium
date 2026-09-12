use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload initializing an upload session.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PostAssetRequest {
    /// Declared media type of the file content.
    pub content_type: String,
    /// Name of the media file being uploaded.
    pub file_name: String,
}

/// Upload session created by a successful initialization.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostAssetResponse {
    /// Unique identifier of the upload session.
    pub id: NumericID,
}

/// Initialize an upload session.
///
/// Validates that the declared content type is a supported media type and
/// returns the identifier of the created upload session.
#[utoipa::path(
    post,
    operation_id = "post_asset",
    path = "/asset",
    tag = "asset",
    request_body(
        content_type = "application/json",
        content = PostAssetRequest,
    ),
    responses(
        (
            status = CREATED,
            body = PostAssetResponse,
            description = "Upload session created"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            description = "The declared content type is not a supported media type"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Initialize an upload session"
)]
pub async fn post_asset(
    State(_state): State<AppState>,
    _caller: AuthenticatedUser,
) -> Result<Json<PostAssetResponse>, ApiError> {
    unimplemented!()
}
