use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::get_permission::{GetPermissionCommand, GetPermissionError};
use crate::domain::alias::NumericID;
use crate::infrastructure::inbound::rest::api_error::{ApiError, ErrorBody};
use crate::infrastructure::inbound::rest::app_state::AppState;
use crate::infrastructure::inbound::rest::hal::hal_json;
use crate::infrastructure::inbound::rest::handler::identity::PermissionResponse;
use crate::infrastructure::inbound::rest::handler::user::parse_user_id;
use crate::infrastructure::inbound::rest::middleware::auth::AuthenticatedUser;

/// Map a fetch-permission error to its API error.
impl From<GetPermissionError> for ApiError {
    fn from(err: GetPermissionError) -> Self {
        match err {
            GetPermissionError::Forbidden => Self::Forbidden(err.to_string()),
            GetPermissionError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            GetPermissionError::NoSuchUser => Self::NotFound(err.to_string()),
            GetPermissionError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Read a user's effective permissions.
///
/// Requires the Root Admin. Returns one entry per configurable action: the
/// scope it resolves to, whether a stored override supplies it, and the
/// aggregates a `LISTED` scope names.
#[utoipa::path(
    get,
    operation_id = "get_permission",
    path = "/{user_id}/permission",
    tag = "identity",
    params(
        ("user_id" = NumericID, Path, description = "Identifier of the user whose permissions are read"),
    ),
    responses(
        (status = OK, body = PermissionResponse, content_type = "application/hal+json", description = "Effective permissions"),
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
            description = "Missing or invalid credentials, or a deleted caller"
        ),
        (
            status = FORBIDDEN,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Caller is not the Root Admin, or the target is the Root Admin"
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
    summary = "Read a user's effective permissions"
)]
pub(crate) async fn get_permission(
    Path(user_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&user_id_value)?;
    let response = state
        .identity_use_case_factory()
        .get_permission()
        .execute(GetPermissionCommand::new(caller.user_id(), user_id))
        .await?;
    Ok(hal_json(PermissionResponse::from(response)))
}
