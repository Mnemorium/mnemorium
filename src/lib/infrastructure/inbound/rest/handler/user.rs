/// The user context endpoints.
pub(crate) mod get_me;
/// The user context endpoints.
pub(crate) mod get_user;
/// The user context endpoints.
pub(crate) mod get_user_list;
/// The user context endpoints.
pub(crate) mod patch_user;

use axum::middleware;
use serde::Serialize;
use tracing::warn;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::application::port::get_user::GetUserResponse as GetUserResponseData;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::SelfLinks;
use crate::infrastructure::inbound::rest::handler::identity::get_permission;
use crate::infrastructure::inbound::rest::handler::identity::put_permission;
use crate::infrastructure::inbound::rest::middleware::auth::authenticate;
use crate::infrastructure::outbound::jwt::token_provider::JwtTokenProvider;

/// User returned by a successful lookup.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct GetUserResponse {
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

/// Register the routes of the User bounded context.
///
/// The handlers carry their own paths, so this only groups them; protection is
/// applied by [`user_routes`].
pub(crate) fn user_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_me::get_me))
        .routes(routes!(get_user::get_user, patch_user::patch_user))
        .routes(routes!(get_user_list::get_user_list))
        .routes(routes!(
            get_permission::get_permission,
            put_permission::put_permission
        ))
}

/// Routes of the user bounded context.
///
/// Every route requires an authenticated caller.
pub(crate) fn user_routes(state: &AppState) -> OpenApiRouter<()> {
    user_router()
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            authenticate::<JwtTokenProvider>,
        ))
        .with_state(state.clone())
}
