use std::future::Future;

use crate::domain::alias::NumericID;
use crate::domain::model::file::File;
use crate::domain::port::error::RepositoryError;

/// Search filters for [`FileRepository::search`]. Every field is optional; an
/// all-`None` filter returns every file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileFilter {
    /// Filter on the file identifier.
    pub id: Option<NumericID>,
    /// Filter on the MD5 integrity digest.
    pub md5_integrity: Option<String>,
    /// Filter on the owning user identifier.
    pub user_id: Option<NumericID>,
}

/// Port for persisting and querying `File`.
#[cfg_attr(test, mockall::automock)]
pub trait FileRepository: Send + Sync {
    /// Insert a new `file`, returning the persisted file with its final
    /// identifier.
    ///
    /// The identifier of `file` is ignored: the repository assigns a fresh
    /// identity.
    fn create(&mut self, file: File) -> impl Future<Output = Result<File, RepositoryError>> + Send;

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

    fn search(
        &mut self,
        filter: &FileFilter,
    ) -> impl Future<Output = Result<Vec<File>, RepositoryError>> + Send {
        (**self).search(filter)
    }
}
