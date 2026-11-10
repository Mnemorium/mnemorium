use std::future::Future;

use crate::domain::NumericID;
use crate::domain::model::File;
use crate::domain::port::RepositoryError;

/// Search filters for [`FileRepository::search`]. Every field is optional; an
/// all-`None` filter returns every file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct FileFilter {
    /// Filter on the file identifier.
    pub id: Option<NumericID>,
    /// Filter on the integrity hash.
    pub integrity_hash: Option<String>,
    /// Filter on the path relative to the library root.
    pub path: Option<String>,
    /// Filter on the owning user identifier.
    pub user_id: Option<NumericID>,
}

/// Port for persisting and querying `File`.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait FileRepository: Send + Sync {
    /// Clear the integrity hash of the file identified by `id`.
    ///
    /// Returns `Ok(true)` when a row carried the identifier and was updated,
    /// and `Ok(false)` when none matched. The per-owner unique key ignores a
    /// null digest, so a transferred file whose content the new owner already
    /// stores can be moved without clearing the old owner's copy.
    fn clear_integrity_hash(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Insert a new `file`, returning the persisted file with its final
    /// identifier.
    ///
    /// The identifier of `file` is ignored: the repository assigns a fresh
    /// identity.
    fn create(&mut self, file: File) -> impl Future<Output = Result<File, RepositoryError>> + Send;

    /// Delete every file matching `filter`.
    ///
    /// Returns `Ok(true)` when at least one file was deleted, and `Ok(false)`
    /// when none matched. An all-`None` filter deletes every file.
    ///
    /// The `file` foreign keys of the Library media (`image.file_id`,
    /// `video.file_id`) do not cascade (`ARCH-004`, `PERS-020`), so the delete
    /// is rejected while a media row still references the file.
    fn delete(
        &mut self,
        filter: &FileFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Reassign the file identified by `id` to `new_owner`.
    ///
    /// Returns `Ok(true)` when a row carried the identifier and was updated,
    /// and `Ok(false)` when none matched (`STY-RUST-046`). The owner is a
    /// narrow, explicit mutation rather than part of [`Self::save`], so the
    /// ingest path cannot rewrite it by accident.
    fn reassign_owner(
        &mut self,
        id: NumericID,
        new_owner: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send;

    /// Update an existing `file` targeted by its identifier, returning the
    /// persisted file.
    ///
    /// A missing target is reported as [`RepositoryError::OperationFailed`]: the
    /// ingest only updates a row it read in the same operation.
    fn save(&mut self, file: File) -> impl Future<Output = Result<File, RepositoryError>> + Send;

    /// Search files matching `filter`, returned as `Vec<File>`.
    ///
    /// Returns an empty list when no file matches; a missing match is a valid
    /// outcome, not an error.
    fn search(
        &mut self,
        filter: &FileFilter,
    ) -> impl Future<Output = Result<Vec<File>, RepositoryError>> + Send;
}

#[cfg(test)]
impl<T: FileRepository + ?Sized> FileRepository for &mut T {
    fn clear_integrity_hash(
        &mut self,
        id: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).clear_integrity_hash(id)
    }

    fn create(&mut self, file: File) -> impl Future<Output = Result<File, RepositoryError>> + Send {
        (**self).create(file)
    }

    fn delete(
        &mut self,
        filter: &FileFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
    }

    fn reassign_owner(
        &mut self,
        id: NumericID,
        new_owner: NumericID,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).reassign_owner(id, new_owner)
    }

    fn save(&mut self, file: File) -> impl Future<Output = Result<File, RepositoryError>> + Send {
        (**self).save(file)
    }

    fn search(
        &mut self,
        filter: &FileFilter,
    ) -> impl Future<Output = Result<Vec<File>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
