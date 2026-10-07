use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Command to list the galleries the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListGalleriesCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Only return galleries whose public flag matches this value, when set.
    is_public: Option<bool>,
    /// Maximum number of galleries to return, when set.
    limit: Option<usize>,
    /// Only return galleries whose name matches this value, when set.
    name: Option<String>,
    /// Number of matching galleries to skip before returning the page.
    offset: Option<usize>,
    /// Only return galleries owned by this user, when set.
    owner_id: Option<NumericID>,
}

impl ListGalleriesCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the public-flag filter, if any.
    #[must_use]
    pub fn is_public(&self) -> Option<bool> {
        self.is_public
    }

    /// Return the maximum number of galleries to return, if any.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-galleries command.
    #[must_use]
    pub fn new(
        caller_id: NumericID,
        name: Option<String>,
        is_public: Option<bool>,
        owner_id: Option<NumericID>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Self {
        Self {
            caller_id,
            is_public,
            limit,
            name,
            offset,
            owner_id,
        }
    }

    /// Return the number of matching galleries to skip, if any.
    #[must_use]
    pub fn offset(&self) -> Option<usize> {
        self.offset
    }

    /// Return the owner filter, if any.
    #[must_use]
    pub fn owner_id(&self) -> Option<NumericID> {
        self.owner_id
    }
}

/// One gallery of a successful listing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListGalleriesItem {
    /// Date and time at which the gallery was created.
    created_at: NaiveDateTime,
    /// Unique identifier of the gallery.
    gallery_id: NumericID,
    /// Whether any authenticated user may read and moderate the gallery.
    is_public: bool,
    /// Number of items the gallery contains.
    item_count: usize,
    /// Date and time of the last modification of the gallery.
    last_modified_at: NaiveDateTime,
    /// Name of the gallery.
    name: String,
    /// Identifier of the owning user, absent for the system gallery.
    user_id: Option<NumericID>,
}

impl ListGalleriesItem {
    /// Return the date and time at which the gallery was created.
    #[must_use]
    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Return the unique identifier of the gallery.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return whether the gallery is public.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the number of items the gallery contains.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Return the date and time of the last modification of the gallery.
    #[must_use]
    pub fn last_modified_at(&self) -> NaiveDateTime {
        self.last_modified_at
    }

    /// Return the name of the gallery.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new listed gallery.
    #[must_use]
    pub fn new(
        gallery_id: NumericID,
        user_id: Option<NumericID>,
        name: String,
        is_public: bool,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
        item_count: usize,
    ) -> Self {
        Self {
            created_at,
            gallery_id,
            is_public,
            item_count,
            last_modified_at,
            name,
            user_id,
        }
    }

    /// Return the identifier of the owning user, if any.
    #[must_use]
    pub fn user_id(&self) -> Option<NumericID> {
        self.user_id
    }
}

/// Response of a successful gallery listing: one page of galleries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListGalleriesResponse {
    /// Galleries of the returned page.
    galleries: Vec<ListGalleriesItem>,
    /// Total number of galleries the caller may see, before pagination.
    total: usize,
}

impl ListGalleriesResponse {
    /// Return the galleries of the returned page.
    #[must_use]
    pub fn galleries(&self) -> &[ListGalleriesItem] {
        &self.galleries
    }

    /// Create a new list-galleries response.
    #[must_use]
    pub fn new(galleries: Vec<ListGalleriesItem>, total: usize) -> Self {
        Self { galleries, total }
    }

    /// Return the total number of galleries, before pagination.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing galleries.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListGalleriesError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the galleries the caller may see.
#[cfg_attr(test, mockall::automock)]
pub trait ListGalleriesUseCase: Send + Sync {
    /// List the galleries visible to the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn ListGalleriesUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: ListGalleriesCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListGalleriesResponse, ListGalleriesError>> + Send + 'future,
        >,
    >;
}
