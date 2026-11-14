/// The permission share revocation endpoint.
mod delete_permission_share;
/// The permission summary endpoint.
mod get_permission;
/// The resource permission read endpoint.
mod get_resource_permission;
/// The identity context endpoints.
mod patch_credential;
/// The identity context endpoints.
mod post_login;
/// The permission share endpoint.
mod post_permission_share;
/// The identity context endpoints.
mod post_register;
/// The identity context endpoints.
mod post_token;
/// The resource permission replacement endpoint.
mod put_resource_permission;

#[cfg(test)]
pub(crate) use post_login::{LoginRequest, LoginResponse, post_login as handle_login};
#[cfg(test)]
pub(crate) use post_token::{TokenResponse, post_token as handle_token};

use axum::middleware;
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::application::port::{
    GrantRequest, GrantedResource, PermissionError, ResourcePermission,
};
use crate::domain::NumericID;
use crate::domain::model::{
    Action, ChildKind, Grant, GrantEffect, Principal, ResourceKind, ResourceRef, ResourceRole,
    Role, Visibility,
};
use crate::infrastructure::inbound::rest::middleware::{LoginRateLimiter, authenticate};
use crate::infrastructure::inbound::rest::{ApiError, AppState, SelfLinks};
use crate::infrastructure::outbound::jwt::JwtTokenProvider;

/// Kind, identifier and stored identity of one addressed resource.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ResourceBody {
    /// Identifier of the entity in its own table.
    pub id: NumericID,
    /// Kind of the addressed resource.
    pub kind: String,
}

impl ResourceBody {
    /// Project one resource address onto its wire shape.
    pub(crate) fn new(address: ResourceRef) -> Self {
        Self {
            id: address.id(),
            kind: address.kind().as_wire().to_owned(),
        }
    }
}

/// The principal a grant names.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PrincipalBody {
    /// Identifier of the user principal, when the grant names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<NumericID>,
    /// Kind of the principal (`USER`, `ROLE` or `ROOT`).
    pub kind: String,
    /// Role the grant names, when the principal is a role.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

impl PrincipalBody {
    /// Project one domain principal onto its wire shape.
    pub(crate) fn from_principal(principal: Principal) -> Self {
        match principal {
            Principal::Role(role) => Self {
                id: None,
                kind: "ROLE".to_owned(),
                role: Some(role.as_wire().to_owned()),
            },
            Principal::Root => Self {
                id: None,
                kind: "ROOT".to_owned(),
                role: None,
            },
            Principal::User(id) => Self {
                id: Some(id),
                kind: "USER".to_owned(),
                role: None,
            },
        }
    }
}

/// Visibility, owner and grants of one resource.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ResourcePermissionBody {
    /// The addressed resource.
    pub address: ResourceBody,
    /// Grants stored on the resource.
    pub grants: Vec<GrantBody>,
    /// Link to the representation itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Identifier of the owning user, when the resource is owned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<NumericID>,
    /// Visibility of the resource.
    pub visibility: String,
}

impl ResourcePermissionBody {
    /// Project one stored permission onto its wire shape.
    pub(crate) fn from_permission(permission: &ResourcePermission) -> Self {
        Self {
            grants: permission
                .grants
                .iter()
                .map(|grant| GrantBody::from_grant(grant, permission.address))
                .collect(),
            links: SelfLinks::new(&resource_permission_href(permission.address)),
            owner_id: permission.owner_id,
            address: ResourceBody::new(permission.address),
            visibility: permission.visibility.as_wire().to_owned(),
        }
    }
}

/// One grant as a permission representation reports it.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct GrantBody {
    /// The resource the grant is stored on.
    pub address: ResourceBody,
    /// Kind of child the grant permits when its role includes `CreateChild`.
    pub child_kind: String,
    /// Whether the grant allows or denies its role's actions.
    pub effect: String,
    /// Stored identity of the grant, when the caller may see grant internals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_id: Option<NumericID>,
    /// Link to the representation itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// The principal the grant applies to, when the caller may see it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal: Option<PrincipalBody>,
    /// The resource role the grant carries.
    pub role: String,
}

