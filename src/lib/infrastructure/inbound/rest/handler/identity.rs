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
/// The Root Admin setup endpoint.
mod post_setup;
/// The identity context endpoints.
mod post_token;
/// The resource permission replacement endpoint.
mod put_resource_permission;

use axum::middleware;
use serde::{Deserialize, Serialize};
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

/// Wire vocabulary of a resource visibility (`REQ-API-018`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum VisibilityWire {
    /// Adds no implicit grant.
    Private,
    /// Grants the viewer actions to every authenticated caller.
    Public,
    /// Grants discovery and metadata to every authenticated caller.
    Restricted,
}

/// Wire vocabulary of the actions a permission decision names (`REQ-API-018`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub(crate) enum ActionWire {
    /// Change a user's role.
    ChangeRole,
    /// Create a child of the addressed creation surface or resource.
    CreateChild,
    /// Remove the resource.
    Delete,
    /// Report that a resource exists and include it in a listing.
    Discover,
    /// Retrieve the original bytes of a resource.
    Download,
    /// Change the metadata of a resource.
    EditMetadata,
    /// Change the visibility or the grant set of a resource.
    ManagePermissions,
    /// Replace a user's credential.
    ReplaceCredential,
    /// Replace the binary content of a resource.
    ReplaceMedia,
    /// Add or revoke a read grant on a resource.
    Share,
    /// Play the audio or video bytes of a resource.
    Stream,
    /// Read the rendered content of a resource.
    View,
    /// Read the metadata of a resource.
    ViewMetadata,
}

/// Wire vocabulary of a resource kind (`REQ-API-018`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub(crate) enum ResourceKindWire {
    /// An audio asset.
    Audio,
    /// The shared catalogue container.
    Catalogue,
    /// An episode.
    Episode,
    /// A stored file.
    File,
    /// A gallery.
    Gallery,
    /// The gallery creation surface.
    GalleryCollection,
    /// An item of a gallery.
    GalleryItem,
    /// An image asset.
    Image,
    /// The instance root.
    Instance,
    /// The library container.
    Library,
    /// A film.
    Movie,
    /// A music album.
    MusicAlbum,
    /// A music group.
    MusicGroup,
    /// A music medium.
    MusicMedium,
    /// A music playlist.
    MusicPlaylist,
    /// A music recording.
    MusicRecording,
    /// The playlist creation surface.
    PlaylistCollection,
    /// A season of a show.
    Season,
    /// A show.
    Series,
    /// An upload session.
    Upload,
    /// The upload creation surface.
    UploadCollection,
    /// A user account.
    User,
    /// The user creation surface.
    UserCollection,
    /// A video asset.
    Video,
}

/// Wire vocabulary of the child kind a grant permits (`REQ-API-018`).
///
/// `ADMIN` is absent because it is root-only and is never stored as a grant.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub(crate) enum ChildKindWire {
    /// The grant permits every child kind.
    All,
    /// An audio asset.
    Audio,
    /// An episode.
    Episode,
    /// A stored file.
    File,
    /// A gallery.
    Gallery,
    /// A gallery item.
    GalleryItem,
    /// An image asset.
    Image,
    /// A film.
    Movie,
    /// A music album.
    MusicAlbum,
    /// A music group.
    MusicGroup,
    /// A music medium.
    MusicMedium,
    /// A music playlist.
    MusicPlaylist,
    /// A music recording.
    MusicRecording,
    /// A season.
    Season,
    /// A show.
    Series,
    /// An upload session.
    Upload,
    /// A user account.
    User,
    /// A video asset.
    Video,
}

/// Wire vocabulary of a grant effect (`REQ-API-018`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum GrantEffectWire {
    /// The role's actions are permitted.
    Allow,
    /// The role's actions are refused.
    Deny,
}

/// Wire vocabulary of a role a caller may store as a grant (`REQ-API-018`).
///
/// `OWNER` is absent because ownership is implicit and is never a user grant.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum GrantRoleWire {
    /// Contributes a child.
    Contributor,
    /// Reads, discovers and downloads.
    Downloader,
    /// Changes metadata and media.
    Editor,
    /// Manages the grant set.
    Manager,
    /// Reads and discovers.
    Viewer,
}

/// Wire vocabulary of a read role a share grants (`REQ-API-018`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum ShareRoleWire {
    /// Reads, discovers and downloads.
    Downloader,
    /// Reads and discovers.
    Viewer,
}

/// Wire vocabulary of a resource role a permission representation reports (`REQ-API-018`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum ResourceRoleWire {
    /// Contributes a child.
    Contributor,
    /// Reads, discovers and downloads.
    Downloader,
    /// Changes metadata and media.
    Editor,
    /// Manages the grant set.
    Manager,
    /// Owns the resource; implicit from ownership.
    Owner,
    /// Reads and discovers.
    Viewer,
}

/// Wire vocabulary of a principal kind a grant may name (`REQ-API-018`).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum PrincipalKindWire {
    /// A role principal.
    Role,
    /// A user principal.
    User,
}

/// Wire vocabulary of a principal kind a representation reports (`REQ-API-018`).
///
/// `ROOT` is response-only: the root holds no stored grant, so a request never
/// names it.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub(crate) enum PrincipalKindResponseWire {
    /// A role principal.
    Role,
    /// The instance root.
    Root,
    /// A user principal.
    User,
}

