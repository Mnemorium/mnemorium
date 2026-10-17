use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;
use crate::domain::model::music_track::MusicTrack;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`MusicAlbumRepository::search`]. Every field is optional;
/// an all-`None` filter returns every album.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicAlbumFilter {
    /// Filter on the album identifier.
    pub id: Option<NumericID>,
    /// Filter on the album name.
    pub name: Option<String>,
}

/// Search filters for [`MusicAlbumRepository::search_tracks`]. Every field is
/// optional; an all-`None` filter returns every album track.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct MusicTrackFilter {
    /// Filter on the track identifier.
    pub id: Option<NumericID>,
    /// Filter on the identifier of the positioning medium.
    pub medium_id: Option<NumericID>,
}

/// Port for querying `MusicAlbum` and its tracks.
#[cfg_attr(test, mockall::automock)]
pub trait MusicAlbumRepository: Send + Sync {
    /// Search the albums matching `filter`, returned as `Vec<MusicAlbum>`.
    ///
    /// Each album carries its genres and its mediums with the recordings they
    /// hold in play order, so the caller reads the aggregate whole and never
    /// stitches a projection from per-album lookups.
    ///
    /// Returns an empty list when no album matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<Vec<MusicAlbum>, RepositoryError>> + Send;

    /// Search the tracks matching `filter`, returned as `Vec<MusicTrack>`.
    ///
    /// An album's tracks are ordered by ascending medium index, then ascending
    /// track index. Rows attached to a playlist rather than a medium are out of
    /// scope and never returned. A missing match is a valid outcome, not an
    /// error.
    fn search_tracks(
        &mut self,
        filter: &MusicTrackFilter,
    ) -> impl Future<Output = Result<Vec<MusicTrack>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MusicAlbumRepository + ?Sized> MusicAlbumRepository for &mut T {
    fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<Vec<MusicAlbum>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn search_tracks(
        &mut self,
        filter: &MusicTrackFilter,
    ) -> impl Future<Output = Result<Vec<MusicTrack>, RepositoryError>> + Send {
        (**self).search_tracks(filter)
    }
}
