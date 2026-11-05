use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;
use crate::domain::model::music_group::MusicGroup;

/// Command to update a music group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UpdateMusicGroupCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the image holding the group's artwork, when the request
    /// replaces it.
    cover_image_id: Option<NumericID>,
    /// Identifier of the group to update.
    group_id: NumericID,
    /// New stage name, when the request renames the group.
    name: Option<String>,
}

impl UpdateMusicGroupCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the replacement cover image, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the identifier of the group to update.
    #[must_use]
    pub(crate) fn group_id(&self) -> NumericID {
        self.group_id
    }

    /// Return the new stage name, if any.
    #[must_use]
    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Create a new update-group command.
    #[must_use]
    pub(crate) fn new(
        caller_id: NumericID,
        group_id: NumericID,
        name: Option<String>,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            caller_id,
            cover_image_id,
            group_id,
            name,
        }
    }
}

/// Response of a successful group update: the stored group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct UpdateMusicGroupResponse {
    /// The group that was stored.
    group: MusicGroup,
}

impl UpdateMusicGroupResponse {
    /// Return the group that was stored.
    #[must_use]
    pub(crate) fn group(&self) -> &MusicGroup {
        &self.group
    }

    /// Create a new updated-group response.
    #[must_use]
    pub(crate) fn new(group: MusicGroup) -> Self {
        Self { group }
    }
}

/// Error returned when updating a music group.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum UpdateMusicGroupError {
    /// The stored group refuses the update: the default group is protected, or
    /// the cover image belongs to a user.
    #[error("the music group cannot be updated")]
    Conflict,
    /// The caller may not manage the library catalogue.
    #[error("the caller may not manage the library")]
    Forbidden,
    /// The cover image belongs to a user, not to the library.
    #[error("the cover image is not a library image")]
    #[expect(
        dead_code,
        reason = "the error enum keeps the catch-all variant the rulebook's error shape declares; no caller constructs it yet"
    )]
    InvalidCover,
    /// The name is empty or made only of whitespace.
    #[error("the name must not be empty")]
    InvalidName,
    /// Another group already carries the name.
    #[error("a music group with this name already exists")]
    NameAlreadyExists,
    /// The name is longer than the name policy allows.
    #[error("the name is too long")]
    NameTooLong,
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No group matches the requested identifier.
    #[error("a music group with this identifier does not exist")]
    NoSuchGroup,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for updating a music group.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait UpdateMusicGroupUseCase: Send + Sync {
    /// Update the requested group.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn UpdateMusicGroupUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: UpdateMusicGroupCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<UpdateMusicGroupResponse, UpdateMusicGroupError>>
                + Send
                + 'future,
        >,
    >;
}
