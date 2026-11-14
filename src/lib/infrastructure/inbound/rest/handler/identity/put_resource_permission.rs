use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::PutResourcePermissionCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::identity::{
    GrantPayload, ResourcePermissionBody, parse_address, parse_visibility,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// A replacement of a resource's visibility and complete user grant set.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PutPermissionRequest {
    /// The complete user grant set that replaces the stored one.
    pub grants: Vec<GrantPayload>,
    /// The visibility the resource takes.
    pub visibility: String,
}

/// Replace one resource's visibility and complete user grant set.
///
/// The replacement is atomic: a rejected grant stores nothing. The caller needs
/// `ManagePermissions` on the resource.
#[utoipa::path(
    put,
    operation_id = "put_resource_permission",
    path = "/permission/resource/{kind}/{id}",
    tag = "identity",
    params(
        ("kind" = String, Path, description = "Kind of the addressed resource"),
        ("id" = NumericID, Path, description = "Identifier of the addressed resource"),
    ),
    request_body = PutPermissionRequest,
    responses(
        (status = OK, body = ResourcePermissionBody, content_type = "application/hal+json", description = "Permission replaced"),
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
            description = "Caller may not replace the policy"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown resource"
        ),
        (
            status = UNSUPPORTED_MEDIA_TYPE,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unsupported request media type"
        ),
        (
            status = UNPROCESSABLE_ENTITY,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown resource kind, visibility or invalid grant set"
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
    summary = "Replace a resource's permission"
)]
pub(crate) async fn put_resource_permission(
    Path((kind, id)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::Json(request): axum::Json<PutPermissionRequest>,
) -> Result<Response, ApiError> {
    let address = parse_address(&kind, &id)?;
    let grants = request
        .grants
        .into_iter()
        .map(GrantPayload::into_request)
        .collect::<Result<Vec<_>, _>>()?;
    let command = PutResourcePermissionCommand {
        address,
        caller_id: caller.user_id(),
        grants,
        visibility: parse_visibility(&request.visibility)?,
    };
    let permission = state
        .identity_use_case_factory()
        .put_resource_permission()
        .execute(command)
        .await?;
    Ok(hal_json(ResourcePermissionBody::from_permission(
        &permission,
    )))
}
