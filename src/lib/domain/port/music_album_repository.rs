use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;
use crate::domain::model::music_medium::MediumIndex;
use crate::domain::model::music_medium::MusicMedium;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MusicAlbumRepository::search`]. Every field is optional;
/// an all-`None` filter returns every album.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MusicAlbumFilter {
    /// Filter on the album identifier.
    pub id: Option<NumericID>,
    /// Filter on the identifier of the music group the album belongs to.
    pub music_group_id: Option<NumericID>,
    /// Filter on the album name.
    pub name: Option<String>,
}

/// Port for querying and mutating `MusicAlbum` with the recordings its mediums
/// sequence.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait MusicAlbumRepository: Send + Sync {
    /// Insert a new `album` and its genre links, returning the stored album with
    /// the identifier it was assigned.
    ///
    /// The identifier of `album` is ignored: the repository assigns a fresh
    /// identity. A duplicate `(name, music_group_id)` is reported as
    /// [`RepositoryError::AlreadyExist`]; a cover image the library does not own
    /// is reported as [`RepositoryError::Conflict`].
    fn create(
        &mut self,
        album: MusicAlbum,
    ) -> impl Future<Output = Result<MusicAlbum, RepositoryError>> + Send;

    /// Return whether `recording_id` currently holds a medium placement.
    ///
    /// A recording holds one medium placement at most, so the answer tells a
    /// placement apart from a move. A recording the catalogue does not know has
    /// no placement.
    fn has_medium_placement(
        &mut self,
        recording_id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Place `recording_id` at `(medium_index, track_index)` of `album_id`,
    /// removing the recording's previous medium placement in the same
    /// transaction.
    ///
    /// Returns the placed recording at its destination address, or `Ok(None)`
    /// when the recording, the album or the medium does not exist, so a missing
    /// target is a valid outcome, not an error. An occupied destination is
    /// reported as [`RepositoryError::Conflict`].
    fn place_track(
        &mut self,
        album_id: NumericID,
        medium_index: u8,
        track_index: u8,
        recording_id: NumericID,
    ) -> impl Future<Output = Result<Option<MusicRecording>, RepositoryError>> + Send;

    /// Place every `(medium_index, track_index, recording_id)` in `placements`
    /// after removing the named recordings' previous medium placements.
    ///
    /// The whole set is applied in one transaction, so a swap of two recordings
    /// does not collide with itself.
    fn place_tracks(
        &mut self,
        album_id: NumericID,
        placements: &[(u8, u8, NumericID)],
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Create or retype the medium at `medium.index()` of `album_id`, keeping the
    /// tracks it already sequences.
    ///
    /// Returns the stored medium, or `Ok(None)` when no album matches
    /// `album_id`; a missing album is a valid outcome, not an error.
    fn put_medium(
        &mut self,
        album_id: NumericID,
        medium: MediumIndex,
    ) -> impl Future<Output = Result<Option<MusicMedium>, RepositoryError>> + Send;

    /// Replace the medium structure of `album_id` with `mediums`.
    ///
    /// Every named medium is created or retyped; a stored medium outside the set
    /// is removed only when it sequences no recording, so an apply never
    /// discards a track placement.
    fn replace_mediums(
        &mut self,
        album_id: NumericID,
        mediums: &[MediumIndex],
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Persist the name and ISRC of `recording` over the stored recording
    /// carrying its identifier.
    ///
    /// Returns `Ok(None)` when no recording matches; a missing recording is a
    /// valid outcome, not an error. A duplicate ISRC is reported as
    /// [`RepositoryError::AlreadyExist`].
    fn save_recording(
        &mut self,
        recording: MusicRecording,
    ) -> impl Future<Output = Result<Option<MusicRecording>, RepositoryError>> + Send;

    /// Search the albums matching `filter`, returned as `Vec<MusicAlbum>`.
    ///
    /// Each album carries its genres and its mediums with the recordings they
    /// sequence in play order, so the caller reads the aggregate whole and never
    /// stitches a projection from per-medium lookups.
    ///
    /// Returns an empty list when no album matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<Vec<MusicAlbum>, RepositoryError>> + Send;

    /// Persist `album` over the stored album carrying its identifier, replacing
    /// its genre links.
    ///
    /// Returns `Ok(None)` when no album matches; a missing album is a valid
    /// outcome, not an error. A duplicate `(name, music_group_id)` is reported as
    /// [`RepositoryError::AlreadyExist`].
    fn update(
        &mut self,
        album: MusicAlbum,
    ) -> impl Future<Output = Result<Option<MusicAlbum>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MusicAlbumRepository + ?Sized> MusicAlbumRepository for &mut T {
    fn create(
        &mut self,
        album: MusicAlbum,
    ) -> impl Future<Output = Result<MusicAlbum, RepositoryError>> + Send {
        (**self).create(album)
    }

    fn has_medium_placement(
        &mut self,
        recording_id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).has_medium_placement(recording_id)
    }

    fn place_track(
        &mut self,
        album_id: NumericID,
        medium_index: u8,
        track_index: u8,
        recording_id: NumericID,
    ) -> impl Future<Output = Result<Option<MusicRecording>, RepositoryError>> + Send {
        (**self).place_track(album_id, medium_index, track_index, recording_id)
    }

    fn place_tracks(
        &mut self,
        album_id: NumericID,
        placements: &[(u8, u8, NumericID)],
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        (**self).place_tracks(album_id, placements)
    }

    fn put_medium(
        &mut self,
        album_id: NumericID,
        medium: MediumIndex,
    ) -> impl Future<Output = Result<Option<MusicMedium>, RepositoryError>> + Send {
        (**self).put_medium(album_id, medium)
    }

    fn replace_mediums(
        &mut self,
        album_id: NumericID,
        mediums: &[MediumIndex],
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        (**self).replace_mediums(album_id, mediums)
    }

    fn save_recording(
        &mut self,
        recording: MusicRecording,
    ) -> impl Future<Output = Result<Option<MusicRecording>, RepositoryError>> + Send {
        (**self).save_recording(recording)
    }

    fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<Vec<MusicAlbum>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn update(
        &mut self,
        album: MusicAlbum,
    ) -> impl Future<Output = Result<Option<MusicAlbum>, RepositoryError>> + Send {
        (**self).update(album)
    }
}
