use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;

/// Command to fetch one music group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetMusicGroupCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the group to fetch.
    group_id: NumericID,
}

impl GetMusicGroupCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the group to fetch.
    #[must_use]
    pub(crate) fn group_id(&self) -> NumericID {
        self.group_id
    }

    /// Create a new fetch-group command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, group_id: NumericID) -> Self {
        Self {
            caller_id,
            group_id,
        }
    }
}

/// Response of a successful group lookup: one group.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct GetMusicGroupResponse {
    /// Identifier of the image holding the group's artwork, when set.
    cover_image_id: Option<NumericID>,
    /// Unique identifier of the group.
    group_id: NumericID,
    /// Stage name of the group.
    name: String,
}

impl GetMusicGroupResponse {
    /// Return the identifier of the image holding the group's artwork, if any.
    #[must_use]
    pub(crate) fn cover_image_id(&self) -> Option<NumericID> {
        self.cover_image_id
    }

    /// Return the unique identifier of the group.
    #[must_use]
    pub(crate) fn group_id(&self) -> NumericID {
        self.group_id
    }

    /// Return the stage name of the group.
    #[must_use]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Create a new fetched-group response.
    #[must_use]
    pub(crate) fn new(
        name: String,
        group_id: NumericID,
        cover_image_id: Option<NumericID>,
    ) -> Self {
        Self {
            cover_image_id,
            group_id,
            name,
        }
    }
}

/// Error returned when fetching one music group.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum GetMusicGroupError {
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

/// Use case for fetching one music group.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait GetMusicGroupUseCase: Send + Sync {
    /// Fetch the requested group.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn GetMusicGroupUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: GetMusicGroupCommand,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<GetMusicGroupResponse, GetMusicGroupError>> + Send + 'future,
        >,
    >;
}