impl GrantBody {
    /// Project one grant with its stored identity.
    pub(crate) fn detailed(granted: &GrantedResource) -> Self {
        let mut response = Self::effective(granted);
        response.grant_id = Some(granted.grant.id());
        response.principal = Some(PrincipalBody::from_principal(granted.grant.principal()));
        response.links = SelfLinks::new(&share_href(granted.resource, granted.grant.id()));
        response
    }

    /// Project one effective grant without its stored identity.
    ///
    /// An ordinary caller receives the permissions, never the grants behind
    /// them (`REQ-AUTHORIZATION-029`).
    pub(crate) fn effective(granted: &GrantedResource) -> Self {
        Self {
            child_kind: granted.grant.child_kind().as_wire().to_owned(),
            effect: granted.grant.effect().as_wire().to_owned(),
            grant_id: None,
            principal: None,
            address: ResourceBody::new(granted.resource),
            links: SelfLinks::new(&resource_permission_href(granted.resource)),
            role: granted.grant.role().as_wire().to_owned(),
        }
    }

    /// Project one stored grant on a known resource.
    pub(crate) fn from_grant(grant: &Grant, address: ResourceRef) -> Self {
        Self {
            child_kind: grant.child_kind().as_wire().to_owned(),
            effect: grant.effect().as_wire().to_owned(),
            grant_id: Some(grant.id()),
            principal: Some(PrincipalBody::from_principal(grant.principal())),
            address: ResourceBody::new(address),
            links: SelfLinks::new(&share_href(address, grant.id())),
            role: grant.role().as_wire().to_owned(),
        }
    }
}

/// The principal a grant names.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PrincipalPayload {
    /// Identifier of the user principal.
    pub id: Option<NumericID>,
    /// Kind of the principal (`USER` or `ROLE`).
    pub kind: String,
    /// Role the principal names, when the principal is a role.
    pub role: Option<String>,
}

impl PrincipalPayload {
    /// Turn the payload into the domain principal it names.
    pub(crate) fn into_principal(self) -> Result<Principal, ApiError> {
        match self.kind.as_str() {
            "USER" => {
                let id = self.id.ok_or_else(|| {
                    ApiError::UnprocessableEntity("the principal identifier is required".to_owned())
                })?;
                Ok(Principal::User(id))
            }
            "ROLE" => {
                let stored = self.role.ok_or_else(|| {
                    ApiError::UnprocessableEntity("the principal role is required".to_owned())
                })?;
                let role = Role::from_wire(&stored).ok_or_else(|| {
                    ApiError::UnprocessableEntity("unknown principal role".to_owned())
                })?;
                Ok(Principal::Role(role))
            }
            _ => Err(ApiError::UnprocessableEntity(
                "unknown principal kind".to_owned(),
            )),
        }
    }
}

/// One grant a replacement requests.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct GrantPayload {
    /// Kind of child the grant permits when its role includes `CreateChild`.
    pub child_kind: Option<String>,
    /// Whether the grant allows or denies its role's actions.
    pub effect: String,
    /// The user principal the grant applies to.
    pub principal: PrincipalPayload,
    /// The resource role the grant carries.
    pub role: String,
}

impl GrantPayload {
    /// Turn the payload into its application-layer request.
    pub(crate) fn into_request(self) -> Result<GrantRequest, ApiError> {
        let principal = self.principal.into_principal()?;
        let child_kind = match self.child_kind.as_deref() {
            None => ChildKind::All,
            Some(value) => ChildKind::from_wire(value)
                .ok_or_else(|| ApiError::UnprocessableEntity("unknown child kind".to_owned()))?,
        };
        let effect = GrantEffect::from_wire(&self.effect)
            .ok_or_else(|| ApiError::UnprocessableEntity("unknown grant effect".to_owned()))?;
        Ok(GrantRequest {
            child_kind,
            effect,
            principal,
            role: parse_resource_role(&self.role)?,
        })
    }
}

/// Map a permission error to its API error.
impl From<PermissionError> for ApiError {
    fn from(error: PermissionError) -> Self {
        match error {
            PermissionError::Forbidden => Self::Forbidden(error.to_string()),
            PermissionError::Invalid(reason) => Self::UnprocessableEntity(reason.to_owned()),
            PermissionError::NotFound => Self::NotFound(error.to_string()),
            PermissionError::Unknown(_) => Self::InternalServerError,
        }
    }
}

