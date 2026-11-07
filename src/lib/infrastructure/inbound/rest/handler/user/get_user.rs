use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_user::{GetUserCommand, GetUserError};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::user::{GetUserResponse, parse_user_id};
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a fetch-user error to its API error.
impl From<GetUserError> for ApiError {
    fn from(err: GetUserError) -> Self {
        match err {
            GetUserError::NotAdmin => Self::Forbidden(err.to_string()),
            GetUserError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetUserError::NoSuchUser => Self::NotFound(err.to_string()),
            GetUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Fetch a user by its identifier.
///
/// Requires an `Admin` caller. Returns the profile — identifier, username,
/// email and role — of the requested user.
#[utoipa::path(
    get,
    operation_id = "get_user",
    path = "/{id}",
    tag = "user",
    params(
        ("id" = NumericID, Path, description = "Identifier of the user to fetch"),
    ),
    responses(
        (status = OK, body = GetUserResponse, content_type = "application/hal+json", description = "User found"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid user identifier"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not an administrator"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown user"
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
    summary = "Fetch a user by identifier"
)]
pub(crate) async fn get_user(
    Path(id): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&id)?;
    let response = state
        .user_use_case_factory()
        .get_user()
        .execute(GetUserCommand::new(caller.user_id(), user_id))
        .await?;
    Ok(hal_json(GetUserResponse::from(response)))
}
