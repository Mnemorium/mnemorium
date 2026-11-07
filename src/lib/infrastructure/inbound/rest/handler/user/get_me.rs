use axum::extract::State;
use axum::response::Response;
use serde::Serialize;
use utoipa::ToSchema;

use crate::application::port::get_current_user::{
    GetCurrentUserCommand, GetCurrentUserError, GetCurrentUserResponse,
};
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::{SelfLinks, hal_json};
use crate::infrastructure::inbound::rest::handler::user::user_self_href;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Current user returned by a successful lookup.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
struct GetMeResponse {
    /// Email address of the caller, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the caller.
    pub id: NumericID,
    /// Link to the caller's user resource.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Role of the caller.
    pub role: Role,
    /// Username of the caller.
    pub username: String,
}

/// Map the current-user response onto its HTTP representation.
impl From<GetCurrentUserResponse> for GetMeResponse {
    fn from(response: GetCurrentUserResponse) -> Self {
        Self {
            email: response.email().map(str::to_owned),
            id: response.id(),
            links: SelfLinks::new(&user_self_href(response.id())),
            role: response.role(),
            username: response.username().to_owned(),
        }
    }
}

/// Map a current-user error to its API error.
impl From<GetCurrentUserError> for ApiError {
    fn from(err: GetCurrentUserError) -> Self {
        match err {
            GetCurrentUserError::NoSuchUser => Self::Unauthorized(err.to_string()),
            GetCurrentUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch the account of the authenticated caller.
///
/// Returns the caller's own profile — identifier, username, email and role.
#[utoipa::path(
    get,
    operation_id = "get_me",
    path = "/me",
    tag = "user",
    responses(
        (status = OK, body = GetMeResponse, content_type = "application/hal+json", description = "Current user"),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "Fetch the current user"
)]
pub(crate) async fn get_me(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let response = state
        .user_use_case_factory()
        .get_current_user()
        .execute(GetCurrentUserCommand::new(caller.user_id()))
        .await?;
    Ok(hal_json(GetMeResponse::from(response)))
}
