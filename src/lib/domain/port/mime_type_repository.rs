use std::future::Future;

use crate::domain::port::error::RepositoryError;

/// Port for querying mime types.
#[cfg_attr(test, mockall::automock)]
pub trait MimeTypeRepository: Send + Sync {
    /// Return whether a mime type identified by `mime_type_id` exists.
    fn exists(
        &mut self,
        mime_type_id: &str,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MimeTypeRepository + ?Sized> MimeTypeRepository for &mut T {
    fn exists(
        &mut self,
        mime_type_id: &str,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).exists(mime_type_id)
    }
}
