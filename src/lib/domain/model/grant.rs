use super::action::Action;
use super::principal::Principal;
use super::role::ResourceRole;
use super::user::Role;
use crate::domain::NumericID;

/// Effect of a grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum GrantEffect {
    /// The role's actions are permitted.
    Allow,
    /// The role's actions are refused, and a deny wins over every allow.
    Deny,
}

impl GrantEffect {
    /// Render the effect as its stored representation (`ALLOW`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Allow => "ALLOW",
            Self::Deny => "DENY",
        }
    }

    /// Parse an effect from its stored representation (`ALLOW`).
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "ALLOW" => Some(Self::Allow),
            "DENY" => Some(Self::Deny),
            _ => None,
        }
    }
}

/// Kind of child a `CreateChild` grant permits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ChildKind {
    /// The grant permits an Admin account; root-only and never stored.
    Admin,
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

impl ChildKind {
    /// Render the kind as its stored representation (`ALL`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Admin => "ADMIN",
            Self::All => "ALL",
            Self::Audio => "AUDIO",
            Self::Episode => "EPISODE",
            Self::File => "FILE",
            Self::Gallery => "GALLERY",
            Self::GalleryItem => "GALLERY_ITEM",
            Self::Image => "IMAGE",
            Self::Movie => "MOVIE",
            Self::MusicAlbum => "MUSIC_ALBUM",
            Self::MusicGroup => "MUSIC_GROUP",
            Self::MusicMedium => "MUSIC_MEDIUM",
            Self::MusicPlaylist => "MUSIC_PLAYLIST",
            Self::MusicRecording => "MUSIC_RECORDING",
            Self::Season => "SEASON",
            Self::Series => "SERIES",
            Self::Upload => "UPLOAD",
            Self::User => "USER",
            Self::Video => "VIDEO",
        }
    }

    /// Return whether the grant covers a request for `requested`.
    #[must_use]
    pub(crate) fn covers(self, requested: Self) -> bool {
        self == Self::All || self == requested
    }

    /// Parse a child kind from its stored representation (`ALL`).
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Some(match value {
            "ADMIN" => Self::Admin,
            "ALL" => Self::All,
            "AUDIO" => Self::Audio,
            "EPISODE" => Self::Episode,
            "FILE" => Self::File,
            "GALLERY" => Self::Gallery,
            "GALLERY_ITEM" => Self::GalleryItem,
            "IMAGE" => Self::Image,
            "MOVIE" => Self::Movie,
            "MUSIC_ALBUM" => Self::MusicAlbum,
            "MUSIC_GROUP" => Self::MusicGroup,
            "MUSIC_MEDIUM" => Self::MusicMedium,
            "MUSIC_PLAYLIST" => Self::MusicPlaylist,
            "MUSIC_RECORDING" => Self::MusicRecording,
            "SEASON" => Self::Season,
            "SERIES" => Self::Series,
            "UPLOAD" => Self::Upload,
            "USER" => Self::User,
            "VIDEO" => Self::Video,
            _ => return None,
        })
    }
}

/// Error returned when constructing or validating a grant.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GrantError {
    /// `ChildKind::Admin` is root-only and is never stored as a grant.
    #[error("an Admin child is not grantable")]
    AdminNotGrantable,
    /// `Owner` is implicit from ownership and is not a user grant.
    #[error("the owner role is not grantable to a user")]
    OwnerNotGrantable,
    /// The grant names the root, which holds no stored grant.
    #[error("the root holds no stored grant")]
    RootNotStorable,
}

/// One stored permission record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Grant {
    /// Kind of child the grant permits when its role includes `CreateChild`.
    child_kind: ChildKind,
    /// Whether the grant allows or denies its role's actions.
    effect: GrantEffect,
    /// Surrogate identifier of the stored row; `0` before it is stored.
    id: NumericID,
    /// The subject the grant applies to.
    principal: Principal,
    /// Identifier of the addressed resource.
    resource_id: NumericID,
    /// The resource role the grant carries.
    role: ResourceRole,
}

impl Grant {
    /// Return whether the grant applies to `caller`.
    #[must_use]
    pub(crate) fn applies_to(&self, caller_id: NumericID, caller_role: Role) -> bool {
        match self.principal {
            Principal::Role(role) => role == caller_role,
            Principal::User(id) => id == caller_id,
            Principal::Root => false,
        }
    }

    /// Return the kind of child the grant permits.
    #[must_use]
    pub(crate) fn child_kind(&self) -> ChildKind {
        self.child_kind
    }

    /// Return whether the grant allows or denies its role's actions.
    #[must_use]
    pub(crate) fn effect(&self) -> GrantEffect {
        self.effect
    }

    /// Initialise a stored grant from its persisted columns.
    #[must_use]
    pub(crate) fn from_stored(
        grant_id: NumericID,
        principal: Principal,
        resource_id: NumericID,
        role: ResourceRole,
        child_kind: ChildKind,
        effect: GrantEffect,
    ) -> Self {
        Self {
            child_kind,
            effect,
            id: grant_id,
            principal,
            resource_id,
            role,
        }
    }

    /// Return the surrogate identifier of the stored grant.
    ///
    /// `0` marks a grant that has not been stored yet.
    #[must_use]
    pub(crate) fn id(&self) -> NumericID {
        self.id
    }

    /// Return whether the grant applies to `action` for `child_kind`.
    ///
    /// The child kind matters only when the role includes `CreateChild`.
    #[must_use]
    pub(crate) fn matches(&self, action: Action, child_kind: Option<ChildKind>) -> bool {
        if !self.role.includes(action) {
            return false;
        }
        if action != Action::CreateChild {
            return true;
        }
        let requested = child_kind.unwrap_or(ChildKind::All);
        self.child_kind.covers(requested)
    }

    /// Return the subject the grant applies to.
    #[must_use]
    pub(crate) fn principal(&self) -> Principal {
        self.principal
    }

    /// Return the identifier of the addressed resource.
    #[must_use]
    pub(crate) fn resource_id(&self) -> NumericID {
        self.resource_id
    }

    /// Return the resource role the grant carries.
    #[must_use]
    pub(crate) fn role(&self) -> ResourceRole {
        self.role
    }

    /// Construct a grant, applying the storable-principal rules.
    pub(crate) fn try_new(
        principal: Principal,
        resource_id: NumericID,
        role: ResourceRole,
        child_kind: ChildKind,
        effect: GrantEffect,
    ) -> Result<Self, GrantError> {
        if principal.is_root() {
            return Err(GrantError::RootNotStorable);
        }
        if child_kind == ChildKind::Admin {
            return Err(GrantError::AdminNotGrantable);
        }
        if matches!(principal, Principal::User(_)) && !role.is_grantable_by_user() {
            return Err(GrantError::OwnerNotGrantable);
        }
        Ok(Self::from_stored(
            0,
            principal,
            resource_id,
            role,
            child_kind,
            effect,
        ))
    }
}
