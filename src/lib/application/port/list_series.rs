use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to list the shows the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListSeriesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Maximum number of shows to return, when set.
    limit: Option<usize>,
    /// Only return shows whose name matches this value, when set.
    name: Option<String>,
    /// Number of matching shows to skip before returning the page.
    offset: Option<usize>,
}

impl ListSeriesCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the maximum number of shows to return, if any.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-shows command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        name: Option<String>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Self {
        Self {
            caller_id,
            limit,
            name,
            offset,
        }
    }

    /// Return the number of matching shows to skip, if any.
    #[must_use]
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }
}

/// One show of a successful listing: its identifier and name only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListSeriesItem {
    /// Unique identifier of the show.
    id: NumericID,
    /// Name of the show.
    name: String,
}

impl ListSeriesItem {
    /// Return the unique identifier of the show.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return the name of the show.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new listed show.
    #[must_use]
    pub fn new(id: NumericID, name: String) -> Self {
        Self { id, name }
    }
}

/// Response of a successful show listing: one page of shows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListSeriesResponse {
    /// The shows of the returned page.
    series: Vec<ListSeriesItem>,
    /// Total number of shows, before pagination.
    total: usize,
}

impl ListSeriesResponse {
    /// Create a new list-shows response.
    #[must_use]
    pub fn new(series: Vec<ListSeriesItem>, total: usize) -> Self {
        Self { series, total }
    }

    /// Return the shows of the returned page.
    #[must_use]
    pub fn series(&self) -> &[ListSeriesItem] {
        &self.series
    }

    /// Return the total number of shows, before pagination.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing shows.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListSeriesError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the shows the caller may see.
#[cfg_attr(test, mockall::automock)]
pub trait ListSeriesUseCase: Send + Sync {
    /// List the shows visible to the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned so the use case can be stored as `Arc<dyn ListSeriesUseCase>`.
    fn execute<'future>(
        &'future self,
        command: ListSeriesCommand,
    ) -> Pin<Box<dyn Future<Output = Result<ListSeriesResponse, ListSeriesError>> + Send + 'future>>;
}
