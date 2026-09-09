use axum::Json;
use axum::extract::Query;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use utoipa::ToSchema;

use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// User returned by a successful lookup.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct GetListUserResponse {
    /// Email address of the user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the user.
    pub id: NumericID,
    /// Role of the user.
    pub role: Role,
    /// Username of the user.
    pub username: String,
}

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

/// List the users of the instance.
///
/// Requires an `Admin` caller. Returns the profiles of every user matching
/// the optional filters. An empty list is a valid outcome.
#[utoipa::path(
    get,
    operation_id = "list_users",
    path = "/user",
    tag = "user",
    params(ListUsersQuery),
    responses(
        (
            status = OK,
            body = [GetListUserResponse],
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
pub async fn get_user_list(
    Query(_query): Query<ListUsersQuery>,
    _caller: AuthenticatedUser,
    State(_state): State<AppState>,
) -> Result<Json<Vec<GetListUserResponse>>, ApiError> {
    unimplemented!()
}
