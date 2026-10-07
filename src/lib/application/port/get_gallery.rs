use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::gallery_item::GalleryItemMedia;

/// Command to fetch one gallery and its items.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetGalleryCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the gallery to fetch.
    gallery_id: NumericID,
}

impl GetGalleryCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the gallery to fetch.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Create a new fetch-gallery command.
    #[must_use]
    pub fn new(caller_id: NumericID, gallery_id: NumericID) -> Self {
        Self {
            caller_id,
            gallery_id,
        }
    }
}

/// One item of a gallery, without the metadata of its medium.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GalleryItemSummary {
    /// Date and time at which the item was added.
    added_at: NaiveDateTime,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Identifier of the gallery the item belongs to.
    gallery_id: NumericID,
    /// Unique identifier of the item.
    gallery_item_id: NumericID,
    /// Position of the item within its gallery.
    item_index: i64,
    /// Medium the item references.
    media: GalleryItemMedia,
    /// Name of the item: the image name, or the file path for a video.
    name: String,
}

impl GalleryItemSummary {
    /// Return the date and time at which the item was added.
    #[must_use]
    pub fn added_at(&self) -> NaiveDateTime {
        self.added_at
    }

    /// Return the identifier of the backing file.
    #[must_use]
    pub fn file_id(&self) -> NumericID {
        self.file_id
    }

    /// Return the identifier of the gallery the item belongs to.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return the unique identifier of the item.
    #[must_use]
    pub fn gallery_item_id(&self) -> NumericID {
        self.gallery_item_id
    }

    /// Return the position of the item within its gallery.
    #[must_use]
    pub fn item_index(&self) -> i64 {
        self.item_index
    }

    /// Return the medium the item references.
    #[must_use]
    pub fn media(&self) -> GalleryItemMedia {
        self.media
    }

    /// Return the name of the item.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Create a new gallery-item summary.
    #[must_use]
    pub fn new(
        gallery_item_id: NumericID,
        gallery_id: NumericID,
        media: GalleryItemMedia,
        file_id: NumericID,
        name: String,
        item_index: i64,
        added_at: NaiveDateTime,
    ) -> Self {
        Self {
            added_at,
            file_id,
            gallery_id,
            gallery_item_id,
            item_index,
            media,
            name,
        }
    }
}

/// Response of a successful gallery lookup: the gallery and its items.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GetGalleryResponse {
    /// Date and time at which the gallery was created.
    created_at: NaiveDateTime,
    /// Unique identifier of the gallery.
    gallery_id: NumericID,
    /// Whether any authenticated user may read and moderate the gallery.
    is_public: bool,
    /// Number of items the gallery contains.
    item_count: usize,
    /// Items of the gallery, ordered by position.
    items: Vec<GalleryItemSummary>,
    /// Date and time of the last modification of the gallery.
    last_modified_at: NaiveDateTime,
    /// Name of the gallery.
    name: String,
    /// Identifier of the owning user, absent for the system gallery.
    user_id: Option<NumericID>,
}

impl GetGalleryResponse {
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

    /// Return the items of the gallery.
    #[must_use]
    pub fn items(&self) -> &[GalleryItemSummary] {
        &self.items
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

    /// Create a new fetch-gallery response.
    #[must_use]
    pub fn new(
        gallery_id: NumericID,
        user_id: Option<NumericID>,
        name: String,
        is_public: bool,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
        items: Vec<GalleryItemSummary>,
    ) -> Self {
        Self {
            created_at,
            gallery_id,
            is_public,
            item_count: items.len(),
            items,
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

/// Error returned when fetching a gallery.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetGalleryError {
    /// The caller is not allowed to read the gallery.
    #[error("the caller is not allowed to read this gallery")]
    Forbidden,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No gallery matches the requested identifier.
    #[error("a gallery with this identifier does not exist")]
    NoSuchGallery,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for fetching one gallery and its items.
#[cfg_attr(test, mockall::automock)]
pub trait GetGalleryUseCase: Send + Sync {
    /// Fetch the requested gallery and its items.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetGalleryUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetGalleryCommand,
    ) -> Pin<Box<dyn Future<Output = Result<GetGalleryResponse, GetGalleryError>> + Send + 'future>>;
}
