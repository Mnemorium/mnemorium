pub mod get_me;
pub mod get_user;
pub mod get_user_list;

use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::get;

use crate::application::port::UseCaseCatalog;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::user::get_me::get_me;
use crate::infrastructure::inbound::rest::handler::user::get_user::get_user;
use crate::infrastructure::inbound::rest::handler::user::get_user_list::get_user_list;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Routes of the user bounded context.
///
/// Every route requires an authenticated caller.
pub fn user_routes(state: &AppState, catalog: &UseCaseCatalog) -> Router {
    let me: Router<AppState> = Router::new()
        .route("/me", get(get_me))
        .with_state(Arc::clone(&catalog.get_current_user));

    let stubs = Router::new()
        .route("/", get(get_user_list))
        .route("/{id}", get(get_user));

    me.merge(stubs)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}
