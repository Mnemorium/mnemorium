use std::future::Future;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::upload::Upload;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`UploadRepository::search`]. Every field is optional;
/// an all-`None` filter returns every upload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UploadFilter {
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

    /// Delete the upload identified by `id`.
    ///
    /// Returns `Ok(true)` when an upload matched `id` and was deleted, and
    /// `Ok(false)` when no upload matched. A missing upload is a valid outcome,
    /// not an error.
    fn delete(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Delete every recorded chunk row of the upload identified by `id`.
    ///
    /// Returns `Ok(true)` when at least one chunk row was deleted, and
    /// `Ok(false)` when the upload recorded none. A missing upload is a valid
    /// outcome, not an error.
    fn delete_chunks(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Return pending, unsealed uploads created before `instant`, oldest first,
    /// at most `limit` of them.
    ///
    /// Returns an empty list when nothing matches; a missing match is a valid
    /// outcome, not an error.
    fn expired(
        &mut self,
        instant: NaiveDateTime,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send;

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

    /// Record `chunk_number` as received for the upload identified by `id`.
    ///
    /// Recording an already recorded chunk is a no-op (idempotent). Fails with
    /// [`RepositoryError::Conflict`] when the upload is finished or sealed: its
    /// chunks are sealed once a completion verifies it.
    fn record_chunk(
        &mut self,
        id: NumericID,
        chunk_number: u64,
    ) -> impl Future<Output = Result<(), RepositoryError>> + Send;

    /// Seal the upload identified by `id` for verification at `sealed_at`.
    ///
    /// Returns `Ok(true)` when a pending, unsealed upload transitioned to
    /// sealed, and `Ok(false)` when the guard matched no row: the upload is
    /// missing, is already finished, or another verification holds the seal. A
    /// missing upload is a valid outcome, not an error.
    fn seal(
        &mut self,
        id: NumericID,
        sealed_at: NaiveDateTime,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Search uploads matching `filter`, returned as `Vec<Upload>`.
    ///
    /// Returns an empty list when no upload matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &UploadFilter,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send;

    /// Return uploads whose verification seal started before `instant`, at most
    /// `limit` of them.
    ///
    /// Returns an empty list when nothing matches; a missing match is a valid
    /// outcome, not an error.
    fn stale_seals(
        &mut self,
        instant: NaiveDateTime,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send;

    /// Clear the verification seal of the upload identified by `id`.
    ///
    /// Unsealing an upload that is not sealed is a no-op. Returns `Ok(true)` when
    /// a sealed upload was unsealed and `Ok(false)` when no upload matched.
    fn unseal(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;
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

    fn delete_chunks(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete_chunks(id)
    }

    fn expired(
        &mut self,
        instant: NaiveDateTime,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send {
        (**self).expired(instant, limit)
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

    fn seal(
        &mut self,
        id: NumericID,
        sealed_at: NaiveDateTime,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).seal(id, sealed_at)
    }

    fn search(
        &mut self,
        filter: &UploadFilter,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send {
        (**self).search(filter)
    }

    fn stale_seals(
        &mut self,
        instant: NaiveDateTime,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Upload>, RepositoryError>> + Send {
        (**self).stale_seals(instant, limit)
    }

    fn unseal(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).unseal(id)
    }
}
