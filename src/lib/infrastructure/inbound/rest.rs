pub mod api_error;
pub mod app_state;
pub mod hal;
pub mod handler;
pub mod middleware;

use handler::asset::get_upload::__path_get_upload;
use handler::asset::post_upload::__path_post_upload;
use handler::asset::post_upload_complete::__path_post_upload_complete;
use handler::asset::put_upload_chunk::__path_put_upload_chunk;
use handler::get_health::__path_get_health;
use handler::identity::patch_credential::__path_patch_credential;
use handler::identity::post_login::__path_post_login;
use handler::identity::post_register::__path_post_register;
use handler::user::get_me::__path_get_me;
use handler::user::get_user::__path_get_user;
use handler::user::get_user_list::__path_get_user_list;
use handler::user::patch_user::__path_patch_user;

use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::hal::Link;
use crate::infrastructure::inbound::rest::handler::asset::get_upload::GetUploadResponse;
use crate::infrastructure::inbound::rest::handler::asset::links::UploadSessionLinks;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::PostUploadRequest;
use crate::infrastructure::inbound::rest::handler::asset::post_upload::PostUploadResponse;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_complete::PostUploadCompleteRequest;
use crate::infrastructure::inbound::rest::handler::asset::post_upload_complete::PostUploadCompleteResponse;
use crate::infrastructure::inbound::rest::handler::asset::put_upload_chunk::PutUploadChunkRequest;
use crate::infrastructure::inbound::rest::handler::asset::put_upload_chunk::PutUploadChunkResponse;
use crate::infrastructure::inbound::rest::handler::identity::patch_credential::PatchCredentialRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_login::LoginRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_login::LoginResponse;
use crate::infrastructure::inbound::rest::handler::identity::post_register::RegisterRequest;
use crate::infrastructure::inbound::rest::handler::identity::post_register::RegisterResponse;
use crate::infrastructure::inbound::rest::handler::user::get_me::GetMeResponse;
use crate::infrastructure::inbound::rest::handler::user::get_user::GetUserResponse;
use crate::infrastructure::inbound::rest::handler::user::patch_user::PatchUserRequest;
use crate::infrastructure::inbound::rest::handler::user::patch_user::PatchUserResponse;

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
        version = env!("CARGO_PKG_VERSION"),
        description = "HTTP API of the Mnemorium service"
    ),
    servers(
        (url = "http://0.0.0.0:4080/api/v1", description = "Local development server")
    ),
    paths(get_health, get_upload, patch_credential, post_login, post_register, post_upload, post_upload_complete, put_upload_chunk, get_me, get_user, get_user_list, patch_user),
    components(schemas(ErrorBody, GetMeResponse, GetUploadResponse, GetUserResponse, Link, LoginRequest, LoginResponse, PatchCredentialRequest, PatchUserRequest, PatchUserResponse, PostUploadCompleteRequest, PostUploadCompleteResponse, PostUploadRequest, PostUploadResponse, PutUploadChunkRequest, PutUploadChunkResponse, RegisterRequest, RegisterResponse, Role, UploadSessionLinks)),
    tags(
        (name = "system", description = "System-level endpoints"),
        (name = "asset", description = "Asset bounded context"),
        (name = "identity", description = "Identity bounded context"),
        (name = "user", description = "User bounded context")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;
