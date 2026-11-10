use std::future::Future;

use crate::domain::model::MimeType;
use crate::domain::port::RepositoryError;

/// Search filters for [`MimeTypeRepository::search`]. Every field is optional;
/// an all-`None` filter returns every supported mime type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct MimeTypeFilter {
    /// Filter on the mime type identifier.
    pub mime_type_id: Option<String>,
}

/// Port for querying mime types.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait MimeTypeRepository: Send + Sync {
    /// Search the supported mime types matching `filter`, returned as
    /// `Vec<MimeType>`.
    ///
    /// The seeded `mime_type` table is the supported set a content-type
    /// sniffer's output is checked against: the sniffer reports what the
    /// content is, and the caller decides whether that type is supported by
    /// whether the search found it.
    ///
    /// Returns an empty list when no mime type matches; a missing match is a
    /// valid outcome, not an error.
    fn search(
        &mut self,
        filter: &MimeTypeFilter,
    ) -> impl Future<Output = Result<Vec<MimeType>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: MimeTypeRepository + ?Sized> MimeTypeRepository for &mut T {
    fn search(
        &mut self,
        filter: &MimeTypeFilter,
    ) -> impl Future<Output = Result<Vec<MimeType>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
