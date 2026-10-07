use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to delete one item of a gallery.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[expect(
    clippy::struct_field_names,
    reason = "`gallery_item_id` is the primary key name mandated by the SQL section and the domain field naming"
)]
pub struct DeleteGalleryItemCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the gallery the item belongs to.
    gallery_id: NumericID,
    /// Identifier of the item to delete.
    gallery_item_id: NumericID,
}

impl DeleteGalleryItemCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the gallery the item belongs to.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return the identifier of the item to delete.
    #[must_use]
    pub fn gallery_item_id(&self) -> NumericID {
        self.gallery_item_id
    }

    /// Create a new delete-gallery-item command.
    #[must_use]
    pub fn new(caller_id: NumericID, gallery_id: NumericID, gallery_item_id: NumericID) -> Self {
        Self {
            caller_id,
            gallery_id,
            gallery_item_id,
        }
    }
}

/// Error returned when deleting one item of a gallery.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DeleteGalleryItemError {
    /// The caller is not allowed to delete items of the gallery.
    #[error("the caller is not allowed to delete items of this gallery")]
    Forbidden,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No gallery matches the requested identifier.
    #[error("a gallery with this identifier does not exist")]
    NoSuchGallery,
    /// No item matches the requested identifier in the requested gallery.
    #[error("an item with this identifier does not exist in this gallery")]
    NoSuchItem,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for deleting one item of a gallery.
#[cfg_attr(test, mockall::automock)]
pub trait DeleteGalleryItemUseCase: Send + Sync {
    /// Delete the requested item of the gallery.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn DeleteGalleryItemUseCase>`. `Box` keeps the future on the heap
    /// at a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: DeleteGalleryItemCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteGalleryItemError>> + Send + 'future>>;
}
