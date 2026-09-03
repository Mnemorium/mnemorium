pub mod api_error;
pub mod bootstrap;
pub mod handler;

use handler::get_health::__path_get_health;

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
    paths(get_health),
    tags(
        (name = "system", description = "System-level endpoints")
    )
)]
pub struct ApiDoc;
