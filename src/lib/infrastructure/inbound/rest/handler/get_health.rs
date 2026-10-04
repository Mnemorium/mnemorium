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
    security(()),
    summary = "Check service health"
)]
pub async fn get_health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use axum::Router;
    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::routing::get;
    use serde_json::Value;
    use serde_json::json;
    use tower::ServiceExt as _;

    use super::get_health;

    #[tokio::test]
    async fn get_health_service_running_returns_ok() -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder()
            .method("GET")
            .uri("/health")
            .body(Body::empty())?;
        let router = Router::new().route("/health", get(get_health));

        // Act
        let response = router.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let payload: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(payload, json!({ "status": "ok" }));
        Ok(())
    }
}
