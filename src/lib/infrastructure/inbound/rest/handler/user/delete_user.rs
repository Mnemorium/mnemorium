use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::application::port::{DeleteUserCommand, DeleteUserError};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::user::parse_user_id;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Map a delete-user error to its API error.
impl From<DeleteUserError> for ApiError {
    fn from(err: DeleteUserError) -> Self {
        match err {
            DeleteUserError::NotAdmin | DeleteUserError::TargetNotModifiable => {
                Self::Forbidden(err.to_string())
            }
            DeleteUserError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            DeleteUserError::NoSuchUser => Self::NotFound(err.to_string()),
            DeleteUserError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Delete a user account and the data it owns by its identifier.
///
/// Requires `user.delete`: the Admin preset grants it, a Standard user needs an
/// override, and only the Root Admin may delete an Admin account. The Root
/// Admin's own account is never deletable. Deletion is permanent: private media
/// is removed with its bytes, while media another user's resource or the shared
/// catalogue still references is kept under the Root Admin's ownership.
#[utoipa::path(
    delete,
    operation_id = "delete_user",
    path = "/{id}",
    tag = "user",
    params(
        ("id" = NumericID, Path, description = "Identifier of the user to delete"),
    ),
    responses(
        (
            status = NO_CONTENT,
            description = "User deleted"
        ),
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
            description = "Caller may not delete the target account"
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
    summary = "Delete a user account"
)]
pub(crate) async fn delete_user(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user_id = parse_user_id(&id)?;
    state
        .user_use_case_factory()
        .delete_user()
        .execute(DeleteUserCommand::new(caller.user_id(), user_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
