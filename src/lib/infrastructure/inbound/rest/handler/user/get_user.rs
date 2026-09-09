use axum::Json;
use axum::extract::Path;
use axum::extract::State;
use serde::{Deserialize, Serialize};
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
pub struct GetUserResponse {
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

// NOTE(stub): the `From<GetUserError> for ApiError` mapping will be declared
// here (style-guide item order: response struct -> error mapping -> handler)
// once the `get_user` use case exists. Expected shape:
//   GetUserError::NoSuchUser => ApiError::NotFound(err.to_string()),
//   GetUserError::Forbidden  => ApiError::Forbidden(err.to_string()),
//   GetUserError::Unknown(_) => ApiError::InternalServerError,
// No placeholder error type is declared now: nothing in the stub can construct
// it, so it would be dead code that fails `-D warnings`.

/// Fetch a user by its identifier.
///
/// The caller must present a valid `Bearer` token and hold the `Admin` role.
/// The response carries the profile — identifier, username, email and role —
/// never credential data.
///
/// # Errors
///
/// Returns [`ApiError`] once implemented, mapping the use-case errors to their
/// HTTP responses.
///
/// # Panics
///
/// The endpoint is a stub: this handler always panics via `unimplemented!()`
/// until the `get_user` use case is implemented.
#[utoipa::path(
    get,
    operation_id = "get_user",
    path = "/user/{id}",
    tag = "user",
    params(
        ("id" = NumericID, Path, description = "Identifier of the user to fetch"),
    ),
    responses(
        (status = OK, body = GetUserResponse, description = "User found"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid user identifier"
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
            status = NOT_FOUND,
            body = ErrorBody,
            description = "Unknown user"
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
    summary = "Fetch a user by identifier"
)]
pub async fn get_user(
    Path(_id): Path<NumericID>,
    _caller: AuthenticatedUser,
    State(_state): State<AppState>,
) -> Result<Json<GetUserResponse>, ApiError> {
    unimplemented!()
}