/// Kind, identifier and stored identity of one addressed resource.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct ResourceBody {
    /// Identifier of the entity in its own table.
    pub id: NumericID,
    /// Kind of the addressed resource.
    #[schema(value_type = ResourceKindWire)]
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
    #[schema(value_type = PrincipalKindResponseWire)]
    pub kind: String,
    /// Role the grant names, when the principal is a role.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Role>)]
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
    #[schema(value_type = VisibilityWire)]
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
    #[schema(value_type = ChildKindWire)]
    pub child_kind: String,
    /// Whether the grant allows or denies its role's actions.
    #[schema(value_type = GrantEffectWire)]
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
    #[schema(value_type = ResourceRoleWire)]
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
    #[schema(value_type = PrincipalKindWire)]
    pub kind: String,
    /// Role the principal names, when the principal is a role.
    #[schema(value_type = Option<Role>)]
    pub role: Option<String>,
}

impl PrincipalPayload {
    /// Turn the payload into the domain principal it names.
    pub(crate) fn into_principal(self) -> Result<Principal, ApiError> {
        let Self { id, kind, role } = self;
        // The schema publishes `role` as an enum, so a value outside it is
        // refused whatever `kind` is: a property the document declares is
        // enforced even when the branch does not use it (`REQ-API-018`).
        let validated_role = match role.as_deref() {
            None => None,
            Some(stored) => {
                Some(Role::from_wire(stored).ok_or_else(|| ApiError::reject_enum("role"))?)
            }
        };
        match kind.as_str() {
            "USER" => {
                let user_id = id.ok_or_else(|| {
                    ApiError::UnprocessableEntity("the principal identifier is required".to_owned())
                })?;
                Ok(Principal::User(user_id))
            }
            "ROLE" => {
                let principal_role = validated_role.ok_or_else(|| {
                    ApiError::UnprocessableEntity("the principal role is required".to_owned())
                })?;
                Ok(Principal::Role(principal_role))
            }
            _ => Err(ApiError::reject_enum("kind")),
        }
    }
}

/// One grant a replacement requests.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct GrantPayload {
    /// Kind of child the grant permits when its role includes `CreateChild`.
    #[schema(value_type = Option<ChildKindWire>)]
    pub child_kind: Option<String>,
    /// Whether the grant allows or denies its role's actions.
    #[schema(value_type = GrantEffectWire)]
    pub effect: String,
    /// The user principal the grant applies to.
    pub principal: PrincipalPayload,
    /// The resource role the grant carries.
    #[schema(value_type = GrantRoleWire)]
    pub role: String,
}

impl GrantPayload {
    /// Turn the payload into its application-layer request.
    pub(crate) fn into_request(self) -> Result<GrantRequest, ApiError> {
        let principal = self.principal.into_principal()?;
        let child_kind = match self.child_kind.as_deref() {
            None => ChildKind::All,
            Some(value) => parse_child_kind(value)?,
        };
        Ok(GrantRequest {
            child_kind,
            effect: parse_grant_effect(&self.effect)?,
            principal,
            role: parse_grant_role(&self.role)?,
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
    Action::from_wire(value).ok_or_else(|| ApiError::reject_enum("action"))
}

/// Parse a resource kind from its wire representation.
pub(crate) fn parse_resource_kind(value: &str) -> Result<ResourceKind, ApiError> {
    ResourceKind::from_wire(value).ok_or_else(|| ApiError::reject_enum("resource_kind"))
}

/// Parse a role a caller may store as a grant.
///
/// `OWNER` is implicit from ownership and is never a user grant, so it is
/// outside the published vocabulary and is rejected as an unknown value.
pub(crate) fn parse_grant_role(value: &str) -> Result<ResourceRole, ApiError> {
    ResourceRole::from_wire(value)
        .filter(|role| role.is_grantable_by_user())
        .ok_or_else(|| ApiError::reject_enum("role"))
}

/// Parse a read role a share grants.
pub(crate) fn parse_share_role(value: &str) -> Result<ResourceRole, ApiError> {
    ResourceRole::from_wire(value)
        .filter(|role| matches!(role, ResourceRole::Viewer | ResourceRole::Downloader))
        .ok_or_else(|| ApiError::reject_enum("role"))
}

/// Parse a child kind a grant may permit.
///
/// `ADMIN` is root-only and is never stored as a grant, so it is outside the
/// published vocabulary and is rejected as an unknown value.
pub(crate) fn parse_child_kind(value: &str) -> Result<ChildKind, ApiError> {
    ChildKind::from_wire(value)
        .filter(|kind| *kind != ChildKind::Admin)
        .ok_or_else(|| ApiError::reject_enum("child_kind"))
}

/// Parse a grant effect from its wire representation.
pub(crate) fn parse_grant_effect(value: &str) -> Result<GrantEffect, ApiError> {
    GrantEffect::from_wire(value).ok_or_else(|| ApiError::reject_enum("effect"))
}

/// Parse a visibility from its wire representation.
pub(crate) fn parse_visibility(value: &str) -> Result<Visibility, ApiError> {
    Visibility::from_wire(value).ok_or_else(|| ApiError::reject_enum("visibility"))
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
/// The handlers carry their own paths, so this only groups them; the per-client
/// rate limit is applied by [`identity_routes`].
pub(crate) fn identity_public_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(post_login::post_login))
        .routes(routes!(post_setup::post_setup))
}

/// Routes of the identity bounded context.
///
/// `login` and `setup` are public; `register`, the credential password change
/// and token regeneration require an authenticated caller. The public routes are
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
