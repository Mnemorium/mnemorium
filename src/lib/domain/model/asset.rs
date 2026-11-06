use crate::domain::alias::NumericID;
use crate::domain::model::permission::{ResourceKind, ResourceRef};

/// The resource that owns a stored asset, as far as authorization is concerned.
///
/// An image or video carries a [`crate::domain::model::scope::Scope`] fixed by
/// its first attachment. A `LIBRARY` asset is shared catalogue art, while a
/// `USER` asset is owned by exactly one referrer: a gallery item or a playlist
/// cover. Reading the asset delegates to the read action of that owner, so the
/// asset is exactly as readable as the resource it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum AssetContext {
    /// Media of one gallery, authorized through `gallery.read`.
    Gallery(ResourceRef),
    /// Shared catalogue art, authorized through `library.read`.
    Library,
    /// Artwork of one playlist, authorized through `playlist.read`.
    Playlist(ResourceRef),
}

impl AssetContext {
    /// Return the gallery context naming `gallery_id`.
    #[must_use]
    pub(crate) fn gallery(gallery_id: NumericID) -> Self {
        Self::Gallery(ResourceRef::new(ResourceKind::Gallery, gallery_id))
    }

    /// Return the playlist context naming `playlist_id`.
    #[must_use]
    pub(crate) fn playlist(playlist_id: NumericID) -> Self {
        Self::Playlist(ResourceRef::new(ResourceKind::MusicPlaylist, playlist_id))
    }

    /// Return the reference the context names, absent for shared catalogue art.
    #[must_use]
    #[expect(
        dead_code,
        reason = "part of the type's declared contract; no caller exercises it yet"
    )]
    pub(crate) fn resource_ref(self) -> Option<ResourceRef> {
        match self {
            Self::Library => None,
            Self::Gallery(reference) | Self::Playlist(reference) => Some(reference),
        }
    }
}
