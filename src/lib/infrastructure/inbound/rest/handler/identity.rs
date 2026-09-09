pub mod post_login;
pub mod post_register;

use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::post;

use crate::application::port::UseCaseCatalog;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::identity::post_login::post_login;
use crate::infrastructure::inbound::rest::handler::identity::post_register::post_register;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Routes of the identity bounded context.
///
/// `login` is public; `register` requires an authenticated caller.
pub fn identity_routes(state: &AppState, catalog: &UseCaseCatalog) -> Router {
    let register: Router<AppState> = Router::new()
        .route("/register", post(post_register))
        .with_state(Arc::clone(&catalog.register_user));

    let login: Router<AppState> = Router::new()
        .route("/login", post(post_login))
        .with_state(Arc::clone(&catalog.login_user));

    register
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .merge(login)
        .with_state(state.clone())
}
