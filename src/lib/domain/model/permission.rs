#![expect(
    clippy::arbitrary_source_item_ordering,
    reason = "the module keeps the scope algebra, the catalog and the aggregate next to the concept each documents"
)]

use crate::domain::alias::NumericID;
use crate::domain::model::user::Role;

/// Error returned when initialising a `Permission`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PermissionError {
    /// The action repeats inside the permission set.
    #[error("the action repeats inside the permission set")]
    DuplicateAction,
    /// The `LISTED` scope names the same resource more than once.
    #[error("a listed scope must not name the same resource twice")]
    DuplicateResource,
    /// The `LISTED` scope names no resource.
    #[error("a listed scope must name at least one resource")]
    EmptyListed,
    /// The action cannot be overridden.
    #[error("the action cannot be overridden")]
    NotConfigurable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// The `LISTED` scope names a resource kind the action does not target.
    #[error("the action does not target the listed resource kind")]
    UnsupportedResourceKind,
    /// The action does not support the requested scope.
    #[error("the action does not support the requested scope")]
    UnsupportedScope,
}

/// Kind of aggregate a `LISTED` scope names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceKind {
    /// A stored file.
    File,
    /// A gallery.
    Gallery,
    /// A film.
    Movie,
    /// A music album.
    MusicAlbum,
    /// A music playlist.
    MusicPlaylist,
    /// A show.
    Series,
    /// An upload session.
    Upload,
    /// A user account.
    User,
}

impl ResourceKind {
    /// Render the kind as its stored representation (`GALLERY`).
    ///
    /// The names are the values persisted in
    /// `permission_resource.resource_type`.
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::File => "FILE",
            Self::Gallery => "GALLERY",
            Self::Movie => "MOVIE",
            Self::MusicAlbum => "MUSIC_ALBUM",
            Self::MusicPlaylist => "MUSIC_PLAYLIST",
            Self::Series => "SERIES",
            Self::Upload => "UPLOAD",
            Self::User => "USER",
        }
    }

    /// Parse a kind from its stored representation (`GALLERY`).
    ///
    /// Returns `None` when `value` is not one of the persisted names.
    #[must_use]
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "FILE" => Some(Self::File),
            "GALLERY" => Some(Self::Gallery),
            "MOVIE" => Some(Self::Movie),
            "MUSIC_ALBUM" => Some(Self::MusicAlbum),
            "MUSIC_PLAYLIST" => Some(Self::MusicPlaylist),
            "SERIES" => Some(Self::Series),
            "UPLOAD" => Some(Self::Upload),
            "USER" => Some(Self::User),
            _ => None,
        }
    }
}

/// Reference to one aggregate a `LISTED` scope names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceRef {
    /// Identifier of the aggregate.
    id: NumericID,
    /// Kind of the aggregate.
    kind: ResourceKind,
}

impl ResourceRef {
    /// Initialise a new `ResourceRef`.
    #[must_use]
    pub fn new(kind: ResourceKind, id: NumericID) -> Self {
        Self { id, kind }
    }

    /// Return the identifier of the aggregate.
    #[must_use]
    pub fn id(self) -> NumericID {
        self.id
    }

    /// Return the kind of the aggregate.
    #[must_use]
    pub fn kind(self) -> ResourceKind {
        self.kind
    }
}

/// The part of a resource an authorization decision needs.
///
/// A collection-wide or creation action addresses no resource and uses
/// [`Global`] instead.
pub trait Protected {
    /// Return whether `self` is an account only the Root Admin may update.
    ///
    /// The relation guard refuses an administration action on another
    /// privileged account; it defaults to `false` because only a user is a
    /// privileged account.
    fn is_privileged_account(&self) -> bool {
        false
    }

    /// Return whether `self` is visible to every authenticated caller.
    fn is_publicly_visible(&self) -> bool;

    /// Return the identifier of the owning user, if any.
    fn owner_id(&self) -> Option<NumericID>;

    /// Return the reference a `LISTED` scope matches against, if any.
    fn resource_ref(&self) -> Option<ResourceRef>;
}

