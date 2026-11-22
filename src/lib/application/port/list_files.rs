use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;

use super::get_file::GetFileResponse;

/// Command to list the stored files the caller may discover.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ListFilesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Maximum number of files to return, when set.
    limit: Option<usize>,
    /// Number of matching files to skip before returning the page.
    offset: Option<usize>,
    /// Only return the file whose root-relative path matches this value
    /// exactly, when set.
    path: Option<String>,
}

impl ListFilesCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the maximum number of files to return, if any.
    #[must_use]
    pub(crate) fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Create a new list-files command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        path: Option<String>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Self {
        Self {
            caller_id,
            limit,
            offset,
            path,
        }
    }

    /// Return the number of matching files to skip, if any.
    #[must_use]
    pub(crate) fn offset(&self) -> Option<usize> {
        self.offset
    }

    /// Return the exact root-relative path filter, if any.
    #[must_use]
    pub(crate) fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

/// Response of a successful stored-file listing: one page of files.
///
/// Each item is the single-file read model, so a list item and a fetched file
/// carry the same representation and `_links.self` of an item is the route
/// `get_file` serves.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct ListFilesResponse {
    /// Files of the returned page.
    files: Vec<GetFileResponse>,
    /// Total number of files the caller may discover, before pagination.
    total: usize,
}

impl ListFilesResponse {
    /// Return the files of the returned page.
    #[must_use]
    pub(crate) fn files(&self) -> &[GetFileResponse] {
        &self.files
    }

    /// Create a new list-files response.
    #[must_use]
    pub(crate) fn new(files: Vec<GetFileResponse>, total: usize) -> Self {
        Self { files, total }
    }

    /// Return the total number of files, before pagination.
    #[must_use]
    pub(crate) fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing stored files.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum ListFilesError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the stored files the caller may discover.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait ListFilesUseCase: Send + Sync {
    /// List the stored files discoverable by the authenticated caller.
    fn execute<'future>(
        &'future self,
        command: ListFilesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListFilesResponse, ListFilesError>> + Send + 'future>>;
}
