use crate::domain::port::gallery_repository::GalleryRepository;
use crate::domain::port::image_repository::ImageRepository;
use crate::domain::port::music_album_repository::MusicAlbumRepository;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::video_repository::VideoRepository;

/// Library bounded-context view over a [`UnitOfWork`].
pub trait LibraryUnitOfWork: UnitOfWork {
    /// Repository persisting galleries and their items.
    fn galleries(&mut self) -> impl GalleryRepository + '_;

    /// Repository persisting images.
    fn images(&mut self) -> impl ImageRepository + '_;

    /// Repository querying music albums and their tracks.
    fn music_albums(&mut self) -> impl MusicAlbumRepository + '_;

    /// Repository persisting videos.
    fn videos(&mut self) -> impl VideoRepository + '_;
}