/// The resource an action addresses when it targets no single aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Global;

impl Protected for Global {
    fn is_privileged_account(&self) -> bool {
        false
    }

    fn is_publicly_visible(&self) -> bool {
        false
    }

    fn owner_id(&self) -> Option<NumericID> {
        None
    }

    fn resource_ref(&self) -> Option<ResourceRef> {
        None
    }
}

/// Every scope a content action supports.
pub const CONTENT_SCOPES: [&str; 5] = ["ALL", "LISTED", "NONE", "OWNED", "PUBLIC_OR_OWNED"];

/// The scopes a global or administration action supports.
pub const GLOBAL_SCOPES: [&str; 2] = ["ALL", "NONE"];

/// The only scope an upload action supports.
pub const OWNER_SCOPES: [&str; 1] = ["OWNED"];

/// Scope an action grants over a resource family.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Scope {
    /// Every resource of the action's family.
    All,
    /// Exactly the named aggregates.
    Listed(Vec<ResourceRef>),
    /// No resource.
    None,
    /// Only the caller's own resources.
    Owned,
    /// Public resources and the caller's own.
    PublicOrOwned,
}

impl Scope {
    /// Render the scope discriminant as its stored representation
    /// (`PUBLIC_OR_OWNED`).
    ///
    /// The names are the values the `chk_permission_scope` constraint admits.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the scope owns its listed resources, so its methods match a reference"
    )]
    pub fn as_wire(&self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Listed(_) => "LISTED",
            Self::None => "NONE",
            Self::Owned => "OWNED",
            Self::PublicOrOwned => "PUBLIC_OR_OWNED",
        }
    }

    /// Return whether `self` covers `resource` for the caller `caller_id`.
    #[must_use]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "the scope owns its listed resources, so its methods match a reference"
    )]
    pub fn covers<P: Protected + ?Sized>(&self, caller_id: NumericID, resource: &P) -> bool {
        match self {
            Self::All => true,
            Self::Listed(resources) => resource
                .resource_ref()
                .is_some_and(|reference| resources.contains(&reference)),
            Self::None => false,
            Self::Owned => Self::owns(caller_id, resource),
            Self::PublicOrOwned => {
                resource.is_publicly_visible() || Self::owns(caller_id, resource)
            }
        }
    }

    /// Parse a scope from its stored representation (`PUBLIC_OR_OWNED`).
    ///
    /// `LISTED` parses to an empty list: the named aggregates come from the
    /// child table, and [`Permission::try_new`] rejects the empty list.
    #[must_use]
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "ALL" => Some(Self::All),
            "LISTED" => Some(Self::Listed(Vec::new())),
            "NONE" => Some(Self::None),
            "OWNED" => Some(Self::Owned),
            "PUBLIC_OR_OWNED" => Some(Self::PublicOrOwned),
            _ => None,
        }
    }

    /// Return whether `self` is one of `supported`, comparing wire names.
    #[must_use]
    pub fn is_supported_by(&self, supported: &[&str]) -> bool {
        supported.contains(&self.as_wire())
    }

    /// Return whether `caller_id` owns `resource`.
    fn owns<P: Protected + ?Sized>(caller_id: NumericID, resource: &P) -> bool {
        resource.owner_id() == Some(caller_id)
    }
}

/// How the Root Admin reaches an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RootAccess {
    /// The Root Admin is always allowed.
    All,
    /// The Root Admin falls through to the role preset like every caller.
    OwnerOnly,
}

