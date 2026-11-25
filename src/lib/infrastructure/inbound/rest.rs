/// The transport error mapped onto an HTTP status.
mod api_error;
/// The single router state every handler extracts.
mod app_state;
mod hal;
/// The route table and the per-context endpoint folders.
pub(crate) mod handler;
/// The authentication, tracing, rate-limit and HAL error layers.
pub(crate) mod middleware;

pub(crate) use self::api_error::{ApiError, ApiErrorMessage, ErrorBody, error_response};
pub(crate) use self::app_state::AppState;
pub(crate) use self::hal::{HAL_CONTENT_TYPE, Link, SelfLinks, hal_json};

use std::borrow::Cow;
use utoipa::openapi::RefOr;
use utoipa::openapi::schema::{KnownFormat, ObjectBuilder, Schema, SchemaFormat, Type};

#[cfg(feature = "api-docs")]
use utoipa::OpenApi as _;
#[cfg(feature = "api-docs")]
use utoipa::openapi::OpenApi;
#[cfg(feature = "api-docs")]
use utoipa::openapi::header::HeaderBuilder;
#[cfg(feature = "api-docs")]
use utoipa::openapi::path::{Operation, PathItem};
#[cfg(feature = "api-docs")]
use utoipa::openapi::response::ResponseBuilder;
#[cfg(feature = "api-docs")]
use utoipa::openapi::security::Http;
#[cfg(feature = "api-docs")]
use utoipa::openapi::security::HttpAuthScheme;
#[cfg(feature = "api-docs")]
use utoipa::openapi::security::SecurityScheme;
use utoipa::{PartialSchema, ToSchema};
#[cfg(feature = "api-docs")]
use utoipa_axum::router::OpenApiRouter;
#[cfg(feature = "api-docs")]
use utoipa_axum::routes;

#[cfg(feature = "api-docs")]
use crate::infrastructure::inbound::rest::handler::asset::asset_router;
#[cfg(feature = "api-docs")]
use crate::infrastructure::inbound::rest::handler::identity::{
    identity_protected_router, identity_public_router,
};
#[cfg(feature = "api-docs")]
use crate::infrastructure::inbound::rest::handler::library::library_router;
#[cfg(feature = "api-docs")]
use crate::infrastructure::inbound::rest::handler::user::user_router;
#[cfg(feature = "api-docs")]
use crate::infrastructure::inbound::rest::handler::{
    __path_get_api_index, __path_get_health, get_api_index, get_health,
};

/// A relative URI reference published as a response header value (`REQ-API-019`).
///
/// `utoipa` collects component schemas from response media types only, so a
/// named type in a `headers(...)` tuple is published with `inline(...)` and
/// never leaves a `$ref` that does not reach `components.schemas`.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct UriReference;

impl PartialSchema for UriReference {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .format(Some(SchemaFormat::Custom("uri-reference".to_owned())))
            .description(Some("A URI reference.".to_owned()))
            .into()
    }
}

impl ToSchema for UriReference {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("UriReference")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// An integer `Content-Length` (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct ContentLength;

impl PartialSchema for ContentLength {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::Integer)
            .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int64)))
            .description(Some("Length of the served span, in bytes.".to_owned()))
            .into()
    }
}

impl ToSchema for ContentLength {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("ContentLength")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// An HTTP-date `Last-Modified` (`REQ-API-019`).
///
/// The value is an IMF-fixdate, not an RFC 3339 timestamp: the header
/// validator rejects a date-time value against this header.
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct HttpDate;

impl PartialSchema for HttpDate {
    fn schema() -> RefOr<Schema> {
        patterned_string(
            "An HTTP-date in IMF-fixdate form.",
            "^[A-Z][a-z]{2}, [0-9]{2} [A-Z][a-z]{2} [0-9]{4} [0-9]{2}:[0-9]{2}:[0-9]{2} GMT$",
        )
    }
}

impl ToSchema for HttpDate {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("HttpDate")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// An `ETag` validator (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct EntityTag;

impl PartialSchema for EntityTag {
    fn schema() -> RefOr<Schema> {
        patterned_string("A quoted opaque validator.", "^\"[^\"]+\"$")
    }
}

impl ToSchema for EntityTag {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("EntityTag")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// A `Content-Range` value (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct ContentRangeHeader;

impl PartialSchema for ContentRangeHeader {
    fn schema() -> RefOr<Schema> {
        patterned_string(
            "A served span and total size, or an unsatisfiable total.",
            "^bytes (?:[0-9]+-[0-9]+|\\*)/[0-9]+$",
        )
    }
}

impl ToSchema for ContentRangeHeader {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("ContentRangeHeader")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// An `Allow` value (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct AllowMethods;

impl PartialSchema for AllowMethods {
    fn schema() -> RefOr<Schema> {
        patterned_string("The methods the path accepts.", "^[A-Z]+(?:, ?[A-Z]+)*$")
    }
}

impl ToSchema for AllowMethods {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("AllowMethods")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// An `Accept-Ranges` value (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct AcceptRangesHeader;

impl PartialSchema for AcceptRangesHeader {
    fn schema() -> RefOr<Schema> {
        patterned_string("Always `bytes`.", "^bytes$")
    }
}

impl ToSchema for AcceptRangesHeader {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("AcceptRangesHeader")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// A `Cache-Control` value (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct CacheControlHeader;

impl PartialSchema for CacheControlHeader {
    fn schema() -> RefOr<Schema> {
        patterned_string(
            "A private, bounded cache lifetime.",
            "^private, max-age=[0-9]+$",
        )
    }
}

impl ToSchema for CacheControlHeader {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("CacheControlHeader")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// A `Content-Disposition` value (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct ContentDispositionHeader;

impl PartialSchema for ContentDispositionHeader {
    fn schema() -> RefOr<Schema> {
        patterned_string(
            "An attachment naming the stored file, with an RFC 5987 fallback when needed.",
            "^attachment; filename=\"[^\"]*\"(?:; filename\\*=UTF-8''[A-Za-z0-9%._~-]+)?$",
        )
    }
}

impl ToSchema for ContentDispositionHeader {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("ContentDispositionHeader")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// A `WWW-Authenticate` bearer challenge (`REQ-API-019`).
#[derive(Debug)]
#[non_exhaustive]
#[cfg(feature = "api-docs")]
pub(crate) struct BearerChallenge;

#[cfg(feature = "api-docs")]
impl PartialSchema for BearerChallenge {
    fn schema() -> RefOr<Schema> {
        patterned_string("A bearer challenge.", "^Bearer$")
    }
}

#[cfg(feature = "api-docs")]
impl ToSchema for BearerChallenge {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("BearerChallenge")
    }

