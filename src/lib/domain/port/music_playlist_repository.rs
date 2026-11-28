use std::future::Future;

use crate::domain::NumericID;
use crate::domain::model::MusicPlaylist;
use crate::domain::port::RepositoryError;

/// Search filters for [`MusicPlaylistRepository::search`]. Every field is
/// optional; an all-`None` filter returns every playlist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MusicPlaylistFilter {
    /// Filter on the identifier of the image holding the playlist's artwork.
    pub cover_image_id: Option<NumericID>,
    /// Filter on the playlist identifier.
    pub id: Option<NumericID>,
    /// Filter on whether the playlist is public.
    pub is_public: Option<bool>,
    /// Filter on the playlist name.
    pub name: Option<String>,
    /// Filter on the identifier of the owning user.
    pub user_id: Option<NumericID>,
}

/// Port for persisting and querying `MusicPlaylist` and the recordings it
/// sequences.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait MusicPlaylistRepository: Send + Sync {
    /// Insert a new `playlist`, returning the persisted playlist with its final
    /// identifier.
    ///
    /// The identifier of `playlist` is ignored: the repository assigns a fresh
    /// identity. Its track placements are written by [`Self::save`].
    fn create(
        &mut self,
        playlist: MusicPlaylist,
    ) -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send;

    /// Delete every playlist matching `filter`.
    ///
    /// Its track placements cascade. Returns `Ok(true)` when at least one
    /// playlist was deleted, and `Ok(false)` when none matched. An all-`None`
    /// filter deletes every playlist.
    fn delete(
        &mut self,
        filter: &MusicPlaylistFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Update an existing `playlist` targeted by its identifier, replacing its
    /// track placements with `playlist.tracks()` in order.
    ///
    /// Returns the persisted playlist with its hydrated tracks.
    fn save(
        &mut self,
        playlist: MusicPlaylist,
    ) -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send;

    /// Search the playlists matching `filter`, returned as
    /// `Vec<MusicPlaylist>`.
    ///
    /// Each playlist carries its ordered recording occurrences, so the caller
    /// reads the aggregate whole. Returns an empty list when no playlist
    /// matches; a missing match is a valid outcome, not an error.
    fn search(
        &mut self,
        filter: &MusicPlaylistFilter,
    ) -> impl Future<Output = Result<Vec<MusicPlaylist>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MusicPlaylistRepository + ?Sized> MusicPlaylistRepository for &mut T {
    fn create(
        &mut self,
        playlist: MusicPlaylist,
    ) -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send {
        (**self).create(playlist)
    }

    fn delete(
        &mut self,
        filter: &MusicPlaylistFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }
    fn save(
        &mut self,
        playlist: MusicPlaylist,
    ) -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send {
        (**self).save(playlist)
    }

    fn search(
        &mut self,
        filter: &MusicPlaylistFilter,
    ) -> impl Future<Output = Result<Vec<MusicPlaylist>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
