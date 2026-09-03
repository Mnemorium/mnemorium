use axum::routing::get;

use crate::infrastructure::inbound::rest::handler::get_health::get_health;

pub fn setup_routes() -> axum::Router {
    let v1 = axum::Router::new();

    axum::Router::new()
        .route("/health", get(get_health))
        .nest("/api/v1", v1)
}
