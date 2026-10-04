pub mod patch_credential;
pub mod post_login;
pub mod post_register;

use axum::Router;
use axum::middleware;
use axum::routing::patch;
use axum::routing::post;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::identity::patch_credential::patch_credential;
use crate::infrastructure::inbound::rest::handler::identity::post_login::post_login;
use crate::infrastructure::inbound::rest::handler::identity::post_register::post_register;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::inbound::rest::middleware::rate_limit;
use crate::infrastructure::inbound::rest::middleware::rate_limit::RateLimitCleanup;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// Routes of the identity bounded context.
///
/// `login` is public; `register` and the credential password change require an
/// authenticated caller. The public `login` route is rate limited per client
/// address (`RateLimitCleanup`); the other routes are not.
pub fn identity_routes(state: &AppState) -> (Router, RateLimitCleanup) {
    let register: Router<AppState> = Router::new().route("/register", post(post_register));

    let patch_credential: Router<AppState> =
        Router::new().route("/credential/{id}", patch(patch_credential));

    let login: Router<AppState> = Router::new().route("/login", post(post_login));

    let rate_limit = state.configuration().load().security().rate_limit().clone();
    let (rate_limited_login, cleanup) = rate_limit::apply_login_rate_limit(login, &rate_limit);

    let router = register
        .merge(patch_credential)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .merge(rate_limited_login)
        .with_state(state.clone());

    (router, cleanup)
}
