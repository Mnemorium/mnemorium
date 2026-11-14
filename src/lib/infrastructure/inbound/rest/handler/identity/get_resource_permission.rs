use axum::extract::{Path, State};
use axum::response::Response;

use crate::application::port::GetResourcePermissionCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::identity::{
    ResourcePermissionBody, parse_address,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// Return one resource's visibility, owner and grants.
///
/// The caller needs `Share` or `ManagePermissions` on the resource.
#[utoipa::path(
    get,
    operation_id = "get_resource_permission",
    path = "/permission/resource/{kind}/{id}",
    tag = "identity",
    params(
        ("kind" = String, Path, description = "Kind of the addressed resource"),
        ("id" = NumericID, Path, description = "Identifier of the addressed resource"),
    ),
    responses(
        (status = OK, body = ResourcePermissionBody, content_type = "application/hal+json", description = "Permission returned"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid resource identifier"
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
            description = "Caller may not read the policy"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown resource"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown resource kind"
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
    summary = "Return a resource's permission"
)]
pub(crate) async fn get_resource_permission(
    Path((kind, id)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let command = GetResourcePermissionCommand {
        address: parse_address(&kind, &id)?,
        caller_id: caller.user_id(),
    };
    let permission = state
        .identity_use_case_factory()
        .get_resource_permission()
        .execute(command)
        .await?;
    Ok(hal_json(ResourcePermissionBody::from_permission(
        &permission,
    )))
}