/// An authorizable operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Action {
    /// Read the film, show and album catalogue.
    CatalogRead,
    /// Replace the password behind a credential.
    CredentialPatch,
    /// Read a stored file.
    FileRead,
    /// Reference a stored file from a library resource.
    FileUse,
    /// Rename, change the visibility of, and delete a gallery.
    GalleryManage,
    /// Add and remove gallery items.
    GalleryModerate,
    /// Read a gallery and its items.
    GalleryRead,
    /// Read, replace and reset permission overrides.
    PermissionManage,
    /// Rename, change the visibility of, and delete a playlist.
    PlaylistManage,
    /// Add, remove and reorder playlist tracks.
    PlaylistModerate,
    /// Read a playlist and its tracks.
    PlaylistRead,
    /// Read an upload session.
    UploadRead,
    /// Write a chunk to an upload session and complete it.
    UploadWrite,
    /// Create a Standard user account.
    UserCreate,
    /// Create an Admin user account.
    UserCreateAdmin,
    /// Read a user account.
    UserRead,
    /// Change a user's role.
    UserRoleChange,
    /// Update a user account.
    UserUpdate,
}

/// Every action in the catalog.
pub const ACTIONS: [Action; 18] = [
    Action::CatalogRead,
    Action::CredentialPatch,
    Action::FileRead,
    Action::FileUse,
    Action::GalleryManage,
    Action::GalleryModerate,
    Action::GalleryRead,
    Action::PermissionManage,
    Action::PlaylistManage,
    Action::PlaylistModerate,
    Action::PlaylistRead,
    Action::UploadRead,
    Action::UploadWrite,
    Action::UserCreate,
    Action::UserCreateAdmin,
    Action::UserRead,
    Action::UserRoleChange,
    Action::UserUpdate,
];

