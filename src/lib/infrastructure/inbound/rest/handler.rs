#![allow(
    clippy::missing_errors_doc,
    reason = "Handler are declared in the OpenAPI spec"
)]

pub mod asset;
pub mod get_health;
pub mod identity;
pub mod user;

use axum::middleware;
use axum::routing::get;

use crate::application::port::UseCaseCatalog;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
use crate::infrastructure::inbound::rest::handler::get_health::get_health;
use crate::infrastructure::inbound::rest::handler::identity::identity_routes;
use crate::infrastructure::inbound::rest::handler::user::user_routes;
use crate::infrastructure::inbound::rest::middleware::trace::tracing;

pub fn setup_routes(state: &AppState, catalog: &UseCaseCatalog) -> axum::Router {
    let v1 = axum::Router::new()
        .nest("/asset", asset_routes(state))
        .nest("/identity", identity_routes(state, catalog))
        .nest("/user", user_routes(state, catalog));

    axum::Router::new()
        .route("/health", get(get_health))
        .nest("/api/v1", v1)
        .layer(middleware::from_fn(tracing))
}
