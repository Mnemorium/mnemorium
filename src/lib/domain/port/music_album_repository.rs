use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::music_album::MusicAlbum;
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

/// Port for querying `MusicAlbum` with the recordings its mediums sequence.
#[cfg_attr(test, mockall::automock)]
pub trait MusicAlbumRepository: Send + Sync {
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
}

#[cfg(test)]
impl<T: MusicAlbumRepository + ?Sized> MusicAlbumRepository for &mut T {
    fn search(
        &mut self,
        filter: &MusicAlbumFilter,
    ) -> impl Future<Output = Result<Vec<MusicAlbum>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
