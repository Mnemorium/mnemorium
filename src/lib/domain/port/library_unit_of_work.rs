use crate::domain::port::gallery_repository::GalleryRepository;
use crate::domain::port::media_repository::MediaRepository;
use crate::domain::port::unit_of_work::UnitOfWork;

/// Library bounded-context view over a [`UnitOfWork`].
pub trait LibraryUnitOfWork: UnitOfWork {
    /// Repository persisting galleries and their items.
    fn galleries(&mut self) -> impl GalleryRepository + '_;

    /// Repository persisting media.
    fn media(&mut self) -> impl MediaRepository + '_;
}
