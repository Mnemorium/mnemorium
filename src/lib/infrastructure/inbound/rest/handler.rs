#![allow(
    clippy::missing_errors_doc,
    reason = "Handler are declared in the OpenAPI spec"
)]

/// The Asset context endpoints.
pub(crate) mod asset;
/// The liveness endpoint.
pub(crate) mod get_health;
/// The Identity context endpoints.
pub(crate) mod identity;
/// The Library context endpoints.
pub(crate) mod library;
/// The User context endpoints.
pub(crate) mod user;

use axum::middleware;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[cfg(feature = "api-docs")]
use utoipa_scalar::Servable as _;

use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
use crate::infrastructure::inbound::rest::handler::identity::identity_routes;
use crate::infrastructure::inbound::rest::handler::library::library_routes;
use crate::infrastructure::inbound::rest::handler::user::user_routes;
use crate::infrastructure::inbound::rest::middleware::hal_errors::hal_errors;
use crate::infrastructure::inbound::rest::middleware::rate_limit::LoginRateLimiter;
use crate::infrastructure::inbound::rest::middleware::trace::tracing;
#[cfg(feature = "api-docs")]
use crate::infrastructure::inbound::rest::openapi;

/// Build the application router.
///
/// The composition root (`src/lib/app.rs`, `STY-RUST-067`) builds the
/// process-lifetime adapters and mounts only the router this function returns.
/// `hal_errors` is applied **last** so it is the outermost layer. It therefore
/// rewraps every `4xx`/`5xx` the server emits, including the routing fallback
/// (an unknown or malformed URL) and framework rejections, as the one HAL error
/// envelope (`API-039`). It must remain outermost.
///
/// The `limiter` is the process-lifetime login rate limiter the composition
/// root built; it is applied to the `login` route here.
///
/// Every route is registered through `utoipa-axum`, so the axum route and the
/// `OpenAPI` path item come from the handler's `#[utoipa::path]` declaration and
/// cannot drift. [`crate::infrastructure::inbound::rest::openapi`] assembles
/// the document from the same registration.
///
/// With the `api-docs` feature the interactive documentation is mounted at
/// `/doc`, outside the `/api/v1` API base; it serves the document
/// `rest::openapi` assembles.
pub(crate) fn setup_routes(state: &AppState, limiter: &LoginRateLimiter) -> axum::Router {
    #[cfg_attr(
        not(feature = "api-docs"),
        expect(unused_mut, reason = "the documentation route is feature-gated")
    )]
    let mut api: OpenApiRouter<()> = OpenApiRouter::new()
        .routes(routes!(get_health::get_health))
        .nest(
            "/api/v1",
            OpenApiRouter::new()
                .nest("/asset", asset_routes(state))
                .nest("/identity", identity_routes(state, limiter))
                .nest("/library", library_routes(state))
                .nest("/user", user_routes(state)),
        );

    #[cfg(feature = "api-docs")]
    {
        let documentation: axum::Router = utoipa_scalar::Scalar::with_url("/doc", openapi()).into();
        api = api.merge(OpenApiRouter::from(documentation));
    }

    api.layer(middleware::from_fn(tracing))
        .layer(middleware::from_fn(hal_errors))
        .into()
}
