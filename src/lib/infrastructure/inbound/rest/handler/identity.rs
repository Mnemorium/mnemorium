/// The identity context endpoints.
mod get_permission;
/// The identity context endpoints.
mod patch_credential;
/// The identity context endpoints.
mod post_login;
/// The identity context endpoints.
mod post_register;
/// The identity context endpoints.
mod put_permission;

pub(crate) use self::get_permission::{__path_get_permission, get_permission};
pub(crate) use self::put_permission::{__path_put_permission, put_permission};

use axum::middleware;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::application::port::{
    GetPermissionResponse as GetPermissionResponseData, PermissionRequest as PermissionRequestData,
};
use crate::domain::NumericID;
use crate::domain::model::{Action, ResourceKind, ResourceRef, Role, Scope};
use crate::infrastructure::inbound::rest::middleware::{LoginRateLimiter, authenticate};
use crate::infrastructure::inbound::rest::{ApiError, AppState, SelfLinks};
use crate::infrastructure::outbound::jwt::JwtTokenProvider;

/// Kind of an aggregate a `LISTED` scope names.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) enum PermissionResourceKind {
    /// A stored file.
    #[serde(rename = "FILE")]
    File,
    /// A gallery.
    #[serde(rename = "GALLERY")]
    Gallery,
    /// A film.
    #[serde(rename = "MOVIE")]
    Movie,
    /// A music album.
    #[serde(rename = "MUSIC_ALBUM")]
    MusicAlbum,
    /// A music playlist.
    #[serde(rename = "MUSIC_PLAYLIST")]
    MusicPlaylist,
    /// A show.
    #[serde(rename = "SERIES")]
    Series,
    /// An upload session.
    #[serde(rename = "UPLOAD")]
    Upload,
    /// A user account.
    #[serde(rename = "USER")]
    User,
}

impl From<PermissionResourceKind> for ResourceKind {
    fn from(kind: PermissionResourceKind) -> Self {
        match kind {
            PermissionResourceKind::File => Self::File,
            PermissionResourceKind::Gallery => Self::Gallery,
            PermissionResourceKind::Movie => Self::Movie,
            PermissionResourceKind::MusicAlbum => Self::MusicAlbum,
            PermissionResourceKind::MusicPlaylist => Self::MusicPlaylist,
            PermissionResourceKind::Series => Self::Series,
            PermissionResourceKind::Upload => Self::Upload,
            PermissionResourceKind::User => Self::User,
        }
    }
}

impl From<ResourceKind> for PermissionResourceKind {
    fn from(kind: ResourceKind) -> Self {
        match kind {
            ResourceKind::File => Self::File,
            ResourceKind::Gallery => Self::Gallery,
            ResourceKind::Movie => Self::Movie,
            ResourceKind::MusicAlbum => Self::MusicAlbum,
            ResourceKind::MusicPlaylist => Self::MusicPlaylist,
            ResourceKind::Series => Self::Series,
            ResourceKind::Upload => Self::Upload,
            ResourceKind::User => Self::User,
        }
    }
}

/// Action a permission override names.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) enum PermissionAction {
    /// Replace the password behind a credential.
    #[serde(rename = "credential.patch")]
    CredentialPatch,
    /// Read a stored file.
    #[serde(rename = "file.read")]
    FileRead,
    /// Reference a stored file from a library resource.
    #[serde(rename = "file.use")]
    FileUse,
    /// Manage a gallery.
    #[serde(rename = "gallery.manage")]
    GalleryManage,
    /// Moderate gallery items.
    #[serde(rename = "gallery.moderate")]
    GalleryModerate,
    /// Read a gallery and its items.
    #[serde(rename = "gallery.read")]
    GalleryRead,
    /// Write a film, show or album catalogue item.
    #[serde(rename = "library.manage")]
    LibraryManage,
    /// Read the film, show and album catalogue.
    #[serde(rename = "library.read")]
    LibraryRead,
    /// Read, replace and reset permission overrides.
    #[serde(rename = "permission.manage")]
    PermissionManage,
    /// Manage a playlist.
    #[serde(rename = "playlist.manage")]
    PlaylistManage,
    /// Moderate playlist tracks.
    #[serde(rename = "playlist.moderate")]
    PlaylistModerate,
    /// Read a playlist and its tracks.
    #[serde(rename = "playlist.read")]
    PlaylistRead,
    /// Read an upload session.
    #[serde(rename = "upload.read")]
    UploadRead,
    /// Write an upload session.
    #[serde(rename = "upload.write")]
    UploadWrite,
    /// Create a Standard user account.
    #[serde(rename = "user.create")]
    UserCreate,
    /// Create an Admin user account.
    #[serde(rename = "user.create_admin")]
    UserCreateAdmin,
    /// Read a user account.
    #[serde(rename = "user.read")]
    UserRead,
    /// Change a user's role.
    #[serde(rename = "user.role_change")]
    UserRoleChange,
    /// Update a user account.
    #[serde(rename = "user.update")]
    UserUpdate,
}

