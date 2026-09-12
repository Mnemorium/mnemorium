use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Payload of a chunk upload, declared for the `OpenAPI` contract.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PostAssetChunkRequest {
    /// Raw bytes of the uploaded chunk.
    #[schema(format = "binary")]
    pub chunk: String,
}

/// Store one chunk of an upload session.
///
/// Persists the raw bytes carried by the request body for the upload session
/// identified by `id`.
#[utoipa::path(
    post,
    operation_id = "post_asset_chunk",
    path = "/asset/{id}",
    tag = "asset",
    request_body(
        content_type = "application/octet-stream",
        content = PostAssetChunkRequest,
    ),
    params(
        ("id" = NumericID, Path, description = "Identifier of the upload session"),
    ),
    responses(
        (status = NO_CONTENT, description = "Chunk stored"),
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
    summary = "Upload one chunk"
)]
pub async fn post_asset_chunk(
    Path(id): Path<String>,
    State(_state): State<AppState>,
    _caller: AuthenticatedUser,
) -> Result<StatusCode, ApiError> {
    let Ok(_id) = id.parse::<NumericID>() else {
        return Err(ApiError::BadRequest(
            "invalid upload session identifier".to_owned(),
        ));
    };
    unimplemented!()
}
