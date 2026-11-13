use super::grant::Grant;
use crate::domain::NumericID;

/// Maximum number of authorization-parent edges a decision may walk.
pub(crate) const MAX_AUTHORIZATION_DEPTH: usize = 32;

/// Category of a resource, used to address it and constrain a `CreateChild`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ResourceKind {
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

impl ResourceKind {
    /// Render the kind as its stored representation (`GALLERY`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Audio => "AUDIO",
            Self::Catalogue => "CATALOGUE",
            Self::Episode => "EPISODE",
            Self::File => "FILE",
            Self::Gallery => "GALLERY",
            Self::GalleryCollection => "GALLERY_COLLECTION",
            Self::GalleryItem => "GALLERY_ITEM",
            Self::Image => "IMAGE",
            Self::Instance => "INSTANCE",
            Self::Library => "LIBRARY",
            Self::Movie => "MOVIE",
            Self::MusicAlbum => "MUSIC_ALBUM",
            Self::MusicGroup => "MUSIC_GROUP",
            Self::MusicMedium => "MUSIC_MEDIUM",
            Self::MusicPlaylist => "MUSIC_PLAYLIST",
            Self::MusicRecording => "MUSIC_RECORDING",
            Self::PlaylistCollection => "PLAYLIST_COLLECTION",
            Self::Season => "SEASON",
            Self::Series => "SERIES",
            Self::Upload => "UPLOAD",
            Self::UploadCollection => "UPLOAD_COLLECTION",
            Self::User => "USER",
            Self::UserCollection => "USER_COLLECTION",
            Self::Video => "VIDEO",
        }
    }

    /// Parse a kind from its stored representation (`GALLERY`).
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Some(match value {
            "AUDIO" => Self::Audio,
            "CATALOGUE" => Self::Catalogue,
            "EPISODE" => Self::Episode,
            "FILE" => Self::File,
            "GALLERY" => Self::Gallery,
            "GALLERY_COLLECTION" => Self::GalleryCollection,
            "GALLERY_ITEM" => Self::GalleryItem,
            "IMAGE" => Self::Image,
            "INSTANCE" => Self::Instance,
            "LIBRARY" => Self::Library,
            "MOVIE" => Self::Movie,
            "MUSIC_ALBUM" => Self::MusicAlbum,
            "MUSIC_GROUP" => Self::MusicGroup,
            "MUSIC_MEDIUM" => Self::MusicMedium,
            "MUSIC_PLAYLIST" => Self::MusicPlaylist,
            "MUSIC_RECORDING" => Self::MusicRecording,
            "PLAYLIST_COLLECTION" => Self::PlaylistCollection,
            "SEASON" => Self::Season,
            "SERIES" => Self::Series,
            "UPLOAD" => Self::Upload,
            "UPLOAD_COLLECTION" => Self::UploadCollection,
            "USER" => Self::User,
            "USER_COLLECTION" => Self::UserCollection,
            "VIDEO" => Self::Video,
            _ => return None,
        })
    }
}

/// Coarse resource-level grant carried by a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum Visibility {
    /// Adds no implicit grant.
    Private,
    /// Grants the viewer actions to every authenticated caller.
    Public,
    /// Grants discovery and metadata to every authenticated caller.
    Restricted,
}

impl Visibility {
    /// Render the visibility as its stored representation (`PRIVATE`).
    #[must_use]
    pub(crate) fn as_wire(self) -> &'static str {
        match self {
            Self::Private => "PRIVATE",
            Self::Restricted => "RESTRICTED",
            Self::Public => "PUBLIC",
        }
    }

    /// Parse a visibility from its stored representation (`PRIVATE`).
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "PRIVATE" => Some(Self::Private),
            "RESTRICTED" => Some(Self::Restricted),
            "PUBLIC" => Some(Self::Public),
            _ => None,
        }
    }
}

/// Kind of a containment or reference edge between two resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub(crate) enum ResourceRelationKind {
    /// A recording placed on an album's medium.
    AlbumMember,
    /// An album placed in a music group.
    GroupMember,
    /// A recording placed in a playlist.
    PlaylistEntry,
}

impl ResourceRelationKind {
    /// Parse a relation from its stored representation (`ALBUM_MEMBER`).
    #[must_use]
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "ALBUM_MEMBER" => Some(Self::AlbumMember),
            "PLAYLIST_ENTRY" => Some(Self::PlaylistEntry),
            "GROUP_MEMBER" => Some(Self::GroupMember),
            _ => None,
        }
    }
}

/// External address of a resource: its kind and the identifier of its entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ResourceRef {
    /// Identifier of the addressed entity.
    id: NumericID,
    /// Kind of the addressed entity.
    kind: ResourceKind,
}

impl ResourceRef {
    /// Return the identifier of the addressed entity.
    #[must_use]
    pub(crate) fn id(self) -> NumericID {
        self.id
    }

    /// Return the kind of the addressed entity.
    #[must_use]
    pub(crate) fn kind(self) -> ResourceKind {
        self.kind
    }

