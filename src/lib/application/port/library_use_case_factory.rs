use std::sync::Arc;

use crate::application::port::add_gallery_item::AddGalleryItemUseCase;
use crate::application::port::create_gallery::CreateGalleryUseCase;
use crate::application::port::delete_gallery::DeleteGalleryUseCase;
use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase;
use crate::application::port::get_album::GetAlbumUseCase;
use crate::application::port::get_album_medium::GetAlbumMediumUseCase;
use crate::application::port::get_album_track::GetAlbumTrackUseCase;
use crate::application::port::get_gallery::GetGalleryUseCase;
use crate::application::port::get_gallery_item::GetGalleryItemUseCase;
use crate::application::port::list_album_tracks::ListAlbumTracksUseCase;
use crate::application::port::list_albums::ListAlbumsUseCase;
use crate::application::port::list_galleries::ListGalleriesUseCase;

/// Builds the use cases of the Library bounded context on demand.
///
/// The inbound layer depends on this port instead of a pre-built use case, so
/// that every request gets a use case bound to the current dependencies.
#[cfg_attr(test, mockall::automock)]
pub trait LibraryUseCaseFactory: Send + Sync {
    /// Build the use case adding one of the caller's media to a gallery.
    fn add_gallery_item(&self) -> Arc<dyn AddGalleryItemUseCase>;

    /// Build the use case creating a gallery.
    fn create_gallery(&self) -> Arc<dyn CreateGalleryUseCase>;

    /// Build the use case deleting a gallery and its items.
    fn delete_gallery(&self) -> Arc<dyn DeleteGalleryUseCase>;

    /// Build the use case deleting one item of a gallery.
    fn delete_gallery_item(&self) -> Arc<dyn DeleteGalleryItemUseCase>;

    /// Build the use case fetching one music album and its mediums.
    fn get_album(&self) -> Arc<dyn GetAlbumUseCase>;

    /// Build the use case fetching one medium of a music album.
    fn get_album_medium(&self) -> Arc<dyn GetAlbumMediumUseCase>;

    /// Build the use case fetching one track of a music album medium.
    fn get_album_track(&self) -> Arc<dyn GetAlbumTrackUseCase>;

    /// Build the use case fetching one gallery and its items.
    fn get_gallery(&self) -> Arc<dyn GetGalleryUseCase>;

    /// Build the use case fetching one item of a gallery.
    fn get_gallery_item(&self) -> Arc<dyn GetGalleryItemUseCase>;

    /// Build the use case listing the tracks of a music album.
    fn list_album_tracks(&self) -> Arc<dyn ListAlbumTracksUseCase>;

    /// Build the use case listing the music albums the caller may see.
    fn list_albums(&self) -> Arc<dyn ListAlbumsUseCase>;

    /// Build the use case listing the galleries the caller may see.
    fn list_galleries(&self) -> Arc<dyn ListGalleriesUseCase>;
}
