use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;

/// Command to list the playlists the caller may see.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListPlaylistsCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Only return playlists whose public flag matches this value, when set.
    is_public: Option<bool>,
    /// Maximum number of playlists to return, when set.
    limit: Option<usize>,
    /// Only return playlists whose name matches this value, when set.
    name: Option<String>,
    /// Number of matching playlists to skip before returning the page.
    offset: Option<usize>,
    /// Only return playlists owned by this user, when set.
    owner_id: Option<NumericID>,
}

impl ListPlaylistsCommand {
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

    /// Return the maximum number of playlists to return, if any.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the name filter, if any.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new list-playlists command.
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

    /// Return the number of matching playlists to skip, if any.
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

/// One playlist of a successful listing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListPlaylistsItem {
    /// Identifier of the file holding the playlist's artwork, when set.
    cover_file_id: Option<NumericID>,
    /// Date and time of the creation of the playlist.
    created_at: NaiveDateTime,
    /// Unique identifier of the playlist.
    id: NumericID,
    /// Whether the playlist is visible to every user.
    is_public: bool,
    /// Date and time of the last modification of the playlist.
    last_modified_at: NaiveDateTime,
    /// Name of the playlist.
    name: String,
    /// Number of tracks the playlist holds.
    track_count: usize,
    /// Identifier of the owning user.
    user_id: NumericID,
}

impl ListPlaylistsItem {
    /// Return the identifier of the file holding the playlist's artwork, if any.
    #[must_use]
    pub fn cover_file_id(&self) -> Option<NumericID> {
        self.cover_file_id
    }

    /// Return the date and time of the creation of the playlist.
    #[must_use]
    pub fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Return the unique identifier of the playlist.
    #[must_use]
    pub fn id(&self) -> NumericID {
        self.id
    }

    /// Return whether the playlist is public.
    #[must_use]
    pub fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the date and time of the last modification of the playlist.
    #[must_use]
    pub fn last_modified_at(&self) -> NaiveDateTime {
        self.last_modified_at
    }

    /// Return the name of the playlist.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new listed playlist.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the item's fields one-to-one"
    )]
    pub fn new(
        id: NumericID,
        user_id: NumericID,
        name: String,
        is_public: bool,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
        track_count: usize,
        cover_file_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover_file_id,
            created_at,
            id,
            is_public,
            last_modified_at,
            name,
            track_count,
            user_id,
        }
    }
    /// Return the number of tracks the playlist holds.
    #[must_use]
    pub fn track_count(&self) -> usize {
        self.track_count
    }

    /// Return the identifier of the owning user.
    #[must_use]
    pub fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Response of a successful playlist listing: one page of playlists.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ListPlaylistsResponse {
    /// Playlists of the returned page.
    playlists: Vec<ListPlaylistsItem>,
    /// Total number of playlists the caller may see, before pagination.
    total: usize,
}

impl ListPlaylistsResponse {
    /// Create a new list-playlists response.
    #[must_use]
    pub fn new(playlists: Vec<ListPlaylistsItem>, total: usize) -> Self {
        Self { playlists, total }
    }

    /// Return the playlists of the returned page.
    #[must_use]
    pub fn playlists(&self) -> &[ListPlaylistsItem] {
        &self.playlists
    }

    /// Return the total number of playlists, before pagination.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Error returned when listing playlists.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ListPlaylistsError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for listing the playlists the caller may see.
#[cfg_attr(test, mockall::automock)]
pub trait ListPlaylistsUseCase: Send + Sync {
    /// List the playlists visible to the authenticated caller.
    fn execute<'future>(
        &'future self,
        command: ListPlaylistsCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ListPlaylistsResponse, ListPlaylistsError>> + Send + 'future,
        >,
    >;
}