impl Action {
    /// Render the action as its stored representation (`playlist.read`).
    ///
    /// The names are the values persisted in `permission.action` and the ones
    /// `docs/Permissions.md` documents.
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::CatalogRead => "catalog.read",
            Self::CredentialPatch => "credential.patch",
            Self::FileRead => "file.read",
            Self::FileUse => "file.use",
            Self::GalleryManage => "gallery.manage",
            Self::GalleryModerate => "gallery.moderate",
            Self::GalleryRead => "gallery.read",
            Self::PermissionManage => "permission.manage",
            Self::PlaylistManage => "playlist.manage",
            Self::PlaylistModerate => "playlist.moderate",
            Self::PlaylistRead => "playlist.read",
            Self::UploadRead => "upload.read",
            Self::UploadWrite => "upload.write",
            Self::UserCreate => "user.create",
            Self::UserCreateAdmin => "user.create_admin",
            Self::UserRead => "user.read",
            Self::UserRoleChange => "user.role_change",
            Self::UserUpdate => "user.update",
        }
    }

    /// Return whether the action's denial always conceals the resource.
    ///
    /// A read denial and an upload denial hide the resource; a management or
    /// moderation action conceals only when the caller cannot read the
    /// resource (`Authorization::decide` evaluates the matching read action).
    #[must_use]
    pub fn conceals_denial(self) -> bool {
        matches!(
            self,
            Self::CatalogRead
                | Self::FileRead
                | Self::GalleryRead
                | Self::PlaylistRead
                | Self::UploadRead
                | Self::UploadWrite
        )
    }

    /// Parse an action from its stored representation (`playlist.read`).
    ///
    /// Returns `None` when `value` is not one of the catalog's names.
    #[must_use]
    pub fn from_wire(value: &str) -> Option<Self> {
        ACTIONS.into_iter().find(|action| action.as_wire() == value)
    }

    /// Return whether a caller may override the action's scope.
    #[must_use]
    pub fn is_configurable(self) -> bool {
        !matches!(
            self,
            Self::PermissionManage | Self::UploadRead | Self::UploadWrite
        )
    }

    /// Return the read action guarding `self`, if any.
    #[must_use]
    pub fn read_action(self) -> Option<Self> {
        match self {
            Self::GalleryManage | Self::GalleryModerate => Some(Self::GalleryRead),
            Self::PlaylistManage | Self::PlaylistModerate => Some(Self::PlaylistRead),
            Self::CatalogRead
            | Self::CredentialPatch
            | Self::FileRead
            | Self::FileUse
            | Self::GalleryRead
            | Self::PermissionManage
            | Self::PlaylistRead
            | Self::UploadRead
            | Self::UploadWrite
            | Self::UserCreate
            | Self::UserCreateAdmin
            | Self::UserRead
            | Self::UserRoleChange
            | Self::UserUpdate => None,
        }
    }

    /// Return the kinds a `LISTED` scope may name for the action.
    #[must_use]
    pub fn resource_kinds(self) -> &'static [ResourceKind] {
        match self {
            Self::CatalogRead => &[
                ResourceKind::Movie,
                ResourceKind::Series,
                ResourceKind::MusicAlbum,
            ],
            Self::CredentialPatch
            | Self::PermissionManage
            | Self::UserCreate
            | Self::UserCreateAdmin => &[],
            Self::FileRead | Self::FileUse => &[ResourceKind::File],
            Self::GalleryManage | Self::GalleryModerate | Self::GalleryRead => {
                &[ResourceKind::Gallery]
            }
            Self::PlaylistManage | Self::PlaylistModerate | Self::PlaylistRead => {
                &[ResourceKind::MusicPlaylist]
            }
            Self::UploadRead | Self::UploadWrite => &[ResourceKind::Upload],
            Self::UserRead | Self::UserRoleChange | Self::UserUpdate => &[ResourceKind::User],
        }
    }

    /// Return the scope a `role` obtains when the caller has no override.
    #[must_use]
    pub fn role_scope(self, role: Role) -> Scope {
        match self {
            Self::CatalogRead
            | Self::GalleryModerate
            | Self::GalleryRead
            | Self::PlaylistModerate
            | Self::PlaylistRead => Scope::PublicOrOwned,
            Self::CredentialPatch
            | Self::PermissionManage
            | Self::UserCreateAdmin
            | Self::UserRoleChange => Scope::None,
            Self::FileRead
            | Self::FileUse
            | Self::GalleryManage
            | Self::PlaylistManage
            | Self::UploadRead
            | Self::UploadWrite => Scope::Owned,
            Self::UserCreate | Self::UserRead | Self::UserUpdate => {
                if role == Role::Admin {
                    Scope::All
                } else {
                    Scope::None
                }
            }
        }
    }

    /// Return how the Root Admin reaches the action.
    #[must_use]
    pub fn root_access(self) -> RootAccess {
        match self {
            Self::UploadRead | Self::UploadWrite => RootAccess::OwnerOnly,
            Self::CatalogRead
            | Self::CredentialPatch
            | Self::FileRead
            | Self::FileUse
            | Self::GalleryManage
            | Self::GalleryModerate
            | Self::GalleryRead
            | Self::PermissionManage
            | Self::PlaylistManage
            | Self::PlaylistModerate
            | Self::PlaylistRead
            | Self::UserCreate
            | Self::UserCreateAdmin
            | Self::UserRead
            | Self::UserRoleChange
            | Self::UserUpdate => RootAccess::All,
        }
    }

    /// Return the scopes the action supports.
    #[must_use]
    pub fn supported_scopes(self) -> &'static [&'static str] {
        match self {
            Self::UploadRead | Self::UploadWrite => &OWNER_SCOPES,
            Self::CredentialPatch
            | Self::PermissionManage
            | Self::UserCreate
            | Self::UserCreateAdmin
            | Self::UserRead
            | Self::UserRoleChange
            | Self::UserUpdate => &GLOBAL_SCOPES,
            Self::CatalogRead
            | Self::FileRead
            | Self::FileUse
            | Self::GalleryManage
            | Self::GalleryModerate
            | Self::GalleryRead
            | Self::PlaylistManage
            | Self::PlaylistModerate
            | Self::PlaylistRead => &CONTENT_SCOPES,
        }
    }

    /// Return the event target the action names on a denial.
    #[must_use]
    pub fn target(self) -> &'static str {
        match self {
            Self::CatalogRead => "catalog",
            Self::CredentialPatch
            | Self::PermissionManage
            | Self::UserCreate
            | Self::UserCreateAdmin
            | Self::UserRead
            | Self::UserRoleChange
            | Self::UserUpdate => "user",
            Self::FileRead | Self::FileUse => "file",
            Self::GalleryManage | Self::GalleryModerate | Self::GalleryRead => "gallery",
            Self::PlaylistManage | Self::PlaylistModerate | Self::PlaylistRead => "playlist",
            Self::UploadRead | Self::UploadWrite => "upload",
        }
    }

    /// Return the event verb the action names on a denial.
    #[must_use]
    pub fn verb(self) -> &'static str {
        match self {
            Self::CatalogRead
            | Self::FileRead
            | Self::GalleryRead
            | Self::PlaylistRead
            | Self::UploadRead
            | Self::UserRead => "read",
            Self::CredentialPatch => "patch_credential",
            Self::FileUse => "use",
            Self::GalleryManage | Self::PermissionManage | Self::PlaylistManage => "manage",
            Self::GalleryModerate | Self::PlaylistModerate => "moderate",
            Self::UploadWrite => "write",
            Self::UserCreate | Self::UserCreateAdmin => "create",
            Self::UserRoleChange => "update_role",
            Self::UserUpdate => "update",
        }
    }
}

