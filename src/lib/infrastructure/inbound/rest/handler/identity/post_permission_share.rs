use axum::extract::{Path, State};
use axum::response::Response;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::application::port::ShareResourceCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::identity::{
    GrantBody, PrincipalPayload, parse_address, parse_resource_role,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, hal_json};

/// One read grant a share requests.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ShareRequest {
    /// The user principal the share reaches.
    pub principal: PrincipalPayload,
    /// The read role the share carries (`VIEWER` or `DOWNLOADER`).
    pub role: String,
}

/// Add or update one read grant on a resource.
///
/// The caller needs `Share` or `ManagePermissions`. Only `Viewer` and
/// `Downloader` `Allow` grants are accepted; a higher role is an escalation and
/// is refused.
#[utoipa::path(
    post,
    operation_id = "post_permission_share",
    path = "/permission/resource/{kind}/{id}/share",
    tag = "identity",
    params(
        ("kind" = String, Path, description = "Kind of the addressed resource"),
        ("id" = NumericID, Path, description = "Identifier of the addressed resource"),
    ),
    request_body = ShareRequest,
    responses(
        (status = OK, body = GrantBody, content_type = "application/hal+json", description = "Grant stored"),
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
            description = "Caller may not share, or the role escalates"
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
            description = "Unknown resource kind, principal or role, or invalid grant representation"
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
    summary = "Share a resource with a user"
)]
pub(crate) async fn post_permission_share(
    Path((kind, id)): Path<(String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    axum::Json(request): axum::Json<ShareRequest>,
) -> Result<Response, ApiError> {
    let address = parse_address(&kind, &id)?;
    let principal = request.principal.into_principal()?;
    let grant = state
        .identity_use_case_factory()
        .share_resource()
        .execute(ShareResourceCommand {
            address,
            caller_id: caller.user_id(),
            principal,
            role: parse_resource_role(&request.role)?,
        })
        .await?;
    Ok(hal_json(GrantBody::from_grant(&grant, address)))
}
