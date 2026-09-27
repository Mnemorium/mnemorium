use crate::domain::port::file_repository::FileRepository;
use crate::domain::port::mime_type_repository::MimeTypeRepository;
use crate::domain::port::unit_of_work::UnitOfWork;
use crate::domain::port::upload_repository::UploadRepository;

/// Asset bounded-context view over a [`UnitOfWork`].
pub trait AssetUnitOfWork: UnitOfWork {
    /// Repository persisting files.
    fn files(&mut self) -> impl FileRepository + '_;

    /// Repository querying mime types.
    fn mime_types(&mut self) -> impl MimeTypeRepository + '_;

    /// Repository persisting uploads.
    fn uploads(&mut self) -> impl UploadRepository + '_;
}
