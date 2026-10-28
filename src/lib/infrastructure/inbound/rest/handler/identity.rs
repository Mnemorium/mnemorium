pub mod get_permission;
pub mod patch_credential;
pub mod post_login;
pub mod post_register;
pub mod put_permission;

use axum::Router;
use axum::middleware;
use axum::routing::patch;
use axum::routing::post;

use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::port::get_permission::GetPermissionResponse as GetPermissionResponseData;
use crate::application::port::replace_permission::PermissionRequest as PermissionRequestData;
use crate::domain::alias::NumericID;
use crate::domain::model::permission::Action;
use crate::domain::model::permission::ResourceKind;
use crate::domain::model::permission::ResourceRef;
use crate::domain::model::permission::Scope;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::handler::identity::patch_credential::patch_credential;
use crate::infrastructure::inbound::rest::handler::identity::post_login::post_login;
use crate::infrastructure::inbound::rest::handler::identity::post_register::post_register;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::inbound::rest::middleware::rate_limit::LoginRateLimiter;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// One named aggregate of a `LISTED` scope in a permission representation.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PermissionResource {
    /// Identifier of the aggregate.
    pub id: NumericID,
    /// Kind of the aggregate (`GALLERY`, `MUSIC_PLAYLIST`, ...).
    pub kind: String,
}

/// One action's scope in a permission representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PermissionGrantResponse {
    /// Stored name of the action (`playlist.read`).
    pub action: String,
    /// Aggregates a `LISTED` scope names; empty otherwise.
    pub resources: Vec<PermissionResource>,
    /// The scope the action grants (`ALL`, `LISTED`, ...).
    pub scope: String,
}

/// Effective permissions of one user.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct PermissionResponse {
    /// Role presets per configurable action.
    pub defaults: Vec<PermissionGrantResponse>,
    /// Link to the permission resource itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Stored overrides per action.
    pub overrides: Vec<PermissionGrantResponse>,
    /// Role the presets are read from.
    pub role: Role,
    /// Identifier of the user the permissions apply to.
    pub user_id: NumericID,
}

/// One requested override.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PermissionOverrideRequest {
    /// Action the override names (`playlist.read`).
    pub action: String,
    /// Aggregates a `LISTED` scope names; empty otherwise.
    #[serde(default)]
    pub resources: Vec<PermissionResource>,
    /// Requested scope (`ALL`, `PUBLIC_OR_OWNED`, `OWNED`, `LISTED`, `NONE`).
    pub scope: String,
}

/// Complete override set a user must end with.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct PermissionRequest {
    /// Every override the user must hold.
    pub permissions: Vec<PermissionOverrideRequest>,
}

/// Map the permissions onto their HTTP representation.
impl From<GetPermissionResponseData> for PermissionResponse {
    fn from(response: GetPermissionResponseData) -> Self {
        let user_id = response.user_id();
        Self {
            defaults: response
                .defaults()
                .iter()
                .map(|preset| grant_response(preset.action(), preset.scope()))
                .collect(),
            overrides: response
                .overrides()
                .iter()
                .map(|permission| grant_response(permission.action(), permission.scope()))
                .collect(),
            role: response.role(),
            user_id,
            links: SelfLinks::new(&format!("/api/v1/user/{user_id}/permission")),
        }
    }
}

/// Validate one requested override and map it onto the application request.
impl TryFrom<PermissionOverrideRequest> for PermissionRequestData {
    type Error = ApiError;

    fn try_from(request: PermissionOverrideRequest) -> Result<Self, Self::Error> {
        let action =
            Action::from_wire(&request.action).ok_or_else(|| ApiError::reject_enum("action"))?;
        let stored_scope =
            Scope::from_wire(&request.scope).ok_or_else(|| ApiError::reject_enum("scope"))?;
        let resolved_scope = match stored_scope {
            Scope::Listed(_) => {
                let references = request
                    .resources
                    .iter()
                    .map(|resource| {
                        let kind = ResourceKind::from_wire(&resource.kind)
                            .ok_or_else(|| ApiError::reject_enum("resource.kind"))?;
                        Ok(ResourceRef::new(kind, resource.id))
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?;
                Scope::Listed(references)
            }
            Scope::All | Scope::None | Scope::Owned | Scope::PublicOrOwned => {
                if request.resources.is_empty() {
                    stored_scope
                } else {
                    return Err(ApiError::UnprocessableEntity(
                        "a non-listed scope must not name resources".to_owned(),
                    ));
                }
            }
        };
        Ok(Self::new(action, resolved_scope))
    }
}

/// Map an action and its scope onto their HTTP representation.
#[expect(
    clippy::pattern_type_mismatch,
    reason = "the scope owns its listed resources, so it is matched through a reference"
)]
fn grant_response(action: Action, scope: &Scope) -> PermissionGrantResponse {
    let resources = match scope {
        Scope::Listed(references) => references
            .iter()
            .map(|reference| PermissionResource {
                id: reference.id(),
                kind: reference.kind().as_wire().to_owned(),
            })
            .collect(),
        Scope::All | Scope::None | Scope::Owned | Scope::PublicOrOwned => Vec::new(),
    };
    PermissionGrantResponse {
        action: action.as_wire().to_owned(),
        resources,
        scope: scope.as_wire().to_owned(),
    }
}

/// Routes of the identity bounded context.
///
/// `login` is public; `register` and the credential password change require an
/// authenticated caller. The public `login` route is rate limited per client
/// address by the `limiter` the composition root built; the other routes are
/// not.
pub fn identity_routes(state: &AppState, limiter: &LoginRateLimiter) -> Router {
    let register: Router<AppState> = Router::new().route("/register", post(post_register));

    let patch_credential: Router<AppState> =
        Router::new().route("/credential/{id}", patch(patch_credential));

    let login: Router<AppState> = Router::new().route("/login", post(post_login));

    let rate_limited_login = limiter.apply(login);

    register
        .merge(patch_credential)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .merge(rate_limited_login)
        .with_state(state.clone())
}
