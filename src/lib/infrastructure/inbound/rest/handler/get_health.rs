use axum::response::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Service health status.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct HealthResponse {
    /// Service status.
    #[schema(example = json!("ok"))]
    pub status: String,
}

/// Check the service health.
///
/// Returns `200` with the current health status when the service is running.
#[utoipa::path(
    get,
    operation_id = "get_health",
    path = "/health",
    tag = "system",
    servers(
        (url = "http://0.0.0.0:4080", description = "Root of the service, outside /api/v1")
    ),
    responses(
        (status = OK, body = HealthResponse, description = "Service is healthy"),
    ),
    summary = "Check service health"
)]
pub async fn get_health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_owned(),
    })
}
