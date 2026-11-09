use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;
use crate::domain::model::music_medium::MusicMedium;
use crate::domain::model::music_recording::MusicRecording;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MusicAlbumRepository::search`]. Every field is optional;
/// an all-`None` filter returns every album.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MusicAlbumFilter {
    /// Filter on the identifier of an audio sequenced by one of the album's
    /// recordings. This is a join constraint rather than an album field.
    pub audio_id: Option<NumericID>,
    /// Filter on the album identifier.
    pub id: Option<NumericID>,
    /// Filter on the identifier of the music group the album belongs to.
    pub music_group_id: Option<NumericID>,
    /// Filter on the album name.
    pub name: Option<String>,
}

/// Search filters for [`MusicAlbumRepository::search_recordings`]. Every field
/// is optional; an all-`None` filter returns every recording.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MusicRecordingFilter {
    /// Filter on the identifier of the audio backing the recording.
    pub audio_id: Option<NumericID>,
    /// Filter on the recording identifier.
    pub id: Option<NumericID>,
    /// Filter on the recording ISRC code.
    pub isrc_code: Option<String>,
    /// Filter on the recording name.
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

    /// Delete every album matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one album was deleted, and `Ok(false)`
    /// when none matched. An all-`None` filter deletes every album; mediums,
    /// recordings and track placements cascade.
    #[expect(
        dead_code,
        reason = "part of the type's declared contract; no caller exercises it yet"
    )]
    fn delete(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Delete every recording matching `filter`, its links and every track
    /// placement of it.
    ///
    /// Returns `Ok(true)` when at least one recording was deleted, and
    /// `Ok(false)` when none matched. An all-`None` filter deletes every
    /// recording.
    fn delete_recording(
        &mut self,
        filter: &MusicRecordingFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Persist `album` over the stored album carrying its identifier, replacing
    /// its genre links, mediums and track placements.
    ///
    /// The album is inserted when no row carries its identifier. A duplicate
    /// `(name, music_group_id)` is reported as [`RepositoryError::AlreadyExist`].
    fn save(
        &mut self,
        album: MusicAlbum,
    ) -> impl Future<Output = Result<MusicAlbum, RepositoryError>> + Send;

    /// Create or retype the medium at `medium.index()` of `album_id`, replacing
    /// the recordings it sequences with `medium.tracks()`.
    ///
    /// Returns the stored medium, or `Ok(None)` when no album matches
    /// `album_id`; a missing album is a valid outcome, not an error.
    fn save_medium(
        &mut self,
        album_id: NumericID,
        medium: MusicMedium,
    ) -> impl Future<Output = Result<Option<MusicMedium>, RepositoryError>> + Send;

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

    /// Search the recordings matching `filter`, returned as `Vec<MusicRecording>`.
    ///
    /// Returns an empty list when no recording matches; a missing match is a
    /// valid outcome, not an error.
    fn search_recordings(
        &mut self,
        filter: &MusicRecordingFilter,
    ) -> impl Future<Output = Result<Vec<MusicRecording>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MusicAlbumRepository + ?Sized> MusicAlbumRepository for &mut T {
    fn create(
        &mut self,
        album: MusicAlbum,
    ) -> impl Future<Output = Result<MusicAlbum, RepositoryError>> + Send {
        (**self).create(album)
    }

    fn delete(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn delete_recording(
        &mut self,
        filter: &MusicRecordingFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete_recording(filter)
    }

    fn save(
        &mut self,
        album: MusicAlbum,
    ) -> impl Future<Output = Result<MusicAlbum, RepositoryError>> + Send {
        (**self).save(album)
    }

    fn save_medium(
        &mut self,
        album_id: NumericID,
        medium: MusicMedium,
    ) -> impl Future<Output = Result<Option<MusicMedium>, RepositoryError>> + Send {
        (**self).save_medium(album_id, medium)
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

    fn search_recordings(
        &mut self,
        filter: &MusicRecordingFilter,
    ) -> impl Future<Output = Result<Vec<MusicRecording>, RepositoryError>> + Send {
        (**self).search_recordings(filter)
    }
}
