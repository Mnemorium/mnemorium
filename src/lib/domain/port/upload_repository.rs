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

    /// Mark the upload identified by `id` finished under a guard on its pending
    /// state.
    ///
    /// Returns `Ok(Some(upload))` with the persisted upload when a pending,
    /// complete upload transitioned to finished. Returns `Ok(None)` when no
    /// upload carries the identifier: a missing upload is a valid outcome, not
    /// an error.
    ///
    /// Fails with [`RepositoryError::ConcurrencyConflict`] when an upload
    /// carries the identifier and is already finished, meaning another writer
    /// won the transition, and with [`RepositoryError::Conflict`] when it is
    /// not complete yet.
    fn finish(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Upload>, RepositoryError>> + Send;

    /// Record `chunk_number` as received for the upload identified by `id`.
    ///
    /// Recording an already recorded chunk is a no-op (idempotent). Fails with
    /// [`RepositoryError::Conflict`] when the upload is already finished: its
    /// chunks are sealed once the upload completes.
    fn record_chunk(
        &mut self,
        id: NumericID,
        chunk_number: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

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

    fn finish(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<Option<Upload>, RepositoryError>> + Send {
        (**self).finish(id)
    }

    fn record_chunk(
        &mut self,
        id: NumericID,
        chunk_number: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send {
        (**self).record_chunk(id, chunk_number)
    }

    fn search(
        &mut self,
        filter: &UploadFilter,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
