#![allow(
    clippy::missing_errors_doc,
    reason = "Handler are declared in the OpenAPI spec"
)]

/// The Asset context endpoints.
pub(crate) mod asset;
/// The liveness endpoint.
mod get_health;
/// The Identity context endpoints.
pub(crate) mod identity;
/// The Library context endpoints.
pub(crate) mod library;
/// The User context endpoints.
pub(crate) mod user;

#[cfg(any(feature = "api-docs", test))]
pub(crate) use self::get_health::{__path_get_health, get_health};

use axum::extract::{FromRequest, Request};
use axum::middleware;
use axum::response::Response;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Number, Value};
use tracing::warn;

use axum::routing;
#[cfg(feature = "api-docs")]
use utoipa_scalar::Servable as _;

use crate::infrastructure::inbound::rest::handler::asset::asset_routes;
use crate::infrastructure::inbound::rest::handler::identity::identity_routes;
use crate::infrastructure::inbound::rest::handler::library::library_routes;
use crate::infrastructure::inbound::rest::handler::user::user_routes;
use crate::infrastructure::inbound::rest::middleware::{LoginRateLimiter, hal_errors, tracing};
use crate::infrastructure::inbound::rest::{ApiError, AppState, Link, SelfLinks, hal_json};

/// HAL links exposed by the API index.
///
/// The `self` link addresses the index itself; every other relation reaches one
/// context's entry collection or operation. Item routes are reached from a
/// representation, so the index never enumerates them. The `credential`
/// operation is published as a URI template because its identifier is part of
/// the path the caller must fill.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApiIndexLinks {
    /// Templated link to the credential operation.
    pub credential: Link,
    /// Link to the current caller's account.
    pub current_user: Link,
    /// Link to the gallery collection.
    pub gallery: Link,
    /// Link to the liveness probe.
    pub health: Link,
    /// Link to the login operation.
    pub login: Link,
    /// Link to the film collection.
    pub movie: Link,
    /// Link to the music-album collection.
    pub music_album: Link,
    /// Link to the music-group collection.
    pub music_group: Link,
    /// Link to the caller's permission summary.
    pub permission: Link,
    /// Link to the playlist collection.
    pub playlist: Link,
    /// Link to the registration operation.
    pub register: Link,
    /// Link to the index itself.
    #[serde(rename = "self")]
    pub self_link: Link,
    /// Link to the show collection.
    pub series: Link,
    /// Link to the token-regeneration operation.
    pub token: Link,
    /// Link to the upload operation.
    pub upload: Link,
    /// Link to the user collection.
    pub user: Link,
}

/// The public entry point of the API: the one URI a client must know to reach
/// the surface.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ApiIndexResponse {
    /// Entry points of the authentication, asset, library and user contexts.
    #[serde(rename = "_links")]
    pub links: ApiIndexLinks,
}

/// A JSON request body that must be a JSON object.
///
/// JSON Schema describes every body DTO as an object, so a sequence is not a
/// valid representation (serde would otherwise accept one as a positional
/// struct). JSON Schema also treats a whole-number float such as `1.0` as an
/// integer, while serde rejects it for an integer field; the extractor
/// normalises those floats before deserializing.
#[expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "handlers destructure the crate-private tuple; no public surface is exposed"
)]
pub(crate) struct JsonObject<T>(pub(crate) T);

impl<T, S> FromRequest<S> for JsonObject<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let axum::Json(value) = axum::Json::<Value>::from_request(req, state)
            .await
            .map_err(ApiError::from)?;
        let normalised = normalise_numbers(value);
        if !normalised.is_object() {
            warn!(
                target: "security",
                event = "input_validation_failed",
                field = "body",
                reason = "not_object",
                "rejected a request body"
            );
            return Err(ApiError::UnprocessableEntity(
                "the request body must be a JSON object".to_owned(),
            ));
        }
        serde_json::from_value(normalised).map(Self).map_err(|_| {
            warn!(
                target: "security",
                event = "deserialization_failed",
                source = "json_body",
                reason = "schema_mismatch",
                "rejected a request body"
            );
            ApiError::UnprocessableEntity(
                "the request body does not match the expected schema".to_owned(),
            )
        })
    }
}

