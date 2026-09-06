use axum::middleware;
use axum::routing::{get, post};

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::get_health::get_health;
use crate::infrastructure::inbound::rest::handler::identity::post_register::post_register;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

pub fn setup_routes(state: AppState) -> axum::Router {
    let v1 = axum::Router::new()
        .route("/identity/register", post(post_register))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state);

    axum::Router::new()
        .route("/health", get(get_health))
        .nest("/api/v1", v1)
}
