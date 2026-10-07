use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to delete a gallery and its items.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DeleteGalleryCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the gallery to delete.
    gallery_id: NumericID,
}

impl DeleteGalleryCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the gallery to delete.
    #[must_use]
    pub fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Create a new delete-gallery command.
    #[must_use]
    pub fn new(caller_id: NumericID, gallery_id: NumericID) -> Self {
        Self {
            caller_id,
            gallery_id,
        }
    }
}

/// Error returned when deleting a gallery.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DeleteGalleryError {
    /// The system gallery can never be deleted.
    #[error("the default gallery cannot be deleted")]
    DefaultGallery,
    /// The caller is not the owner of the gallery.
    #[error("only the owner may delete this gallery")]
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

/// Use case for deleting a gallery and its items.
#[cfg_attr(test, mockall::automock)]
pub trait DeleteGalleryUseCase: Send + Sync {
    /// Delete the requested gallery and its items.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn DeleteGalleryUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: DeleteGalleryCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteGalleryError>> + Send + 'future>>;
}
