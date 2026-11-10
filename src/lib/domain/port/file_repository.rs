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
    fn create(&mut self, file: File) -> impl Future<Output = Result<File, RepositoryError>> + Send {
        (**self).create(file)
    }

    fn delete(
        &mut self,
        filter: &FileFilter,
    ) -> impl Future<Output = Result<bool, RepositoryError>> + Send {
        (**self).delete(filter)
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
