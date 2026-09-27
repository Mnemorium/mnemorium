use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::upload::Upload;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`UploadRepository::search`]. Every field is optional;
/// an all-`None` filter returns every upload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct UploadFilter {
    /// Filter on the upload identifier.
    pub id: Option<NumericID>,
    /// Filter on the owning user identifier.
    pub user_id: Option<NumericID>,
}

/// Port for persisting and querying `Upload`.
#[cfg_attr(test, mockall::automock)]
pub trait UploadRepository: Send + Sync {
    /// Insert a new `upload`, returning the persisted upload with its final
    /// identifier.
    ///
    /// The identifier of `upload` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        upload: Upload,
    ) -> impl Future<Output = Result<Upload, RepositoryError>> + Send;

    /// Delete the upload identified by `id`.
    ///
    /// Returns `Ok(true)` when an upload matched `id` and was deleted, and
    /// `Ok(false)` when no upload matched. A missing upload is a valid outcome,
    /// not an error.
    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Update the upload targeted by its identifier using a compare-and-swap on
    /// its `version`.
    ///
    /// Fails with [`RepositoryError::ConcurrentModification`] when the stored
    /// version no longer matches the version carried by `upload`, meaning
    /// another writer won the race.
    fn save(
        &mut self,
        upload: Upload,
    ) -> impl Future<Output = Result<Upload, RepositoryError>> + Send;

    /// Search uploads matching `filter`, returned as `Vec<Upload>`.
    ///
    /// Returns an empty list when no upload matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &UploadFilter,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: UploadRepository + ?Sized> UploadRepository for &mut T {
    fn create(
        &mut self,
        upload: Upload,
    ) -> impl Future<Output = Result<Upload, RepositoryError>> + Send {
        (**self).create(upload)
    }

    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(id)
    }

    fn save(
        &mut self,
        upload: Upload,
    ) -> impl Future<Output = Result<Upload, RepositoryError>> + Send {
        (**self).save(upload)
    }

    fn search(
        &mut self,
        filter: &UploadFilter,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
