use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse as _, Response};

use crate::application::port::RevokeShareCommand;
use crate::domain::NumericID;
use crate::infrastructure::inbound::rest::handler::identity::parse_address;
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody};

/// Revoke one grant from a resource.
///
/// The caller needs `Share` or `ManagePermissions`, and the grant must be
/// stored on the addressed resource.
#[utoipa::path(
    delete,
    operation_id = "delete_permission_share",
    path = "/permission/resource/{kind}/{id}/share/{grant_id}",
    tag = "identity",
    params(
        ("kind" = String, Path, description = "Kind of the addressed resource"),
        ("id" = NumericID, Path, description = "Identifier of the addressed resource"),
        ("grant_id" = NumericID, Path, description = "Identifier of the grant to revoke"),
    ),
    responses(
        (status = NO_CONTENT, description = "Grant revoked"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Invalid resource or grant identifier"
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
            description = "Caller may not revoke the grant"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown resource or grant"
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
    summary = "Revoke a shared grant"
)]
pub(crate) async fn delete_permission_share(
    Path((kind, id, grant_id)): Path<(String, String, String)>,
    State(state): State<AppState>,
    caller: AuthenticatedUser,
) -> Result<Response, ApiError> {
    let address = parse_address(&kind, &id)?;
    let parsed = grant_id
        .parse::<NumericID>()
        .map_err(|_| ApiError::BadRequest("invalid grant identifier".to_owned()))?;
    state
        .identity_use_case_factory()
        .revoke_share()
        .execute(RevokeShareCommand {
            address,
            caller_id: caller.user_id(),
            grant_id: parsed,
        })
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
