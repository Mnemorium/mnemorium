/// The transport error mapped onto an HTTP status.
pub(crate) mod api_error;
/// The single router state every handler extracts.
pub(crate) mod app_state;
pub(crate) mod hal;
/// The route table and the per-context endpoint folders.
pub(crate) mod handler;
/// The authentication, tracing, rate-limit and HAL error layers.
pub(crate) mod middleware;

#[cfg(any(feature = "api-docs", test))]
use utoipa::OpenApi as _;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::OpenApi;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::security::Http;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::security::HttpAuthScheme;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::security::SecurityScheme;
#[cfg(any(feature = "api-docs", test))]
use utoipa_axum::router::OpenApiRouter;
#[cfg(any(feature = "api-docs", test))]
use utoipa_axum::routes;

#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::asset::asset_router;
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::get_health;
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::identity::identity_protected_router;
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::identity::identity_public_router;
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::library::library_router;
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::user::user_router;

/// Add the `bearer_auth` security scheme referenced by the protected
/// endpoints.
#[cfg(any(feature = "api-docs", test))]
struct SecurityAddon;

#[cfg(any(feature = "api-docs", test))]
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
#[cfg(any(feature = "api-docs", test))]
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
#[cfg(any(feature = "api-docs", test))]
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
    use super::*;
    use utoipa::openapi::RefOr;
    use utoipa::openapi::path::{Parameter, PathItem};

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

    #[test]
    fn openapi_collects_the_upload_cancel_operation() {
        // Act
        let document = openapi();
        let item = document.paths.paths.get("/asset/upload/{upload_id}");

        // Assert: cancelling a session is part of the declared surface, so a
        // client can discover it from the served specification.
        assert!(
            item.is_some_and(|path_item| path_item.delete.is_some()),
            "the document must declare DELETE /asset/upload/{{upload_id}}"
        );
    }

    /// Return whether the GET operation of `item` declares `name`.
    fn declares_parameter(item: Option<&PathItem>, name: &str) -> bool {
        item.and_then(|path_item| path_item.get.as_ref())
            .is_some_and(|operation| {
                operation.parameters.iter().flatten().any(|parameter| {
                    matches!(
                        parameter,
                        RefOr::T(Parameter { name: declared, .. }) if declared == name
                    )
                })
            })
    }

    #[test]
    fn openapi_collects_the_video_write_routes() {
        // Act
        let document = openapi();
        let paths = &document.paths.paths;

        // Assert: every create, replace and partial-update route of the video
        // catalogue is registered.
        assert!(
            paths.contains_key("/library/movie"),
            "the document must declare the film create route"
        );
        assert!(
            paths.contains_key("/library/movie/{movie_id}"),
            "the document must declare the film update route"
        );
        assert!(
            paths.contains_key("/library/series"),
            "the document must declare the show create route"
        );
        assert!(
            paths.contains_key("/library/series/{series_id}"),
            "the document must declare the show update route"
        );
        assert!(
            paths.contains_key("/library/series/{series_id}/season/{season_index}"),
            "the document must declare the season write route"
        );
        assert!(
            paths.contains_key(
                "/library/series/{series_id}/season/{season_index}/episode/{episode_index}"
            ),
            "the document must declare the episode write route"
        );
    }

    #[test]
    fn openapi_collects_the_video_apply_routes() {
        // Act
        let document = openapi();
        let paths = &document.paths.paths;

        // Assert
        assert!(
            paths.contains_key("/library/movie/{movie_id}/apply"),
            "the document must declare the film apply route"
        );
        assert!(
            paths.contains_key("/library/series/{series_id}/apply"),
            "the document must declare the show apply route"
        );
        assert!(
            paths.contains_key(
                "/library/series/{series_id}/season/{season_index}/episode/{episode_index}/apply"
            ),
            "the document must declare the episode apply route"
        );
    }

    #[test]
    fn openapi_collects_the_video_identification_routes() {
        // Act
        let document = openapi();
        let paths = &document.paths.paths;

        // Assert: each of the four routes is registered, and each carries the
        // parameters the handler declares.
        assert!(
            paths.contains_key("/library/movie/identify"),
            "the document must declare the film name route"
        );
        assert!(
            paths.contains_key("/library/movie/identify/{provider}/{reference}"),
            "the document must declare the film reference route"
        );
        assert!(
            paths.contains_key("/library/series/identify"),
            "the document must declare the show name route"
        );
        assert!(
            paths.contains_key("/library/series/identify/{provider}/{reference}"),
            "the document must declare the show reference route"
        );
        assert!(
            declares_parameter(paths.get("/library/movie/identify"), "name"),
            "the name identify route must declare its name query parameter"
        );
        let series_reference = paths.get("/library/series/identify/{provider}/{reference}");
        assert!(
            declares_parameter(series_reference, "season"),
            "the series reference route must declare the season position"
        );
        assert!(
            declares_parameter(series_reference, "episode"),
            "the series reference route must declare the episode position"
        );
    }
}
