/// The transport error mapped onto an HTTP status.
pub(crate) mod api_error;
/// The single router state every handler extracts.
pub(crate) mod app_state;
pub(crate) mod hal;
/// The route table and the per-context endpoint folders.
pub(crate) mod handler;
/// The authentication, tracing, rate-limit and HAL error layers.
pub(crate) mod middleware;

use utoipa::OpenApi as _;
use utoipa::openapi::OpenApi;
use utoipa::openapi::security::Http;
use utoipa::openapi::security::HttpAuthScheme;
use utoipa::openapi::security::SecurityScheme;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::infrastructure::inbound::rest::handler::asset::asset_router;
use crate::infrastructure::inbound::rest::handler::get_health;
use crate::infrastructure::inbound::rest::handler::identity::identity_protected_router;
use crate::infrastructure::inbound::rest::handler::identity::identity_public_router;
use crate::infrastructure::inbound::rest::handler::library::library_router;
use crate::infrastructure::inbound::rest::handler::user::user_router;

/// Add the `bearer_auth` security scheme referenced by the protected
/// endpoints.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
            );
        }
    }
}

/// Document-scoped members of the Mnemorium HTTP API.
///
/// The path items and their schemas are not declared here: `utoipa-axum`
/// collects them from the handlers as the routers register their routes, so
/// [`openapi`] is the complete document and this derive carries only the
/// members that do not change when an endpoint is added.
#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Mnemorium API",
        version = env!("CARGO_PKG_VERSION"),
        description = "HTTP API of the Mnemorium service"
    ),
    servers(
        (url = "http://0.0.0.0:4080/api/v1", description = "Local development server")
    ),
    tags(
        (name = "system", description = "System-level endpoints"),
        (name = "asset", description = "Asset bounded context"),
        (name = "identity", description = "Identity bounded context"),
        (name = "library", description = "Library bounded context"),
        (name = "user", description = "User bounded context")
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

/// Build the complete `OpenAPI` document of the Mnemorium HTTP API.
///
/// Each handler carries its route and its path item, so the document is
/// assembled from the same declarations the server serves: the context
/// routers register the routes, `utoipa-axum` collects their paths and
/// schemas, and the static [`ApiDoc`] members seed the rest. The context list
/// mirrors [`handler::setup_routes`].
///
/// The paths are relative to the `/api/v1` base the document's `servers`
/// entry declares, so the contexts are nested here without it; the server
/// router adds that prefix at runtime. With the `api-docs` feature
/// [`handler::setup_routes`] mounts this document at `/doc`.
#[must_use]
pub(crate) fn openapi() -> OpenApi {
    let (_, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_health::get_health))
        // The chunk body limit is a runtime layer; the document only needs the
        // route, so no limit is passed here.
        .nest("/asset", asset_router(None))
        .nest(
            "/identity",
            OpenApiRouter::new()
                .merge(identity_protected_router())
                .merge(identity_public_router()),
        )
        .nest("/library", library_router())
        .nest("/user", user_router())
        .split_for_parts();
    openapi
}

#[cfg(test)]
mod tests {
    use super::openapi;

    #[test]
    fn openapi_collects_every_context_and_the_security_scheme() {
        // Act
        let document = openapi();
        let paths = &document.paths.paths;
        let components = document.components.as_ref();

        // Assert: the routes are collected from the same registration the
        // server uses, so a missing context would drop its whole surface.
        assert!(
            paths.contains_key("/health"),
            "the document must declare the system route"
        );
        assert!(
            paths.contains_key("/asset/upload"),
            "the document must declare the asset context"
        );
        assert!(
            paths.contains_key("/identity/login"),
            "the document must declare the identity context"
        );
        assert!(
            paths.contains_key("/library/gallery"),
            "the document must declare the library context"
        );
        assert!(
            paths.contains_key("/user") && paths.contains_key("/user/me"),
            "the document must declare the user context"
        );
        assert!(
            components.is_some_and(|document_components| {
                document_components.schemas.contains_key("ErrorBody")
            }),
            "the document must carry the payload schemas the handlers reference"
        );
        assert!(
            components.is_some_and(|document_components| {
                document_components
                    .security_schemes
                    .contains_key("bearer_auth")
            }),
            "the security addon must register the bearer scheme"
        );
    }
}