impl From<PermissionAction> for Action {
    fn from(action: PermissionAction) -> Self {
        match action {
            PermissionAction::CredentialPatch => Self::CredentialPatch,
            PermissionAction::FileRead => Self::FileRead,
            PermissionAction::FileUse => Self::FileUse,
            PermissionAction::GalleryManage => Self::GalleryManage,
            PermissionAction::GalleryModerate => Self::GalleryModerate,
            PermissionAction::GalleryRead => Self::GalleryRead,
            PermissionAction::LibraryManage => Self::LibraryManage,
            PermissionAction::LibraryRead => Self::LibraryRead,
            PermissionAction::PermissionManage => Self::PermissionManage,
            PermissionAction::PlaylistManage => Self::PlaylistManage,
            PermissionAction::PlaylistModerate => Self::PlaylistModerate,
            PermissionAction::PlaylistRead => Self::PlaylistRead,
            PermissionAction::UploadRead => Self::UploadRead,
            PermissionAction::UploadWrite => Self::UploadWrite,
            PermissionAction::UserCreate => Self::UserCreate,
            PermissionAction::UserCreateAdmin => Self::UserCreateAdmin,
            PermissionAction::UserRead => Self::UserRead,
            PermissionAction::UserRoleChange => Self::UserRoleChange,
            PermissionAction::UserUpdate => Self::UserUpdate,
        }
    }
}

/// Scope a permission override requests.
#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) enum PermissionScope {
    /// Every aggregate.
    #[serde(rename = "ALL")]
    All,
    /// The named aggregates.
    #[serde(rename = "LISTED")]
    Listed,
    /// No aggregate.
    #[serde(rename = "NONE")]
    None,
    /// Aggregates the caller owns.
    #[serde(rename = "OWNED")]
    Owned,
    /// Public aggregates and the caller's own.
    #[serde(rename = "PUBLIC_OR_OWNED")]
    PublicOrOwned,
}

impl From<PermissionScope> for Scope {
    fn from(scope: PermissionScope) -> Self {
        match scope {
            PermissionScope::All => Self::All,
            PermissionScope::Listed => Self::Listed(Vec::new()),
            PermissionScope::None => Self::None,
            PermissionScope::Owned => Self::Owned,
            PermissionScope::PublicOrOwned => Self::PublicOrOwned,
        }
    }
}

/// One named aggregate of a `LISTED` scope in a permission representation.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PermissionResource {
    /// Identifier of the aggregate.
    pub id: NumericID,
    /// Kind of the aggregate (`GALLERY`, `MUSIC_PLAYLIST`, ...).
    pub kind: PermissionResourceKind,
}

/// One action's scope in a permission representation.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PermissionGrantResponse {
    /// Stored name of the action (`playlist.read`).
    pub action: String,
    /// Aggregates a `LISTED` scope names; empty otherwise.
    pub resources: Vec<PermissionResource>,
    /// The scope the action grants (`ALL`, `LISTED`, ...).
    pub scope: String,
}

/// Effective permissions of one user.
#[derive(Debug, Serialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PermissionResponse {
    /// Role presets per configurable action.
    pub defaults: Vec<PermissionGrantResponse>,
    /// Link to the permission resource itself.
    #[serde(rename = "_links")]
    pub links: SelfLinks,
    /// Stored overrides per action.
    pub overrides: Vec<PermissionGrantResponse>,
    /// Role the presets are read from.
    pub role: Role,
    /// Identifier of the user the permissions apply to.
    pub user_id: NumericID,
}

