pub mod get_upload;
pub mod post_upload;
pub mod post_upload_check;
pub mod post_upload_complete;
pub mod put_upload_chunk;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use axum::routing::post;
use axum::routing::put;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::get_upload::get_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::post_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_check::post_upload_check;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_complete::post_upload_complete;
use crate::infrastructure::inbound::rest::handler::asset::put_upload_chunk::put_upload_chunk;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Routes of the asset bounded context.
///
/// Every route requires an authenticated caller.
pub fn asset_routes(state: &AppState) -> Router {
    let upload: Router<AppState> = Router::new()
        .route("/upload", post(post_upload))
        .route("/upload/complete", post(post_upload_complete))
        .route("/upload/check", post(post_upload_check))
        .route("/upload/{upload_id}", get(get_upload))
        .route(
            "/upload/{upload_id}/chunk/{chunk_number}",
            put(put_upload_chunk),
        )
        .with_state(state.clone());

    upload
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::http::header;
    use axum::response::Response;
    use serde_json::json;
    use tower::ServiceExt as _;

    use crate::application::port::asset_use_case_factory::MockAssetUseCaseFactory;
    use crate::application::port::complete_upload::CompleteUploadResponse;
    use crate::application::port::complete_upload::CompleteUploadUseCase;
    use crate::application::port::complete_upload::MockCompleteUploadUseCase;
    use crate::domain::port::token_provider::TokenProvider as _;
    use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
    use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;
    use crate::test_helpers::app_state_with_asset;

    /// Secret the test `AppState` token provider is built with.
    const TEST_SECRET: &str = "tmptmp";

    /// Build the asset router whose complete-upload use case succeeds.
    fn router() -> Result<axum::Router, Box<dyn Error>> {
        let mut complete_upload = MockCompleteUploadUseCase::new();
        complete_upload
            .expect_execute()
            .times(0..=1)
            .return_once(|_| Box::pin(async { Ok(CompleteUploadResponse::new(11)) }));

        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_complete_upload()
            .times(0..=1)
            .return_once(move || Arc::new(complete_upload) as Arc<dyn CompleteUploadUseCase>);

        let state = app_state_with_asset(Arc::new(factory))?;
        Ok(asset_routes(&state))
    }

    /// Mint a bearer token the test token provider accepts.
    async fn bearer() -> Result<String, Box<dyn Error>> {
        let provider = JwtTokenProvider::new(TEST_SECRET.to_owned(), 3600);
        Ok(provider.issue(3).await?.value().to_owned())
    }

    #[tokio::test]
    async fn asset_routes_static_complete_wins_over_param_upload_id() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let request = Request::builder()
            .method("POST")
            .uri("/upload/complete")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::from(serde_json::to_vec(
                &json!({ "upload_id": 7i64 }),
            )?))?;

        // Act
        let response: Response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn asset_routes_static_check_wins_over_param_upload_id() -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder()
            .method("POST")
            .uri("/upload/check")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = router()?.oneshot(request).await?;

        // Assert
        // The request reaches `post_upload_check` and its empty payload is
        // rejected with `400`; the `GET /upload/{upload_id}` route would reject
        // the method with `405`.
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        Ok(())
    }
}
