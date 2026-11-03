use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to list the music groups the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListMusicGroupsCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Maximum number of groups to return, when set.
    limit: Option<usize>,
    /// Only return groups whose name matches this value, when set.
    name: Option<String>,
    /// Number of matching groups to skip before returning the page.
    offset: Option<usize>,
}

impl ListMusicGroupsCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the maximum number of groups to return, if any.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-groups command.
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

    /// Return the number of matching groups to skip, if any.
    #[must_use]
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }
}

/// One group of a successful listing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListMusicGroupsItem {
    /// Identifier of the image holding the group's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Unique identifier of the group.
    group_id: NumericID,
    /// Stage name of the group.
    name: String,
}

impl ListMusicGroupsItem {
    /// Return the identifier of the image holding the group's artwork, if any.
    #[must_use]
    pub fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the unique identifier of the group.
    #[must_use]
    pub fn group_id(&self) -> NumericID {
        self.group_id
    }

    /// Return the stage name of the group.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new listed group.
    #[must_use]
    pub fn new(group_id: NumericID, name: String, cover_image_id: Option<NumericID>) -> Self {
        Self {
            cover_image_id,
            group_id,
            name,
        }
    }
}

/// Response of a successful group listing: one page of groups.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListMusicGroupsResponse {
    /// The groups of the returned page.
    groups: Vec<ListMusicGroupsItem>,
    /// Total number of groups the caller may see, before pagination.
    total: usize,
}

impl ListMusicGroupsResponse {
    /// Return the groups of the returned page.
    #[must_use]
    pub fn groups(&self) -> &[ListMusicGroupsItem] {
        &self.groups
    }

    /// Create a new list-groups response.
    #[must_use]
    pub fn new(groups: Vec<ListMusicGroupsItem>, total: usize) -> Self {
        Self { groups, total }
    }

    /// Return the total number of groups, before pagination.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing music groups.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListMusicGroupsError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the music groups the caller may see.
#[cfg_attr(test, mockall::automock)]
pub trait ListMusicGroupsUseCase: Send + Sync {
    /// List the music groups visible to the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn ListMusicGroupsUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: ListMusicGroupsCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListMusicGroupsResponse, ListMusicGroupsError>>
                + Send
                + 'future,
        >,
    >;
}
