use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::alias::NumericID;
use crate::domain::model::gallery_item::GalleryItemMedia;

/// Command to add one of the caller's media to a gallery.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AddGalleryItemCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the gallery the media is added to.
    gallery_id: NumericID,
    /// Medium to add.
    media: GalleryItemMedia,
}

impl AddGalleryItemCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the gallery the media is added to.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return the medium to add.
    #[must_use]
    pub fn media(&self) -> GalleryItemMedia {
        self.media
    }

    /// Create a new add-gallery-item command.
    #[must_use]
    pub fn new(caller_id: NumericID, gallery_id: NumericID, media: GalleryItemMedia) -> Self {
        Self {
            caller_id,
            gallery_id,
            media,
        }
    }
}

/// Response of a successful item addition.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AddGalleryItemResponse {
    /// Date and time at which the item was added.
    added_at: NaiveDateTime,
    /// Identifier of the backing file.
    file_id: NumericID,
    /// Identifier of the gallery the item belongs to.
    gallery_id: NumericID,
    /// Unique identifier of the added item.
    gallery_item_id: NumericID,
    /// Position of the item within its gallery.
    item_index: i64,
    /// Medium the item references.
    media: GalleryItemMedia,
    /// Name of the item: the image name, or the file path for a video.
    name: String,
}

impl AddGalleryItemResponse {
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

    /// Return the unique identifier of the added item.
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

    /// Create a new add-gallery-item response.
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

/// Error returned when adding an item to a gallery.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AddGalleryItemError {
    /// The medium already belongs to a gallery.
    #[error("the medium already belongs to a gallery")]
    AlreadyAssigned,
    /// The caller is not allowed to add an item to the gallery.
    #[error("the caller is not allowed to add an item to this gallery")]
    Forbidden,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No gallery matches the requested identifier.
    #[error("a gallery with this identifier does not exist")]
    NoSuchGallery,
    /// No medium matches the requested identifier.
    #[error("a medium with this identifier does not exist")]
    NoSuchMedia,
    /// The caller does not own the medium and is not the Root Admin.
    #[error("the caller does not own this medium")]
    NotOwnedMedia,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for adding one of the caller's media to a gallery.
#[cfg_attr(test, mockall::automock)]
pub trait AddGalleryItemUseCase: Send + Sync {
    /// Add the requested medium to the gallery.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn AddGalleryItemUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: AddGalleryItemCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<AddGalleryItemResponse, AddGalleryItemError>>
                + Send
                + 'future,
        >,
    >;
}
