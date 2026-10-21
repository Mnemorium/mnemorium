use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::music_playlist::MusicPlaylist;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MusicPlaylistRepository::search`]. Every field is
/// optional; an all-`None` filter returns every playlist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicPlaylistFilter {
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
pub trait MusicPlaylistRepository: Send + Sync {
    /// Append `recording_id` to the playlist identified by `playlist_id` at the
    /// next index, returning the hydrated occurrence.
    ///
    /// Returns `Ok(None)` when `recording_id` names no recording, so a missing
    /// recording is a valid outcome, not an error.
    fn add_track(
        &mut self,
        playlist_id: NumericID,
        recording_id: NumericID,
    ) -> impl Future<Output = Result<Option<MusicRecording>, RepositoryError>> + Send;

    /// Insert a new `playlist`, returning the persisted playlist with its final
    /// identifier.
    ///
    /// The identifier of `playlist` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        playlist: MusicPlaylist,
    ) -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send;

    /// Delete the playlist identified by `id`.
    ///
    /// Its track placements cascade. Returns `Ok(true)` when a playlist matched
    /// `id` and was deleted, and `Ok(false)` when no playlist matched. A missing
    /// playlist is a valid outcome, not an error.
    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Remove the placement at `track_index` from `playlist_id`.
    ///
    /// Returns `Ok(true)` when a placement matched the index and was deleted,
    /// and `Ok(false)` when no placement matched. The remaining indices are left
    /// unchanged, so a gap may remain.
    fn remove_track(
        &mut self,
        playlist_id: NumericID,
        track_index: u32,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Replace every placement of `playlist_id` with `order`, reindexing the
    /// tracks to the sequence's positions.
    ///
    /// Deleting and reinserting happens in one transaction.
    fn replace_tracks(
        &mut self,
        playlist_id: NumericID,
        order: &[(u32, NumericID)],
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Update an existing `playlist` targeted by its identifier, returning the
    /// persisted playlist with its hydrated tracks.
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
    fn add_track(
        &mut self,
        playlist_id: NumericID,
        recording_id: NumericID,
    ) -> impl Future<Output = Result<Option<MusicRecording>, RepositoryError>> + Send {
        (**self).add_track(playlist_id, recording_id)
    }

    fn create(
        &mut self,
        playlist: MusicPlaylist,
    ) -> impl Future<Output = Result<MusicPlaylist, RepositoryError>> + Send {
        (**self).create(playlist)
    }

    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(id)
    }

    fn remove_track(
        &mut self,
        playlist_id: NumericID,
        track_index: u32,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).remove_track(playlist_id, track_index)
    }

    fn replace_tracks(
        &mut self,
        playlist_id: NumericID,
        order: &[(u32, NumericID)],
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        (**self).replace_tracks(playlist_id, order)
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
