use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::register_user::RegisterUserError;
use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;
use crate::infrastructure::inbound::rest::api_error::ApiError;
use crate::infrastructure::inbound::rest::api_error::ErrorBody;
use crate::infrastructure::inbound::rest::app_state::AppState;

/// Payload to register a new user.
///
/// The requested `role` is granted only when the authenticated caller is
/// allowed to create it: the Root Admin can register `Admin` or `Standard`
/// users, an `Admin` user can register `Standard` users only.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct RegisterRequest {
    /// Email address of the new user, when provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Password of the new user; never returned by the API.
    #[schema(write_only, min_length = 8)]
    pub password: String,
    /// Role to grant to the new user.
    pub role: Role,
    /// Username of the new user.
    #[schema(min_length = 4, max_length = 100)]
    pub username: String,
}

/// User created by a successful registration.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub struct RegisterResponse {
    /// Email address of the newly registered user, when one was provided.
    #[schema(format = "email")]
    pub email: Option<String>,
    /// Unique identifier of the newly registered user.
    pub id: NumericID,
    /// Role granted to the newly registered user.
    pub role: Role,
    /// Username of the newly registered user.
    pub username: String,
}

/// Map a register-user error to its API error.
impl From<RegisterUserError> for ApiError {
    fn from(err: RegisterUserError) -> Self {
        match err {
            RegisterUserError::Forbidden => Self::Forbidden(err.to_string()),
            RegisterUserError::InvalidEmail
            | RegisterUserError::InvalidPassword
            | RegisterUserError::InvalidUsername => Self::BadRequest(err.to_string()),
            RegisterUserError::Unknown(_) => Self::InternalServerError,
            RegisterUserError::UserAlreadyExists => Self::Conflict(err.to_string()),
        }
    }
}

/// Register a new user on behalf of an authenticated caller.
///
/// The Root Admin (user identifier `0`) may register `Admin` or `Standard`
/// users; an `Admin` user may register `Standard` users only; any other role is
/// rejected with `403`.
///
/// # Errors
///
/// Returns [`ApiError`] when the request cannot be completed, mapping the
/// use-case errors to their HTTP responses.
#[utoipa::path(
    post,
    operation_id = "post_register",
    path = "/identity/register",
    tag = "identity",
    request_body = RegisterRequest,
    responses(
        (
            status = CREATED,
            body = RegisterResponse,
            headers(
                ("Location" = String, description = "URI of the newly registered user"),
            ),
            description = "User registered",
        ),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            description = "Invalid payload"
        ),
        (
            status = UNAUTHORIZED,
            body = ErrorBody,
            description = "Missing or invalid credentials"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            description = "Insufficient privileges to register the requested role"
        ),
        (
            status = CONFLICT,
            body = ErrorBody,
            description = "Username or email is already taken"
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
    summary = "Register a new user"
)]
pub async fn post_register(
    State(_state): State<AppState>,
) -> Result<Json<RegisterResponse>, ApiError> {
    unimplemented!()
}
