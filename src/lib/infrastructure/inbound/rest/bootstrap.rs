use axum::routing::get;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::get_health::get_health;
use crate::infrastructure::inbound::rest::handler::identity::identity_routes;

pub fn setup_routes(state: &AppState) -> axum::Router {
    let v1 = axum::Router::new().nest("/identity", identity_routes(state));

    axum::Router::new()
        .route("/health", get(get_health))
        .nest("/api/v1", v1)
}
