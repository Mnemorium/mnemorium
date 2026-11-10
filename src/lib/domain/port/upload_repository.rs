use std::future::Future;

use chrono::NaiveDateTime;

use crate::domain::NumericID;
use crate::domain::model::Upload;
use crate::domain::port::RepositoryError;

/// Search filters for [`UploadRepository::search`]. Every field is optional;
/// an all-`None` filter returns every upload.
///
/// The time fields bound a comparison rather than name an exact value:
/// `created_at` selects uploads created at or before it, and `sealed_at`
/// selects uploads sealed at or before it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UploadFilter {
    /// Filter on the creation instant.
    pub created_at: Option<NaiveDateTime>,
    /// Filter on whether the upload has finished.
    pub is_finished: Option<bool>,
    /// Filter on whether a completion is verifying the upload.
    pub is_verifying: Option<bool>,
    /// Filter on the verification seal instant.
    pub sealed_at: Option<NaiveDateTime>,
    /// Filter on the upload identifier.
    pub upload_id: Option<NumericID>,
    /// Filter on the owning user identifier.
    pub user_id: Option<NumericID>,
}

/// Port for persisting and querying `Upload`.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait UploadRepository: Send + Sync {
    /// Insert a new `upload`, returning the persisted upload with its final
    /// identifier.
    ///
    /// The identifier of `upload` is ignored: the repository assigns a fresh
    /// identity.
    fn create(
        &mut self,
        upload: Upload,
    ) -> impl Future<Output = Result<Upload, RepositoryError>> + Send;

    /// Delete every upload matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one upload was deleted, and `Ok(false)`
    /// when none matched. An all-`None` filter deletes every upload; chunk rows
    /// cascade.
    fn delete(
        &mut self,
        filter: &UploadFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Mark the upload identified by `id` finished under a guard on its pending
    /// state.
    ///
    /// Returns `Ok(Some(upload))` with the persisted upload when a complete,
    /// sealed upload transitioned to finished, and deletes its chunk rows in the
    /// same transaction. Returns `Ok(None)` when no upload carries the
    /// identifier: a missing upload is a valid outcome, not an error.
    ///
    /// Fails with [`RepositoryError::ConcurrencyConflict`] when an upload
    /// carries the identifier and is already finished, meaning another writer
    /// won the transition, and with [`RepositoryError::Conflict`] when it is
    /// not sealed or not complete yet.
    fn finish(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Upload>, RepositoryError>> + Send;

    /// Persist `upload` over the stored upload carrying its identifier,
    /// replacing its chunk rows with the bitmap it records and its verification
    /// seal.
    ///
    /// Returns the persisted upload. A missing target is reported as
    /// [`RepositoryError::OperationFailed`]: the upload lifecycle only updates
    /// a row it created in the same session.
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
        filter: &UploadFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn finish(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Upload>, RepositoryError>> + Send {
        (**self).finish(id)
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
