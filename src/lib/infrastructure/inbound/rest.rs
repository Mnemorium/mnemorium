pub mod api_error;
pub mod app_state;
pub mod handler;
pub mod middleware;

use handler::asset::post_asset::__path_post_asset;
use handler::asset::post_asset_chunk::__path_post_asset_chunk;
use handler::asset::post_asset_finish::__path_post_asset_finish;
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
use crate::infrastructure::inbound::rest::handler::asset::post_asset::PostAssetRequest;
use crate::infrastructure::inbound::rest::handler::asset::post_asset::PostAssetResponse;
use crate::infrastructure::inbound::rest::handler::asset::post_asset_chunk::PostAssetChunkRequest;
use crate::infrastructure::inbound::rest::handler::asset::post_asset_finish::PostAssetFinishResponse;
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
    paths(get_health, post_asset, post_asset_chunk, post_asset_finish, patch_credential, post_login, post_register, get_me, get_user, get_user_list, patch_user),
    components(schemas(ErrorBody, GetMeResponse, GetUserResponse, LoginRequest, LoginResponse, PatchCredentialRequest, PatchUserRequest, PatchUserResponse, PostAssetChunkRequest, PostAssetFinishResponse, PostAssetRequest, PostAssetResponse, RegisterRequest, RegisterResponse, Role)),
    tags(
        (name = "system", description = "System-level endpoints"),
        (name = "asset", description = "Asset bounded context"),
        (name = "identity", description = "Identity bounded context"),
        (name = "user", description = "User bounded context")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::ApiDoc;
    use utoipa::OpenApi as _;
    use utoipa::openapi::security::SecurityRequirement;

    /// Assert that every operation in the aggregated specification declares
    /// `security` explicitly.
    ///
    /// The endpoint handler contract (`API-011`) requires every handler to
    /// declare the scheme(s) protecting it, or an empty requirement when the
    /// endpoint is public. An absent declaration is indistinguishable from a
    /// forgotten one, which is how `/health` and `/identity/login` drifted
    /// before issue #18.
    #[test]
    fn every_operation_declares_security_explicitly() {
        let openapi = ApiDoc::openapi();
        for (path, item) in &openapi.paths.paths {
            for (method, operation) in [
                ("GET", item.get.as_ref()),
                ("PUT", item.put.as_ref()),
                ("POST", item.post.as_ref()),
                ("DELETE", item.delete.as_ref()),
                ("OPTIONS", item.options.as_ref()),
                ("HEAD", item.head.as_ref()),
                ("PATCH", item.patch.as_ref()),
                ("TRACE", item.trace.as_ref()),
            ] {
                let Some(declared) = operation else {
                    continue;
                };
                assert!(
                    declared.security.is_some(),
                    "{method} {path} must declare an explicit `security` requirement"
                );
            }
        }
    }

    /// Assert that the two public endpoints carry the empty security
    /// requirement, so a regression to an absent declaration or to
    /// `bearer_auth` fails the build.
    #[test]
    fn public_operations_declare_empty_security_requirement() {
        let openapi = ApiDoc::openapi();
        let empty = vec![SecurityRequirement::default()];

        let health = openapi
            .paths
            .get_path_item("/health")
            .and_then(|item| item.get.as_ref())
            .and_then(|operation| operation.security.as_ref());
        assert!(health == Some(&empty), "/health must be public");

        let login = openapi
            .paths
            .get_path_item("/identity/login")
            .and_then(|item| item.post.as_ref())
            .and_then(|operation| operation.security.as_ref());
        assert!(login == Some(&empty), "/identity/login must be public");
    }
}