/// One per-user scope override of an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permission {
    /// The action whose default scope the permission overrides.
    action: Action,
    /// The scope the permission grants.
    scope: Scope,
    /// The user the permission applies to.
    user_id: NumericID,
}

impl Permission {
    /// Return the overridden action.
    #[must_use]
    pub fn action(&self) -> Action {
        self.action
    }

    /// Return the granted scope.
    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Initialise a new `Permission`, validating `scope` against `action`.
    ///
    /// # Errors
    ///
    /// Returns [`PermissionError::NotConfigurable`] when the action cannot be
    /// overridden, [`PermissionError::UnsupportedScope`] when the action does
    /// not support `scope`, [`PermissionError::EmptyListed`] when a `LISTED`
    /// scope names no resource, [`PermissionError::DuplicateResource`] when it
    /// repeats one, and [`PermissionError::UnsupportedResourceKind`] when it
    /// names a kind the action does not target.
    pub fn try_new(
        user_id: NumericID,
        action: Action,
        scope: Scope,
    ) -> Result<Self, PermissionError> {
        if !action.is_configurable() {
            return Err(PermissionError::NotConfigurable);
        }
        if !scope.is_supported_by(action.supported_scopes()) {
            return Err(PermissionError::UnsupportedScope);
        }
        let validated_scope = match scope {
            Scope::Listed(resources) => {
                if resources.is_empty() {
                    return Err(PermissionError::EmptyListed);
                }
                let mut seen = Vec::with_capacity(resources.len());
                for resource in &resources {
                    if !action.resource_kinds().contains(&resource.kind()) {
                        return Err(PermissionError::UnsupportedResourceKind);
                    }
                    if seen.contains(resource) {
                        return Err(PermissionError::DuplicateResource);
                    }
                    seen.push(*resource);
                }
                Scope::Listed(resources)
            }
            other @ (Scope::All | Scope::None | Scope::Owned | Scope::PublicOrOwned) => other,
        };
        Ok(Self {
            action,
            scope: validated_scope,
            user_id,
        })
    }

    /// Return the user the permission applies to.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }

    /// Validate that no action repeats in `permissions`.
    ///
    /// # Errors
    ///
    /// Returns [`PermissionError::DuplicateAction`] when two permissions name
    /// the same action.
    pub fn validate_set(permissions: &[Self]) -> Result<(), PermissionError> {
        let mut seen = Vec::with_capacity(permissions.len());
        for permission in permissions {
            if seen.contains(&permission.action()) {
                return Err(PermissionError::DuplicateAction);
            }
            seen.push(permission.action());
        }
        Ok(())
    }
}

/// One action's role preset scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionPreset {
    /// The action the preset describes.
    action: Action,
    /// The scope the role grants.
    scope: Scope,
}

impl PermissionPreset {
    /// Initialise a new `PermissionPreset`.
    #[must_use]
    pub fn new(action: Action, scope: Scope) -> Self {
        Self { action, scope }
    }

    /// Return the action the preset describes.
    #[must_use]
    pub fn action(&self) -> Action {
        self.action
    }

    /// Return the scope the role grants.
    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}
