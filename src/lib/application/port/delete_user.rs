use std::future::Future;
use std::pin::Pin;

use crate::domain::NumericID;

/// Command to delete a user account and the data it owns.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct DeleteUserCommand {
    /// Identifier of the authenticated caller.
    caller_id: NumericID,
    /// Identifier of the user to delete.
    user_id: NumericID,
}

impl DeleteUserCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub(crate) fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Create a new delete-user command.
    #[must_use]
    pub(crate) fn new(caller_id: NumericID, user_id: NumericID) -> Self {
        Self { caller_id, user_id }
    }

    /// Return the identifier of the user to delete.
    #[must_use]
    pub(crate) fn user_id(&self) -> NumericID {
        self.user_id
    }
}

/// Error returned when deleting a user account.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum DeleteUserError {
    /// The authenticated caller does not exist anymore.
    #[error("the authenticated user does not exist")]
    NoSuchCaller,
    /// No user matches the requested identifier.
    #[error("a user with this identifier does not exist")]
    NoSuchUser,
    /// The caller is not allowed to delete the target account.
    #[error("only administrators may delete users")]
    NotAdmin,
    /// The target user cannot be deleted by the caller.
    #[error("the targeted user cannot be deleted by this caller")]
    TargetNotModifiable,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for deleting a user account and the data it owns.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait DeleteUserUseCase: Send + Sync {
    /// Delete the requested account.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn DeleteUserUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: DeleteUserCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeleteUserError>> + Send + 'future>>;
}
