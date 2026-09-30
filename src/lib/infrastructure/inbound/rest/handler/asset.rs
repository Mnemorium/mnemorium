pub mod get_upload;
pub mod links;
pub mod post_upload;
pub mod post_upload_complete;
pub mod put_upload_chunk;
pub mod upload_session;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::get;
use axum::routing::post;
use axum::routing::put;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::get_upload::get_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::post_upload;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_complete::post_upload_complete;
use crate::infrastructure::inbound::rest::handler::asset::put_upload_chunk::put_upload_chunk;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Routes of the asset bounded context.
///
/// Every route requires an authenticated caller.
pub fn asset_routes(state: &AppState) -> Router {
    // The chunk endpoint receives a full chunk (up to `chunk_size_bytes`), which
    // exceeds axum's 2 MiB default request body limit, so that route's limit is
    // raised to the configured chunk size; every other route keeps the default.
    //
    // TODO(hot-reload): the limit is fixed when the router is built, so a runtime
    // change to `asset.upload.chunk_size_bytes` only takes effect after a
    // restart.
    let chunk_body_limit = usize::try_from(
        state
            .configuration()
            .load()
            .asset()
            .upload()
            .chunk_size_bytes(),
    )
    .unwrap_or(usize::MAX);

    let upload: Router<AppState> = Router::new()
        .route("/upload", post(post_upload))
        .route("/upload/{upload_id}/complete", post(post_upload_complete))
        .route("/upload/{upload_id}", get(get_upload))
        .route(
            "/upload/{upload_id}/chunk/{chunk_number}",
            put(put_upload_chunk).layer(DefaultBodyLimit::max(chunk_body_limit)),
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
    #[expect(
        clippy::single_call_fn,
        reason = "the test router mirrors the asset routes"
    )]
    fn router() -> Result<axum::Router, Box<dyn Error>> {
        let mut complete_upload = MockCompleteUploadUseCase::new();
        complete_upload
            .expect_execute()
            .times(0..=1)
            .return_once(|_| Box::pin(async { Ok(CompleteUploadResponse::new(11, true)) }));

        let mut factory = MockAssetUseCaseFactory::new();
        factory
            .expect_complete_upload()
            .times(0..=1)
            .return_once(move || Arc::new(complete_upload) as Arc<dyn CompleteUploadUseCase>);

        let state = app_state_with_asset(Arc::new(factory))?;
        Ok(asset_routes(&state))
    }

    /// Mint a bearer token the test token provider accepts.
    #[expect(
        clippy::single_call_fn,
        reason = "the test token factory mirrors the auth middleware"
    )]
    async fn bearer() -> Result<String, Box<dyn Error>> {
        let provider = JwtTokenProvider::new(TEST_SECRET.to_owned(), 3600);
        Ok(provider.issue(3).await?.value().to_owned())
    }

    #[tokio::test]
    async fn asset_routes_complete_route_reaches_handler() -> Result<(), Box<dyn Error>> {
        // Arrange
        let request = Request::builder()
            .method("POST")
            .uri("/upload/7/complete")
            .header(header::AUTHORIZATION, format!("Bearer {}", bearer().await?))
            .body(Body::empty())?;

        // Act
        let response: Response = router()?.oneshot(request).await?;

        // Assert
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }
}
