use axum::Json;
use axum::extract::Query;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::handler::user::get_user::GetUserResponse;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Filters restricting the returned users.
///
/// Every filter is optional; an all-`None` query returns every user.
#[derive(Debug, Deserialize, Serialize, IntoParams)]
#[non_exhaustive]
pub struct ListUsersQuery {
    /// Only return users whose email matches this value.
    #[param(format = "email")]
    pub email: Option<String>,
    /// Only return users holding this role.
    pub role: Option<Role>,
    /// Only return users whose username matches this value.
    #[param(min_length = 4, max_length = 100)]
    pub username: Option<String>,
}

// NOTE(stub): the `From<ListUsersError> for ApiError` mapping will be declared
// here (style-guide item order: query object -> response object -> error
// mapping -> handler) once the `list_users` use case exists. Expected shape:
//   ListUsersError::Forbidden  => ApiError::Forbidden(err.to_string()),
//   ListUsersError::Unknown(_) => ApiError::InternalServerError,
// No placeholder error type is declared now: nothing in the stub can construct
// it, so it would be dead code that fails `-D warnings`.

/// List the users of the instance.
///
/// The caller must present a valid `Bearer` token and hold the `Admin` role.
/// The response is the array of user profiles — identifier, username, email
/// and role — filtered by the optional query parameters. An empty list is a
/// valid outcome, not an error.
///
/// # Errors
///
/// Returns [`ApiError`] once implemented, mapping the use-case errors to their
/// HTTP responses.
///
/// # Panics
///
/// The endpoint is a stub: this handler always panics via `unimplemented!()`
/// until the `list_users` use case is implemented.
#[utoipa::path(
    get,
    operation_id = "list_users",
    path = "/user",
    tag = "user",
    params(ListUsersQuery),
    responses(
        (
            status = OK,
            body = [GetUserResponse],
            description = "Users matching the filters"
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid filter values"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            description = "Caller is not an administrator"
        ),
        (
            status = INTERNAL_SERVER_ERROR,
            body = ErrorBody,
            description = "Unexpected error"
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    summary = "List users"
)]
pub async fn list_users(
    Query(_query): Query<ListUsersQuery>,
    _caller: AuthenticatedUser,
    State(_state): State<AppState>,
) -> Result<Json<Vec<GetUserResponse>>, ApiError> {
    unimplemented!()
}