/// Read the public API index.
///
/// The index is public and identical for every caller, so a client that knows
/// only the API base can reach every context by following its links.
#[utoipa::path(
    get,
    operation_id = "get_api_index",
    path = "/",
    tag = "system",
    responses(
        (
            status = OK,
            body = ApiIndexResponse,
            content_type = "application/hal+json",
            description = "Entry points of the API",
        ),
    ),
    security(()),
    summary = "Read the API index"
)]
pub(crate) async fn get_api_index() -> Response {
    hal_json(ApiIndexResponse {
        links: ApiIndexLinks {
            login: Link::new("/api/v1/identity/login"),
            register: Link::new("/api/v1/identity/register"),
            token: Link::new("/api/v1/identity/token"),
            credential: Link::templated("/api/v1/identity/credential/{id}"),
            user: Link::new("/api/v1/user"),
            current_user: Link::new("/api/v1/user/me"),
            gallery: Link::new("/api/v1/library/gallery"),
            permission: Link::new("/api/v1/identity/permission"),
            playlist: Link::new("/api/v1/library/music/playlist"),
            movie: Link::new("/api/v1/library/movie"),
            series: Link::new("/api/v1/library/series"),
            music_group: Link::new("/api/v1/library/music/group"),
            music_album: Link::new("/api/v1/library/music/album"),
            upload: Link::new("/api/v1/asset/upload"),
            health: Link::new("/health"),
            self_link: SelfLinks::new("/api/v1").self_link,
        },
    })
}