/// One requested override.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PermissionOverrideRequest {
    /// Action the override names (`playlist.read`).
    pub action: PermissionAction,
    /// Aggregates a `LISTED` scope names; empty otherwise.
    #[serde(default)]
    pub resources: Vec<PermissionResource>,
    /// Requested scope (`ALL`, `PUBLIC_OR_OWNED`, `OWNED`, `LISTED`, `NONE`).
    pub scope: PermissionScope,
}

/// Complete override set a user must end with.
#[derive(Debug, Deserialize, ToSchema)]
#[non_exhaustive]
pub(crate) struct PermissionRequest {
    /// Every override the user must hold.
    pub permissions: Vec<PermissionOverrideRequest>,
}

/// Map the permissions onto their HTTP representation.
impl From<GetPermissionResponseData> for PermissionResponse {
    fn from(response: GetPermissionResponseData) -> Self {
        let user_id = response.user_id();
        Self {
            defaults: response
                .defaults()
                .iter()
                .map(|preset| grant_response(preset.action(), preset.scope()))
                .collect(),
            overrides: response
                .overrides()
                .iter()
                .map(|permission| grant_response(permission.action(), permission.scope()))
                .collect(),
            role: response.role(),
            user_id,
            links: SelfLinks::new(&format!("/api/v1/user/{user_id}/permission")),
        }
    }
}

/// Validate one requested override and map it onto the application request.
impl TryFrom<PermissionOverrideRequest> for PermissionRequestData {
    type Error = ApiError;

    fn try_from(request: PermissionOverrideRequest) -> Result<Self, Self::Error> {
        let action = Action::from(request.action);
        let stored_scope = Scope::from(request.scope);
        let resolved_scope = match stored_scope {
            Scope::Listed(_) => {
                let references = request
                    .resources
                    .iter()
                    .map(|resource| {
                        ResourceRef::new(ResourceKind::from(resource.kind), resource.id)
                    })
                    .collect::<Vec<_>>();
                Scope::Listed(references)
            }
            Scope::All | Scope::None | Scope::Owned | Scope::PublicOrOwned => {
                if request.resources.is_empty() {
                    stored_scope
                } else {
                    return Err(ApiError::UnprocessableEntity(
                        "a non-listed scope must not name resources".to_owned(),
                    ));
                }
            }
        };
        Ok(Self::new(action, resolved_scope))
    }
}

/// Map an action and its scope onto their HTTP representation.
#[expect(
    clippy::pattern_type_mismatch,
    reason = "the scope owns its listed resources, so it is matched through a reference"
)]
fn grant_response(action: Action, scope: &Scope) -> PermissionGrantResponse {
    let resources = match scope {
        Scope::Listed(references) => references
            .iter()
            .map(|reference| PermissionResource {
                id: reference.id(),
                kind: PermissionResourceKind::from(reference.kind()),
            })
            .collect(),
        Scope::All | Scope::None | Scope::Owned | Scope::PublicOrOwned => Vec::new(),
    };
    PermissionGrantResponse {
        action: action.as_wire().to_owned(),
        resources,
        scope: scope.as_wire().to_owned(),
    }
}

/// Register the protected routes of the identity bounded context.
///
/// The handlers carry their own paths, so this only groups them; protection is
/// applied by [`identity_routes`].
pub(crate) fn identity_protected_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(post_register::post_register))
        .routes(routes!(patch_credential::patch_credential))
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
/// `login` is public; `register` and the credential password change require an
/// authenticated caller. The public `login` route is rate limited per client
/// address by the `limiter` the composition root built; the other routes are
/// not.
pub(crate) fn identity_routes(state: &AppState, limiter: &LoginRateLimiter) -> OpenApiRouter<()> {
    let protected = identity_protected_router().route_layer(middleware::from_fn_with_state(
        state.clone(),
        authenticate::<JwtTokenProvider>,
    ));

    protected
        .merge(limiter.apply(identity_public_router()))
        .with_state(state.clone())
}