    /// Initialise a new `ResourceRef`.
    #[must_use]
    pub(crate) fn new(kind: ResourceKind, id: NumericID) -> Self {
        Self { id, kind }
    }
}

/// One authorizable entity in the resource graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resource {
    /// Identifier of the authorization parent, when one is assigned.
    auth_parent_id: Option<NumericID>,
    /// Identifier of the entity in its own table.
    entity_id: NumericID,
    /// Surrogate identifier of the resource row.
    id: NumericID,
    /// Kind of the resource.
    kind: ResourceKind,
    /// Identifier of the owning user, absent when the resource is unowned.
    owner_id: Option<NumericID>,
    /// Coarse resource-level grant.
    visibility: Visibility,
}

impl Resource {
    /// Return the identifier of the authorization parent, when one is assigned.
    #[must_use]
    pub(crate) fn auth_parent_id(&self) -> Option<NumericID> {
        self.auth_parent_id
    }

    /// Return the identifier of the entity in its own table.
    #[must_use]
    pub(crate) fn entity_id(&self) -> NumericID {
        self.entity_id
    }

    /// Initialise a stored resource from its persisted columns.
    #[must_use]
    pub(crate) fn from_stored(
        resource_id: NumericID,
        kind: ResourceKind,
        entity_id: NumericID,
        owner_id: Option<NumericID>,
        auth_parent_id: Option<NumericID>,
        visibility: Visibility,
    ) -> Self {
        Self {
            auth_parent_id,
            entity_id,
            kind,
            id: resource_id,
            owner_id,
            visibility,
        }
    }

    /// Return the kind of the resource.
    #[must_use]
    pub(crate) fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Return the identifier of the owning user, when one exists.
    #[must_use]
    pub(crate) fn owner_id(&self) -> Option<NumericID> {
        self.owner_id
    }

    /// Return the surrogate identifier of the resource row.
    #[must_use]
    pub(crate) fn resource_id(&self) -> NumericID {
        self.id
    }

    /// Return the resource's visibility.
    #[must_use]
    pub(crate) fn visibility(&self) -> Visibility {
        self.visibility
    }

    /// Return the same resource carrying `visibility`.
    #[must_use]
    pub(crate) fn with_visibility(&self, visibility: Visibility) -> Self {
        Self {
            visibility,
            ..self.clone()
        }
    }
}

/// The resource a decision addresses together with everything it needs.
///
/// One read transaction loads the addressed resource, its authorization-parent
/// ancestors, the matching grants and the relation containers that reference
/// it. The decision engine receives this value and performs no I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResourceAccess {
    /// Authorization-parent ancestors, nearest first, excluding the resource.
    ancestors: Vec<Resource>,
    /// Grants matching the caller's user and role on the resource or an
    /// ancestor.
    grants: Vec<Grant>,
    /// Relation containers that reference the addressed resource.
    relations: Vec<RelationSource>,
    /// The addressed resource.
    resource: Resource,
}

impl ResourceAccess {
    /// Return the authorization-parent ancestors, nearest first.
    #[must_use]
    pub(crate) fn ancestors(&self) -> &[Resource] {
        &self.ancestors
    }

    /// Return the grants matching the caller on the resource or an ancestor.
    #[must_use]
    pub(crate) fn grants(&self) -> &[Grant] {
        &self.grants
    }

    /// Initialise a new `ResourceAccess`.
    #[must_use]
    pub(crate) fn new(
        resource: Resource,
        ancestors: Vec<Resource>,
        grants: Vec<Grant>,
        relations: Vec<RelationSource>,
    ) -> Self {
        Self {
            ancestors,
            grants,
            relations,
            resource,
        }
    }

    /// Return the relation containers that reference the resource.
    #[must_use]
    pub(crate) fn relations(&self) -> &[RelationSource] {
        &self.relations
    }

    /// Return the addressed resource.
    #[must_use]
    pub(crate) fn resource(&self) -> &Resource {
        &self.resource
    }
}

/// A container that references the addressed resource through a relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationSource {
    /// Authorization-parent ancestors of the container, nearest first.
    ancestors: Vec<Resource>,
    /// The container resource.
    container: Resource,
    /// Grants matching the caller's user and role on the container or an
    /// ancestor.
    grants: Vec<Grant>,
}

impl RelationSource {
    /// Return the authorization-parent ancestors of the container.
    #[must_use]
    pub(crate) fn ancestors(&self) -> &[Resource] {
        &self.ancestors
    }

    /// Return the container resource.
    #[must_use]
    pub(crate) fn container(&self) -> &Resource {
        &self.container
    }

    /// Return the grants matching the caller on the container or an ancestor.
    #[must_use]
    pub(crate) fn grants(&self) -> &[Grant] {
        &self.grants
    }

    /// Initialise a new `RelationSource`.
    #[must_use]
    pub(crate) fn new(container: Resource, ancestors: Vec<Resource>, grants: Vec<Grant>) -> Self {
        Self {
            ancestors,
            container,
            grants,
        }
    }
}