/// Convert whole-number floats into integers, recursively.
///
/// The conversion only applies when the value is finite, integral and inside
/// the target range, so a genuinely fractional or out-of-range value still
/// fails deserialization.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is finite, integral and range-checked before the cast, so neither truncation nor sign loss can occur"
)]
fn normalise_numbers(value: Value) -> Value {
    match value {
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                return Value::Number(number);
            }
            let Some(float) = number.as_f64() else {
                return Value::Number(number);
            };
            if !float.is_finite() || float.fract() != 0.0 {
                return Value::Number(number);
            }
            // Strict bounds: an integer literal just outside `i64` is parsed as
            // `f64` and rounds to -2^63 or 2^63. Converting that boundary value
            // would turn an out-of-range integer into a valid one, so leave it
            // for serde to reject. An integral `f64` inside these bounds is
            // exactly representable as its target integer.
            let i64_bound = 2.0f64.powi(63);
            if float > -i64_bound && float < i64_bound {
                return Value::Number(Number::from(float as i64));
            }
            // 2^64 is the exclusive upper bound.
            let u64_bound = 2.0f64.powi(64);
            if float > 0.0 && float < u64_bound {
                return Value::Number(Number::from(float as u64));
            }
            Value::Number(number)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(normalise_numbers).collect()),
        Value::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, nested)| (key, normalise_numbers(nested)))
                .collect(),
        ),
        other @ (Value::Null | Value::Bool(_) | Value::String(_)) => other,
    }
}

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
/// With the `api-docs` feature the documentation is mounted outside the
/// `/api/v1` API base: the interactive reference at `/doc` and the document
/// `rest::openapi` assembles, as JSON, at `/doc/openapi.json`.
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
                .routes(routes!(get_api_index))
                .nest("/asset", asset_routes(state))
                .nest("/identity", identity_routes(state, limiter))
                .nest("/library", library_routes(state))
                .nest("/user", user_routes(state)),
        )
        // The document declares the index at `/`, which resolves to the
        // `/api/v1/` form of the server entry. Serve that trailing-slash form
        // too, so a client following the document reaches the index.
        .route("/api/v1/", routing::get(get_api_index));

    #[cfg(feature = "api-docs")]
    {
        use crate::infrastructure::inbound::rest::openapi;
        let document = openapi();
        let json_document = document.clone();
        let documentation: axum::Router = utoipa_scalar::Scalar::with_url("/doc", document).into();
        let specification: axum::Router = axum::Router::new().route(
            "/doc/openapi.json",
            routing::get(move || {
                let response_document = json_document.clone();
                async move { axum::Json(response_document) }
            }),
        );
        api = api.merge(OpenApiRouter::from(documentation.merge(specification)));
    }

    api.layer(middleware::from_fn(tracing))
        .layer(middleware::from_fn(hal_errors))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde::Deserialize;
    use std::error::Error;

    use axum::body::Body as HttpBody;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt as _;

    use crate::infrastructure::inbound::rest::middleware::LoginRateLimiter;
    use crate::test_helpers::app_state;

    /// A body whose integer fields must accept the schema's whole-number floats.
    #[derive(Debug, Deserialize, PartialEq)]
    struct Body {
        id: i64,
        nested: Vec<u64>,
    }

    #[rstest]
    #[case::no_trailing_slash("/api/v1")]
    #[case::trailing_slash("/api/v1/")]
    #[tokio::test]
    async fn the_api_base_serves_the_index(#[case] uri: &str) -> Result<(), Box<dyn Error>> {
        // Arrange
        let state = app_state()?;
        let (limiter, _cleanup) =
            LoginRateLimiter::new(state.configuration().security().rate_limit());

        // Act
        let request = Request::builder().uri(uri).body(HttpBody::empty())?;
        let response = setup_routes(&state, &limiter).oneshot(request).await?;

        // Assert: the index is registered under the versioned base, so a
        // client that knows only `/api/v1` reaches it with or without the
        // trailing slash the document's root path implies.
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{uri} must serve the index"
        );
        Ok(())
    }

    #[test]
    fn whole_number_floats_become_integers() -> Result<(), Box<dyn Error>> {
        // Arrange
        let value: Value = serde_json::from_str(
            r#"{"id":-1435657098873592.0,"nested":[1.0,9223372036854775808.0]}"#,
        )?;
        // Act
        let body: Body = serde_json::from_value(normalise_numbers(value))?;
        // Assert
        assert_eq!(body.id, -1_435_657_098_873_592);
        assert_eq!(body.nested, [1, 9_223_372_036_854_775_808]);
        Ok(())
    }

    #[test]
    fn a_whole_float_is_read_exactly_before_normalisation() -> Result<(), Box<dyn Error>> {
        // Arrange
        // `2251799813685247.0` is exactly representable, but without the
        // `float_roundtrip` feature the JSON parser reads it as
        // `2251799813685247.25`, so the normaliser sees a fraction and serde
        // rejects the integer field.
        let value: Value = serde_json::from_str(r#"{"id":2251799813685247.0,"nested":[]}"#)?;
        // Act
        let body: Body = serde_json::from_value(normalise_numbers(value))?;
        // Assert
        assert_eq!(body.id, 2_251_799_813_685_247);
        Ok(())
    }

    #[test]
    fn fractional_floats_are_left_for_serde_to_reject() -> Result<(), Box<dyn Error>> {
        // Arrange
        let value: Value = serde_json::from_str(r#"{"id":1.5,"nested":[]}"#)?;
        // Act
        let result = serde_json::from_value::<Body>(normalise_numbers(value));
        // Assert
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn an_integer_outside_i64_stays_a_float() -> Result<(), Box<dyn Error>> {
        // Arrange
        let value: Value = serde_json::from_str(r#"{"id":-9223372036854775809,"nested":[]}"#)?;
        // Act
        let result = serde_json::from_value::<Body>(normalise_numbers(value));
        // Assert
        assert!(result.is_err());
        Ok(())
    }
}
