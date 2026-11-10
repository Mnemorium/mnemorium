use crate::domain::port::{FileRepository, MimeTypeRepository, UnitOfWork, UploadRepository};

/// Asset bounded-context view over a [`UnitOfWork`].
pub(crate) trait AssetUnitOfWork: UnitOfWork {
    /// Repository persisting files.
    fn files(&mut self) -> impl FileRepository + '_;

    /// Repository querying mime types.
    fn mime_types(&mut self) -> impl MimeTypeRepository + '_;

    /// Repository persisting uploads.
    fn uploads(&mut self) -> impl UploadRepository + '_;
}
