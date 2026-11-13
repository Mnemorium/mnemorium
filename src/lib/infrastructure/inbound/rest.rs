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

#[cfg(any(feature = "api-docs", test))]
use utoipa::OpenApi as _;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::OpenApi;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::RefOr;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::header::HeaderBuilder;
#[cfg(any(feature = "api-docs", test))]
use utoipa::openapi::path::{Operation, PathItem};
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
use crate::infrastructure::inbound::rest::handler::identity::{
    identity_protected_router, identity_public_router,
};
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::library::library_router;
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::user::user_router;
#[cfg(any(feature = "api-docs", test))]
use crate::infrastructure::inbound::rest::handler::{
    __path_get_api_index, __path_get_health, get_api_index, get_health,
};

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
#[cfg(any(feature = "api-docs", test))]
struct UnauthorizedChallengeAddon;

#[cfg(any(feature = "api-docs", test))]
impl utoipa::Modify for UnauthorizedChallengeAddon {
    fn modify(&self, openapi: &mut OpenApi) {
        let challenge = HeaderBuilder::new()
            .description(Some("Bearer challenge"))
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

/// Every operation a path item declares.
#[cfg(any(feature = "api-docs", test))]
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
#[cfg(any(feature = "api-docs", test))]
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
    openapi
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::openapi::path::Parameter;

    use std::error::Error;
    use std::sync::Arc;

    use axum::body::to_bytes;
    use axum::extract::State;
    use axum::http::header;
    use rstest::rstest;
    use serde_json::Value;

    use crate::application::port::{
        LoginUserUseCase, MockAssetUseCaseFactory, MockIdentityUseCaseFactory,
        MockLibraryUseCaseFactory, MockLoginUserUseCase, MockRegenerateTokenUseCase,
        MockUserUseCaseFactory, RegenerateTokenUseCase,
    };
    use crate::domain::model::Configuration;
    use crate::domain::port::IssuedToken;
    use crate::infrastructure::inbound::rest::handler::JsonObject;
    use crate::infrastructure::inbound::rest::handler::identity::{
        LoginRequest, LoginResponse, TokenResponse, handle_login, handle_token,
    };
    use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
    use crate::test_helpers::instance_state;

    /// An application state whose identity factory answers with `factory`.
    fn identity_state(factory: MockIdentityUseCaseFactory) -> Result<AppState, Box<dyn Error>> {
        Ok(AppState::new(
            Arc::new(MockAssetUseCaseFactory::new()),
            Arc::new(Configuration::default()),
            Arc::new(factory),
            Arc::new(instance_state()?),
            Arc::new(MockLibraryUseCaseFactory::new()),
            Arc::new(MockUserUseCaseFactory::new()),
        ))
    }

    #[tokio::test]
    async fn the_probe_is_a_hal_representation_of_the_operation() -> Result<(), Box<dyn Error>> {
        // Act
        let response = get_health().await;
        let content_type = response.headers()[header::CONTENT_TYPE].clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body: Value = serde_json::from_slice(&bytes)?;

        // Assert
        assert_eq!(content_type, HAL_CONTENT_TYPE);
        assert_eq!(body.get("status").and_then(Value::as_str), Some("ok"));
        assert_eq!(
            body.pointer("/_links/self/href").and_then(Value::as_str),
            Some("/health")
        );
        Ok(())
    }

    #[tokio::test]
    async fn login_is_a_hal_representation_of_the_operation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockLoginUserUseCase::new();
        use_case.expect_execute().times(1).returning(|_| {
            Box::pin(async { Ok(IssuedToken::new("secret-token".to_owned(), 3600)) })
        });
        let mut factory = MockIdentityUseCaseFactory::new();
        factory
            .expect_login_user()
            .times(1)
            .return_once(move || Arc::new(use_case) as Arc<dyn LoginUserUseCase>);
        let state = identity_state(factory)?;
        let request = JsonObject(LoginRequest {
            username: "alice".to_owned(),
            password: "secret".to_owned(),
        });
        let redacted = format!(
            "{:?}",
            LoginResponse::from(IssuedToken::new("secret-token".to_owned(), 3600))
        );

        // Act
        let response = handle_login(State(state), request)
            .await
            .map_err(|error| format!("the login handler failed: {error:?}"))?;
        let content_type = response.headers()[header::CONTENT_TYPE].clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body: Value = serde_json::from_slice(&bytes)?;

        // Assert
        assert_eq!(content_type, HAL_CONTENT_TYPE);
        assert_eq!(
            body.get("access_token").and_then(Value::as_str),
            Some("secret-token")
        );
        assert_eq!(
            body.get("expires_in").and_then(Value::as_u64),
            Some(3_600u64)
        );
        assert_eq!(
            body.get("token_type").and_then(Value::as_str),
            Some("bearer")
        );
        assert_eq!(
            body.pointer("/_links/self/href").and_then(Value::as_str),
            Some("/api/v1/identity/login")
        );
        assert!(
            !redacted.contains("secret-token"),
            "the token must stay redacted"
        );
        Ok(())
    }

