pub mod get_me;
pub mod get_user;
pub mod get_user_list;
pub mod patch_user;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use axum::routing::patch;
use serde::Serialize;
use tracing::warn;
use utoipa::ToSchema;

use crate::application::port::get_user::GetUserResponse as GetUserResponseData;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::handler::identity::get_permission::get_permission;
use crate::infrastructure::inbound::rest::handler::identity::put_permission::put_permission;
use crate::infrastructure::inbound::rest::handler::user::get_me::get_me;
use crate::infrastructure::inbound::rest::handler::user::get_user::get_user;
use crate::infrastructure::inbound::rest::handler::user::get_user_list::get_user_list;
use crate::infrastructure::inbound::rest::handler::user::patch_user::patch_user;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// User returned by a successful lookup.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub struct GetUserResponse {
    /// Email address of the user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the user.
    pub id: NumericID,
    /// Link to the user resource itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Role of the user.
    pub role: Role,
    /// Username of the user.
    pub username: String,
}

/// Map the fetched-user response onto its HTTP representation.
impl From<GetUserResponseData> for GetUserResponse {
    fn from(response: GetUserResponseData) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            links: SelfLinks::new(&user_self_href(response.id())),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Canonical URI reference of the user resource identified by `id`.
///
/// Shared by the user representations and the registration response so the
/// `_links.self` target and the `Location` header cannot drift (`API-032`).
#[must_use]
pub(crate) fn user_self_href(id: NumericID) -> String {
    format!("/api/v1/user/{id}")
}

/// Parse a user identifier from a path segment.
pub(crate) fn parse_user_id(value: &str) -> Result<NumericID, ApiError> {
    value.parse::<NumericID>().map_err(|_| {
        // The malformed value and the parser detail never reach the log
        // (`OBS-003`); only the classification and its declared fields do
        // (`OBS-006`).
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "user_id",
            reason = "invalid_identifier",
            "rejected a malformed user identifier"
        );
        ApiError::BadRequest("invalid user identifier".to_owned())
    })
}

/// Routes of the user bounded context.
///
/// Every route requires an authenticated caller.
pub fn user_routes(state: &AppState) -> Router {
    let me: Router<AppState> = Router::new().route("/me", get(get_me));

    let by_id: Router<AppState> = Router::new().route("/{id}", get(get_user));

    let patch_by_id: Router<AppState> = Router::new().route("/{id}", patch(patch_user));

    let list_users: Router<AppState> = Router::new().route("/", get(get_user_list));

    let permission: Router<AppState> =
        Router::new().route("/{id}/permission", get(get_permission).put(put_permission));

    me.merge(by_id)
        .merge(patch_by_id)
        .merge(list_users)
        .merge(permission)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}
