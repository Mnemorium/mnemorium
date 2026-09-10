pub mod patch_credential;
pub mod post_login;
pub mod post_register;

use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::patch;
use axum::routing::post;

use crate::application::port::UseCaseCatalog;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::identity::patch_credential::patch_credential;
use crate::infrastructure::inbound::rest::handler::identity::post_login::post_login;
use crate::infrastructure::inbound::rest::handler::identity::post_register::post_register;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Routes of the identity bounded context.
///
/// `login` is public; `register` and the credential password change require an
/// authenticated caller.
pub fn identity_routes(state: &AppState, catalog: &UseCaseCatalog) -> Router {
    let register: Router<AppState> = Router::new()
        .route("/register", post(post_register))
        .with_state(Arc::clone(&catalog.register_user));

    let patch_credential: Router<AppState> = Router::new()
        .route("/credential/{id}", patch(patch_credential))
        .with_state(Arc::clone(&catalog.patch_credential));

    let login: Router<AppState> = Router::new()
        .route("/login", post(post_login))
        .with_state(Arc::clone(&catalog.login_user));

    register
        .merge(patch_credential)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .merge(login)
        .with_state(state.clone())
}