/// Render the URI reference of one shared grant.
#[must_use]
pub(crate) fn share_href(address: ResourceRef, grant_id: NumericID) -> String {
    format!(
        "/api/v1/identity/permission/resource/{}/{}/share/{grant_id}",
        address.kind().as_wire(),
        address.id()
    )
}

/// Render a resource's permission URI reference.
#[must_use]
pub(crate) fn resource_permission_href(address: ResourceRef) -> String {
    format!(
        "/api/v1/identity/permission/resource/{}/{}",
        address.kind().as_wire(),
        address.id()
    )
}

/// Parse an addressed resource from its path segments.
pub(crate) fn parse_address(kind: &str, id: &str) -> Result<ResourceRef, ApiError> {
    let parsed = id
        .parse::<NumericID>()
        .map_err(|_| ApiError::BadRequest("invalid resource identifier".to_owned()))?;
    Ok(ResourceRef::new(parse_resource_kind(kind)?, parsed))
}

/// Parse an action from its wire representation.
pub(crate) fn parse_action(value: &str) -> Result<Action, ApiError> {
    Action::from_wire(value).ok_or_else(|| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "action",
            reason = "invalid_enum",
            "rejected an unknown action"
        );
        ApiError::UnprocessableEntity("unknown action".to_owned())
    })
}

/// Parse a resource kind from its wire representation.
pub(crate) fn parse_resource_kind(value: &str) -> Result<ResourceKind, ApiError> {
    ResourceKind::from_wire(value).ok_or_else(|| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "resource_kind",
            reason = "invalid_enum",
            "rejected an unknown resource kind"
        );
        ApiError::UnprocessableEntity("unknown resource kind".to_owned())
    })
}

/// Parse a resource role from its wire representation.
pub(crate) fn parse_resource_role(value: &str) -> Result<ResourceRole, ApiError> {
    ResourceRole::from_wire(value).ok_or_else(|| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "role",
            reason = "invalid_enum",
            "rejected an unknown resource role"
        );
        ApiError::UnprocessableEntity("unknown resource role".to_owned())
    })
}

/// Parse a visibility from its wire representation.
pub(crate) fn parse_visibility(value: &str) -> Result<Visibility, ApiError> {
    Visibility::from_wire(value).ok_or_else(|| {
        warn!(
            target: "security",
            event = "input_validation_failed",
            field = "visibility",
            reason = "invalid_enum",
            "rejected an unknown visibility"
        );
        ApiError::UnprocessableEntity("unknown visibility".to_owned())
    })
}

/// Register the protected routes of the identity bounded context.
///
/// The handlers carry their own paths, so this only groups them; protection is
/// applied by [`identity_routes`].
pub(crate) fn identity_protected_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_permission::get_permission))
        .routes(routes!(get_resource_permission::get_resource_permission))
        .routes(routes!(delete_permission_share::delete_permission_share))
        .routes(routes!(patch_credential::patch_credential))
        .routes(routes!(post_permission_share::post_permission_share))
        .routes(routes!(post_register::post_register))
        .routes(routes!(post_token::post_token))
        .routes(routes!(put_resource_permission::put_resource_permission))
}

/// Register the public routes of the identity bounded context.
///
/// The handlers carry their own paths, so this only groups them; the login rate
/// limit is applied by [`identity_routes`].
pub(crate) fn identity_public_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(post_login::post_login))
}

/// Routes of the identity bounded context.
///
/// `login` is public; `register`, the credential password change and token
/// regeneration require an authenticated caller. The public `login` route is
/// rate limited per client address by the `limiter` the composition root built;
/// the other routes are not.
pub(crate) fn identity_routes(state: &AppState, limiter: &LoginRateLimiter) -> OpenApiRouter<()> {
    let protected = identity_protected_router().route_layer(middleware::from_fn_with_state(
        state.clone(),
        authenticate::<JwtTokenProvider>,
    ));

    protected
        .merge(limiter.apply(identity_public_router()))
        .with_state(state.clone())
}
