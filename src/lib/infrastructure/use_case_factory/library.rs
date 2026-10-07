use std::sync::Arc;

use crate::application::port::add_gallery_item::AddGalleryItemUseCase;
use crate::application::port::create_gallery::CreateGalleryUseCase;
use crate::application::port::delete_gallery::DeleteGalleryUseCase;
use crate::application::port::delete_gallery_item::DeleteGalleryItemUseCase;
use crate::application::port::get_gallery::GetGalleryUseCase;
use crate::application::port::get_gallery_item::GetGalleryItemUseCase;
use crate::application::port::library_use_case_factory::LibraryUseCaseFactory;
use crate::application::port::list_galleries::ListGalleriesUseCase;
use crate::application::use_case::add_gallery_item::AddGalleryItem;
use crate::application::use_case::create_gallery::CreateGallery;
use crate::application::use_case::delete_gallery::DeleteGallery;
use crate::application::use_case::delete_gallery_item::DeleteGalleryItem;
use crate::application::use_case::get_gallery::GetGallery;
use crate::application::use_case::get_gallery_item::GetGalleryItem;
use crate::application::use_case::list_galleries::ListGalleries;
use crate::infrastructure::outbound::sqlx::unit_of_work::SqlxUnitOfWorkFactory;

/// Builds the Library use cases over the shared unit of work factory.
pub struct RuntimeLibraryUseCaseFactory {
    /// Factory opening the unit of work wrapping the Library use cases.
    unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>,
}

impl RuntimeLibraryUseCaseFactory {
    /// Create a new factory.
    #[must_use]
    pub fn new(unit_of_work_factory: Arc<SqlxUnitOfWorkFactory>) -> Self {
        Self {
            unit_of_work_factory,
        }
    }
}

impl LibraryUseCaseFactory for RuntimeLibraryUseCaseFactory {
    fn add_gallery_item(&self) -> Arc<dyn AddGalleryItemUseCase> {
        Arc::new(AddGalleryItem::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn create_gallery(&self) -> Arc<dyn CreateGalleryUseCase> {
        Arc::new(CreateGallery::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn delete_gallery(&self) -> Arc<dyn DeleteGalleryUseCase> {
        Arc::new(DeleteGallery::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn delete_gallery_item(&self) -> Arc<dyn DeleteGalleryItemUseCase> {
        Arc::new(DeleteGalleryItem::new(Arc::clone(
            &self.unit_of_work_factory,
        )))
    }

    fn get_gallery(&self) -> Arc<dyn GetGalleryUseCase> {
        Arc::new(GetGallery::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn get_gallery_item(&self) -> Arc<dyn GetGalleryItemUseCase> {
        Arc::new(GetGalleryItem::new(Arc::clone(&self.unit_of_work_factory)))
    }

    fn list_galleries(&self) -> Arc<dyn ListGalleriesUseCase> {
        Arc::new(ListGalleries::new(Arc::clone(&self.unit_of_work_factory)))
    }
}
