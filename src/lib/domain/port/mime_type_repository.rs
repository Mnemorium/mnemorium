use std::future::Future;

use crate::domain::port::error::RepositoryError;

/// Port for querying mime types.
#[cfg_attr(test, mockall::automock)]
pub trait MimeTypeRepository: Send + Sync {
    /// Return every supported mime type identifier.
    ///
    /// The seeded `mime_type` table is the supported set a content-type
    /// sniffer's output is checked against: the sniffer reports what the
    /// content is, and the caller decides whether that type is supported.
    fn all_ids(&mut self) -> impl Future<Output = Result<Vec<String>, RepositoryError>> + Send;

    /// Return whether a mime type identified by `mime_type_id` exists.
    fn exists(
        &mut self,
        mime_type_id: &str,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MimeTypeRepository + ?Sized> MimeTypeRepository for &mut T {
    fn all_ids(&mut self) -> impl Future<Output = Result<Vec<String>, RepositoryError>> + Send {
        (**self).all_ids()
    }

    fn exists(
        &mut self,
        mime_type_id: &str,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).exists(mime_type_id)
    }
}
