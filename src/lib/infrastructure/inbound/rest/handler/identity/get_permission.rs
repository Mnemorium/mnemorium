use axum::extract::{Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::application::port::{
    GetPermissionSummaryCommand, PermissionDecisionQuery, PermissionSummary,
};
use crate::domain::NumericID;
use crate::domain::model::ResourceRef;
use crate::domain::service::Explanation;
use crate::infrastructure::inbound::rest::handler::identity::{
    GrantBody, ResourceBody, parse_action, parse_resource_kind,
};
use crate::infrastructure::inbound::rest::middleware::AuthenticatedUser;
use crate::infrastructure::inbound::rest::{ApiError, AppState, ErrorBody, SelfLinks, hal_json};

/// Explanation of one authorization decision.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct DecisionResponse {
    /// Whether the caller may perform the action.
    pub allowed: bool,
    /// The addressed resource followed by its authorization parents.
    pub auth_path: Vec<ResourceBody>,
    /// Identifier of the grant that denied the action, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denied_by: Option<NumericID>,
    /// Identifiers of every grant that matched the action.
    pub matched: Vec<NumericID>,
    /// Why the decision allowed or denied the action.
    pub reason: String,
    /// Visibility of the addressed resource.
    pub visibility: String,
}

impl DecisionResponse {
    /// Project one domain explanation onto its wire shape.
    pub(crate) fn from_explanation(explanation: &Explanation) -> Self {
        Self {
            allowed: explanation.allowed(),
            auth_path: explanation
                .auth_path()
                .iter()
                .map(|address| ResourceBody::new(*address))
                .collect(),
            denied_by: explanation.denied_by(),
            matched: explanation.matched().to_vec(),
            reason: explanation.reason().as_wire().to_owned(),
            visibility: explanation.visibility().as_wire().to_owned(),
        }
    }
}

/// Effective permissions of one user.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PermissionSummaryResponse {
    /// Explanation of the requested decision, when one was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<DecisionResponse>,
    /// Grants that apply to the user.
    pub grants: Vec<GrantBody>,
    /// Link to the representation itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Role of the user the summary describes.
    pub role: String,
    /// Identifier of the user the summary describes.
    pub user_id: NumericID,
}

impl PermissionSummaryResponse {
    /// Project one summary onto its wire shape, hiding grant internals from an
    /// ordinary caller (`REQ-AUTHORIZATION-029`).
    pub(crate) fn from_summary(summary: &PermissionSummary, href: &str) -> Self {
        let grants = summary
            .grants
            .iter()
            .map(|granted| {
                if summary.detailed {
                    GrantBody::detailed(granted)
                } else {
                    GrantBody::effective(granted)
                }
            })
            .collect();
        Self {
            decision: summary
                .explanation
                .as_ref()
                .map(DecisionResponse::from_explanation),
            grants,
            links: SelfLinks::new(href),
            role: summary.role.as_wire().to_owned(),
            user_id: summary.user_id,
        }
    }
}

/// Query narrowing the summary to one decision.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[non_exhaustive]
pub(crate) struct PermissionQuery {
    /// Action the decision answers.
    pub action: Option<String>,
    /// Identifier of the addressed resource.
    pub resource_id: Option<NumericID>,
    /// Kind of the addressed resource.
    pub resource_kind: Option<String>,
    /// Identifier of the user whose summary is requested.
    pub user_id: Option<NumericID>,
}

impl PermissionQuery {
    /// Turn the query into its use-case command.
    fn into_command(self, caller_id: NumericID) -> Result<GetPermissionSummaryCommand, ApiError> {
        let user_id = self
            .user_id
            .ok_or_else(|| ApiError::BadRequest("user_id is required".to_owned()))?;
        let decision = match (self.resource_kind, self.resource_id, self.action) {
            (None, None, None) => None,
            (Some(kind), Some(id), Some(action)) => Some(PermissionDecisionQuery {
                action: parse_action(&action)?,
                resource: ResourceRef::new(parse_resource_kind(&kind)?, id),
            }),
            _ => {
                return Err(ApiError::BadRequest(
                    "resource_kind, resource_id and action must be supplied together".to_owned(),
                ));
            }
        };
        Ok(GetPermissionSummaryCommand {
            caller_id,
            decision,
            user_id,
        })
    }
}

/// Return the `_links.self` target of a user's summary.
#[must_use]
pub(crate) fn summary_href(user_id: NumericID) -> String {
    format!("/api/v1/identity/permission?user_id={user_id}")
}

/// Return one user's effective permissions.
///
/// A caller reads its own summary; another user's summary needs
/// `ManagePermissions` on that `User` resource. Supplying `resource_kind`,
/// `resource_id` and `action` also returns the explanation, which only a
/// `ManagePermissions` holder receives.
#[utoipa::path(
    get,
    operation_id = "get_permission_summary",
    path = "/permission",
    tag = "identity",
    params(PermissionQuery),
    responses(
        (status = OK, body = PermissionSummaryResponse, content_type = "application/hal+json", description = "Summary returned"),
        (
            status = BAD_REQUEST,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Missing or inconsistent query"
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
            description = "Caller may not read the summary"
        ),
        (
            status = NOT_FOUND,
            body = ErrorBody,
            content_type = "application/hal+json",
            description = "Unknown user or resource"
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
    summary = "Return a user's effective permissions"
)]
pub(crate) async fn get_permission(
    State(state): State<AppState>,
    caller: AuthenticatedUser,
    Query(query): Query<PermissionQuery>,
) -> Result<Response, ApiError> {
    let command = query.into_command(caller.user_id())?;
    let summary = state
        .identity_use_case_factory()
        .get_permission_summary()
        .execute(command)
        .await?;
    let href = summary_href(summary.user_id);
    Ok(hal_json(PermissionSummaryResponse::from_summary(
        &summary, &href,
    )))
}
