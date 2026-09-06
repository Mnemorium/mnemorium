pub mod api_error;
pub mod app_state;
pub mod bootstrap;
pub mod handler;
pub mod middleware;

use handler::get_health::__path_get_health;
use handler::identity::post_login::__path_post_login;
use handler::identity::post_register::__path_post_register;

use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::handler::identity::post_login::LoginRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_login::LoginResponse;
use crate::infrastructure::inbound::rest::handler::identity::post_register::RegisterRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_register::RegisterResponse;

use utoipa::openapi::OpenApi;
use utoipa::openapi::security::Http;
use utoipa::openapi::security::HttpAuthScheme;
use utoipa::openapi::security::SecurityScheme;

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

/// Root `OpenAPI` aggregation for the Mnemorium HTTP API.
#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Mnemorium API",
        version = "0.1.0",
        description = "HTTP API of the Mnemorium service"
    ),
    servers(
        (url = "http://0.0.0.0:4080/api/v1", description = "Local development server")
    ),
    paths(get_health, post_login, post_register),
    components(schemas(ErrorBody, LoginRequest, LoginResponse, RegisterRequest, RegisterResponse, Role)),
    tags(
        (name = "system", description = "System-level endpoints"),
        (name = "identity", description = "Identity bounded context")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
