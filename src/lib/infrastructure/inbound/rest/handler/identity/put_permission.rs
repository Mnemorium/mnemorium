use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::{
    PermissionRequest as PermissionRequestData, ReplacePermissionCommand, ReplacePermissionError,
};
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::JsonObject;
use crate::infrastructure::inbound::rest::handler::identity::{
    PermissionRequest, PermissionResponse,
};
use crate::infrastructure::inbound::rest::handler::user::parse_user_id;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Map a replace-permission error to its API error.
impl From<ReplacePermissionError> for ApiError {
    fn from(err: ReplacePermissionError) -> Self {
        match err {
            ReplacePermissionError::DuplicateAction | ReplacePermissionError::InvalidPermission => {
                Self::UnprocessableEntity(err.to_string())
            }
            ReplacePermissionError::Forbidden => Self::Forbidden(err.to_string()),
            ReplacePermissionError::NoSuchCaller => Self::Unauthorized(err.to_string()),
            ReplacePermissionError::NoSuchUser => Self::NotFound(err.to_string()),
            ReplacePermissionError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Replace a user's permission overrides.
///
/// Requires the Root Admin. The body carries the complete override set the
/// user must end with: an action absent from the body falls back to its role
/// preset. The request is validated as a whole and applied atomically.
#[utoipa::path(
    put,
    operation_id = "put_permission",
    path = "/{user_id}/permission",
    tag = "identity",
    request_body = PermissionRequest,
    params(
        ("user_id" = NumericID, Path, description = "Identifier of the user whose permissions are replaced"),
    ),
    responses(
        (status = OK, body = PermissionResponse, content_type = "application/hal+json", description = "Permissions replaced"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid user identifier or malformed request body"
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
            status = PAYLOAD_TOO_LARGE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body exceeds the maximum allowed size"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "The request body is not application/json"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown user"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "An action, scope or listed resource is unknown, or a permission is invalid"
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
    summary = "Replace a user's permission overrides"
)]
pub(crate) async fn put_permission(
    Path(user_id_value): Path<String>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    payload: JsonObject<PermissionRequest>,
) -> Result<Response, ApiError> {
    let user_id = parse_user_id(&user_id_value)?;
    let JsonObject(request) = payload;
    let permissions = request
        .permissions
        .into_iter()
        .map(PermissionRequestData::try_from)
        .collect::<Result<Vec<_>, ApiError>>()?;
    let response = state
        .identity_use_case_factory()
        .replace_permission()
        .execute(ReplacePermissionCommand::new(
            caller.user_id(),
            user_id,
            permissions,
        ))
        .await?;
    Ok(hal_json(PermissionResponse::from(response)))
}