    fn schemas(_schemas: &mut Vec<(String, RefOr<Schema>)>) {}
}

/// Add the `bearer_auth` security scheme referenced by the protected
/// endpoints.
#[cfg(feature = "api-docs")]
struct SecurityAddon;

#[cfg(feature = "api-docs")]
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

/// Declare the `WWW-Authenticate: Bearer` challenge on every `401` response.
///
/// `ApiError::into_response` attaches the header to every `401` the server
/// emits, so the document declares it once here instead of repeating the
/// header on each of the 82 handler paths. Every operation that can answer
/// `401` — each `authenticate`-guarded route and `POST /identity/login` —
/// declares that response, and no unguarded operation declares one.
///
/// The addon runs after `utoipa-axum` merges the handler paths: the
/// document-scoped modifiers in [`ApiDoc`] run before any route is registered,
/// so they would see an empty path map.
#[cfg(feature = "api-docs")]
struct UnauthorizedChallengeAddon;

#[cfg(feature = "api-docs")]
impl utoipa::Modify for UnauthorizedChallengeAddon {
    fn modify(&self, openapi: &mut OpenApi) {
        let challenge = HeaderBuilder::new()
            .description(Some("Bearer challenge"))
            .schema(Some(<BearerChallenge as PartialSchema>::schema()))
            .build();
        for path_item in openapi.paths.paths.values_mut() {
            for operation in operations_mut(path_item) {
                if let Some(&mut RefOr::T(ref mut response)) =
                    operation.responses.responses.get_mut("401")
                {
                    response
                        .headers
                        .insert("WWW-Authenticate".to_owned(), RefOr::T(challenge.clone()));
                }
            }
        }
    }
}

/// Declare the transport-level `414` every request can receive.
///
/// The HTTP/1 parser refuses a request line longer than its 64 KiB buffer
/// before the router runs, so no handler can answer it and the response has no
/// body. The document declares it once for every operation rather than
/// repeating it in each `responses(...)` block.
#[cfg(feature = "api-docs")]
struct UriTooLongAddon;

#[cfg(feature = "api-docs")]
impl utoipa::Modify for UriTooLongAddon {
    fn modify(&self, openapi: &mut OpenApi) {
        let response = RefOr::T(
            ResponseBuilder::new()
                .description("The request URI exceeds the transport's 64 KiB limit")
                .build(),
        );
        for path_item in openapi.paths.paths.values_mut() {
            for operation in operations_mut(path_item) {
                operation
                    .responses
                    .responses
                    .entry("414".to_owned())
                    .or_insert_with(|| response.clone());
            }
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
#[cfg(feature = "api-docs")]
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
/// [`handler::setup_routes`] serves this document as HTML at `/doc` and as
/// JSON at `/doc/openapi.json`.
#[must_use]
#[cfg(feature = "api-docs")]
pub(crate) fn openapi() -> OpenApi {
    let (_, mut openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_health))
        .routes(routes!(get_api_index))
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
    utoipa::Modify::modify(&UnauthorizedChallengeAddon, &mut openapi);
    utoipa::Modify::modify(&UriTooLongAddon, &mut openapi);
    openapi
}

/// Every operation a path item declares.
#[cfg(feature = "api-docs")]
fn operations_mut(path_item: &mut PathItem) -> impl Iterator<Item = &mut Operation> {
    [
        path_item.get.as_mut(),
        path_item.put.as_mut(),
        path_item.post.as_mut(),
        path_item.delete.as_mut(),
        path_item.options.as_mut(),
        path_item.head.as_mut(),
        path_item.patch.as_mut(),
        path_item.trace.as_mut(),
        path_item.query.as_mut(),
    ]
    .into_iter()
    .flatten()
    .chain(path_item.additional_operations.values_mut())
}

/// Build a string schema with a pattern, for a response header whose value
/// obeys a shape (`REQ-API-019`).
fn patterned_string(description: &str, pattern: &str) -> RefOr<Schema> {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .pattern(Some(pattern.to_owned()))
        .description(Some(description.to_owned()))
        .into()
}
