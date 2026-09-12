use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// File finalized by a successful upload.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PostAssetFinishResponse {
    /// Name of the media file that was uploaded.
    pub file_name: String,
    /// Unique identifier of the stored file.
    pub id: NumericID,
}

/// Finalize an upload session.
///
/// Marks the upload session identified by `id` as finished and returns the
/// stored file.
#[utoipa::path(
    post,
    operation_id = "post_asset_finish",
    path = "/asset/{id}/finish",
    tag = "asset",
    params(
        ("id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (
            status = OK,
            body = PostAssetFinishResponse,
            description = "Upload finished"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid upload session identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            description = "Unknown upload session"
        ),
        (
            status = GONE,
            body = ErrorBody,
            description = "Upload session already finished or expired"
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
    summary = "Finish an upload session"
)]
pub async fn post_asset_finish(
    Path(id): Path<String>,
    State(_state): State<AppState>,
    _caller: AuthenticatedUser,
) -> Result<Json<PostAssetFinishResponse>, ApiError> {
    let Ok(_id) = id.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    unimplemented!()
}
