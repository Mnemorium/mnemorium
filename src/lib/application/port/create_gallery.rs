use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;

use crate::domain::NumericID;
use crate::domain::service::MAX_NAME_LENGTH;

/// Command to create a gallery owned by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CreateGalleryCommand {
    /// Identifier of the authenticated caller that will own the gallery.
    caller_id: NumericID,
    /// Whether any authenticated user may read and moderate the gallery.
    is_public: bool,
    /// Name of the gallery.
    name: String,
}

impl CreateGalleryCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return whether the gallery is public.
    #[must_use]
    pub(crate) fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the name of the gallery.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new create-gallery command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, name: String, is_public: bool) -> Self {
        Self {
            caller_id,
            is_public,
            name,
        }
    }
}

/// Response of a successful gallery creation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct CreateGalleryResponse {
    /// Date and time at which the gallery was created.
    created_at: NaiveDateTime,
    /// Unique identifier of the created gallery.
    gallery_id: NumericID,
    /// Whether any authenticated user may read and moderate the gallery.
    is_public: bool,
    /// Date and time of the last modification of the gallery.
    last_modified_at: NaiveDateTime,
    /// Name of the created gallery.
    name: String,
    /// Identifier of the owning user.
    user_id: NumericID,
}

impl CreateGalleryResponse {
    /// Return the date and time at which the gallery was created.
    #[must_use]
    pub(crate) fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Return the unique identifier of the created gallery.
    #[must_use]
    pub(crate) fn gallery_id(&self) -> NumericID {
        self.gallery_id
    }

    /// Return whether the gallery is public.
    #[must_use]
    pub(crate) fn is_public(&self) -> bool {
        self.is_public
    }

    /// Return the date and time of the last modification of the gallery.
    #[must_use]
    pub(crate) fn last_modified_at(&self) -> NaiveDateTime {
        self.last_modified_at
    }

    /// Return the name of the created gallery.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new create-gallery response.
    #[must_use]
    pub(crate) fn new(
        gallery_id: NumericID,
        user_id: NumericID,
        name: String,
        is_public: bool,
        created_at: NaiveDateTime,
        last_modified_at: NaiveDateTime,
    ) -> Self {
        Self {
            created_at,
            gallery_id,
            is_public,
            last_modified_at,
            name,
            user_id,
        }
    }

    /// Return the identifier of the owning user.
    #[must_use]
    pub(crate) fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Error returned when creating a gallery.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum CreateGalleryError {
    /// The name is empty or made only of whitespace.
    #[error("gallery name must not be empty")]
    InvalidName,
    /// The caller already owns a gallery with the same name.
    #[error("the caller already owns a gallery with this name")]
    NameAlreadyExists,
    /// The name is longer than [`MAX_NAME_LENGTH`] characters.
    #[error("gallery name must be at most {MAX_NAME_LENGTH} characters long")]
    NameTooLong,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for creating a gallery.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait CreateGalleryUseCase: Send + Sync {
    /// Create a gallery owned by the authenticated caller.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn CreateGalleryUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: CreateGalleryCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<CreateGalleryResponse, CreateGalleryError>> + Send + 'future,
        >,
    >;
}
