pub mod post_asset;
pub mod post_asset_chunk;
pub mod post_asset_finish;

use axum::Router;
use axum::middleware;
use axum::routing::post;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::post_asset::post_asset;
use crate::infrastructure::inbound::rest::handler::asset::post_asset_chunk::post_asset_chunk;
use crate::infrastructure::inbound::rest::handler::asset::post_asset_finish::post_asset_finish;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Routes of the asset bounded context.
///
/// Every route requires an authenticated caller.
pub fn asset_routes(state: &AppState) -> Router {
    let init: Router<AppState> = Router::new()
        .route("/", post(post_asset))
        .with_state(state.clone());

    let chunk: Router<AppState> = Router::new()
        .route("/{id}", post(post_asset_chunk))
        .with_state(state.clone());

    let finish: Router<AppState> = Router::new()
        .route("/{id}/finish", post(post_asset_finish))
        .with_state(state.clone());

    init.merge(chunk)
        .merge(finish)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}