    #[tokio::test]
    async fn regeneration_is_a_hal_representation_of_the_operation() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut use_case = MockRegenerateTokenUseCase::new();
        use_case.expect_execute().times(1).returning(|_| {
            Box::pin(async { Ok(IssuedToken::new("secret-token".to_owned(), 3600)) })
        });
        let mut factory = MockIdentityUseCaseFactory::new();
        factory
            .expect_regenerate_token()
            .times(1)
            .return_once(move || Arc::new(use_case) as Arc<dyn RegenerateTokenUseCase>);
        let state = identity_state(factory)?;
        let redacted = format!(
            "{:?}",
            TokenResponse::from(IssuedToken::new("secret-token".to_owned(), 3600))
        );

        // Act
        let response = handle_token(State(state), AuthenticatedUser::from(0))
            .await
            .map_err(|error| format!("the regeneration handler failed: {error:?}"))?;
        let content_type = response.headers()[header::CONTENT_TYPE].clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body: Value = serde_json::from_slice(&bytes)?;

        // Assert
        assert_eq!(content_type, HAL_CONTENT_TYPE);
        assert_eq!(
            body.get("access_token").and_then(Value::as_str),
            Some("secret-token")
        );
        assert_eq!(
            body.get("expires_in").and_then(Value::as_u64),
            Some(3_600u64)
        );
        assert_eq!(
            body.get("token_type").and_then(Value::as_str),
            Some("bearer")
        );
        assert_eq!(
            body.pointer("/_links/self/href").and_then(Value::as_str),
            Some("/api/v1/identity/token")
        );
        assert!(
            !redacted.contains("secret-token"),
            "the token must stay redacted"
        );
        Ok(())
    }

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
    fn openapi_collects_the_user_delete_operation() {
        // Act
        let document = openapi();
        let item = document.paths.paths.get("/user/{id}");

        // Assert: deleting an account is part of the declared surface, so a
        // client can discover it from the served specification.
        assert!(
            item.is_some_and(|path_item| path_item.delete.is_some()),
            "the document must declare DELETE /user/{{id}}"
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

    #[test]
    fn openapi_declares_the_bearer_challenge_on_every_unauthorized_response() {
        // Arrange
        let document = openapi();

        // Act & Assert: the addon reached every declared 401, including the
        // login response and a sampled guarded operation.
        let unauthorized = count_bearer_challenges(&document);
        assert!(
            unauthorized > 1usize,
            "the document must declare 401 responses"
        );
        let login = document
            .paths
            .paths
            .get("/identity/login")
            .and_then(|path_item| path_item.post.as_ref());
        assert!(
            declares_bearer_challenge(login),
            "the login 401 must declare the bearer challenge"
        );
        let me = document
            .paths
            .paths
            .get("/user/me")
            .and_then(|path_item| path_item.get.as_ref());
        assert!(
            declares_bearer_challenge(me),
            "a guarded operation's 401 must declare the bearer challenge"
        );
    }

    #[test]
    fn openapi_collects_the_public_index() {
        // Act
        let document = openapi();
        let index = document
            .paths
            .paths
            .get("/")
            .and_then(|path_item| path_item.get.as_ref());
        let declared = serde_json::to_value(index).unwrap_or(Value::Null);

        // Assert: the index is a public operation served as a HAL representation
        // at the document root, which resolves to the `/api/v1` server entry.
        assert!(index.is_some(), "the document must declare GET /");
        assert_eq!(
            declared.get("security"),
            Some(&serde_json::json!([{}])),
            "the index must declare no security requirement"
        );
        assert!(
            index.is_some_and(|operation| {
                operation
                    .responses
                    .responses
                    .get("200")
                    .is_some_and(|response| {
                        matches!(
                            response,
                            RefOr::T(declared_response)
                                if declared_response
                                    .content
                                    .contains_key("application/hal+json")
                        )
                    })
            }),
            "the index's 200 response must be application/hal+json"
        );
    }

    #[test]
    fn openapi_collects_the_token_regeneration_operation() {
        // Act
        let document = openapi();
        let declared = document
            .paths
            .paths
            .get("/identity/token")
            .and_then(|path_item| path_item.post.as_ref());

        // Assert: regeneration is part of the declared surface with the same
        // success and error responses as login.
        assert!(
            declared.is_some(),
            "the document must declare POST /identity/token"
        );
        assert!(
            declared.is_some_and(
                |operation| operation.responses.responses.contains_key("200")
                    && operation.responses.responses.contains_key("401")
                    && operation.responses.responses.contains_key("500")
            ),
            "regeneration must declare its 200, 401 and 500 responses"
        );
    }

    #[test]
    fn openapi_declares_no_logout_or_revocation_operation() {
        // Act
        let document = openapi();

        // Assert: tokens are stateless, so the surface offers neither logout
        // nor revocation.
        let offending = revocation_operation_ids(&document);
        assert!(
            offending.is_empty(),
            "the document must not declare {offending:?}"
        );
    }

    /// Assert every declared 401 carries the bearer challenge and return how
    /// many operations declare one.
    fn count_bearer_challenges(document: &OpenApi) -> usize {
        let declared: Vec<&Operation> = document
            .paths
            .paths
            .values()
            .flat_map(declared_operations)
            .filter(|operation| operation.responses.responses.contains_key("401"))
            .collect();
        for operation in &declared {
            assert!(
                declares_bearer_challenge(Some(operation)),
                "every declared 401 must carry the bearer challenge"
            );
        }
        declared.len()
    }

    /// Return the operation IDs naming a logout or revocation capability.
    fn revocation_operation_ids(document: &OpenApi) -> Vec<String> {
        const FORBIDDEN: [&str; 3] = ["logout", "revoke", "revocation"];

        let mut offending = Vec::new();
        for (path, path_item) in &document.paths.paths {
            for operation in declared_operations(path_item) {
                let operation_id = operation.operation_id.as_deref().unwrap_or_default();
                let lowered_id = operation_id.to_lowercase();
                let lowered_path = path.to_lowercase();
                if FORBIDDEN
                    .iter()
                    .any(|word| lowered_id.contains(word) || lowered_path.contains(word))
                {
                    offending.push(operation_id.to_owned());
                }
            }
        }
        offending
    }

    /// Return the operations `path_item` declares.
    fn declared_operations(path_item: &PathItem) -> impl Iterator<Item = &Operation> {
        [
            path_item.get.as_ref(),
            path_item.put.as_ref(),
            path_item.post.as_ref(),
            path_item.delete.as_ref(),
            path_item.options.as_ref(),
            path_item.head.as_ref(),
            path_item.patch.as_ref(),
            path_item.trace.as_ref(),
            path_item.query.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(path_item.additional_operations.values())
    }

    fn declares_bearer_challenge(operation: Option<&Operation>) -> bool {
        operation
            .and_then(|declared| declared.responses.responses.get("401"))
            .is_some_and(|declared| match declared.clone() {
                RefOr::T(challenge) => challenge.headers.contains_key("WWW-Authenticate"),
                RefOr::Ref(_) => false,
            })
    }

    #[rstest]
    #[case::file("/asset/file/{file_id}/content")]
    #[case::image("/asset/image/{image_id}/content")]
    #[case::audio("/asset/audio/{audio_id}/content")]
    #[case::video("/asset/video/{video_id}/content")]
    fn openapi_content_operations_declare_no_bad_request(#[case] path: &str) {
        // Act
        let document = openapi();
        let declared = document
            .paths
            .paths
            .get(path)
            .and_then(|item| item.get.as_ref());

        // Assert: the content routes accept only a bearer token, so no route
        // declares the ambiguous-credential refusal.
        assert!(declared.is_some(), "the document must declare GET {path}");
        assert!(
            !declared.is_some_and(|operation| operation.responses.responses.contains_key("400")),
            "{path} must not declare a 400"
        );
    }

    #[test]
    fn openapi_file_content_declares_the_disposition_header() {
        // Act
        let document = openapi();
        let operation = document
            .paths
            .paths
            .get("/asset/file/{file_id}/content")
            .and_then(|item| item.get.as_ref());

        // Assert
        assert!(
            operation.is_some(),
            "the document must declare the file content route"
        );
        let has_disposition = operation
            .and_then(|declared| declared.responses.responses.get("200"))
            .cloned()
            .is_some_and(|declared| {
                matches!(
                    declared,
                    RefOr::T(response) if response.headers.contains_key("Content-Disposition")
                )
            });
        assert!(
            has_disposition,
            "the save surface must declare its attachment name"
        );
    }
}
