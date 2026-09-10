use std::future::Future;
use std::pin::Pin;

use crate::domain::alias::NumericID;

/// Command to change the password behind a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PatchCredentialCommand {
    /// Identifier of the authenticated caller requesting the change.
    caller_id: NumericID,
    /// Identifier of the credential whose password is changed.
    credential_id: NumericID,
    /// New password to store behind the credential.
    password: String,
}

impl PatchCredentialCommand {
    /// Return the identifier of the authenticated caller.
    #[must_use]
    pub fn caller_id(&self) -> NumericID {
        self.caller_id
    }

    /// Return the identifier of the credential whose password is changed.
    #[must_use]
    pub fn credential_id(&self) -> NumericID {
        self.credential_id
    }

    /// Create a new change-password command.
    #[must_use]
    pub fn new(caller_id: NumericID, credential_id: NumericID, password: String) -> Self {
        Self {
            caller_id,
            credential_id,
            password,
        }
    }

    /// Return the new password.
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }
}

/// Error returned when changing the password behind a credential.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PatchCredentialError {
    /// The caller is not the Root Admin.
    #[error("only the Root Admin may change a credential password")]
    Forbidden,
    /// The password does not satisfy the password policy.
    #[error("password does not satisfy the password policy")]
    InvalidPassword,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
    /// No credential exists with the provided identifier.
    #[error("no credential matches the provided identifier")]
    UnknownCredential,
}

/// Use case for changing the password behind a credential.
#[cfg_attr(test, mockall::automock)]
pub trait PatchCredentialUseCase: Send + Sync {
    /// Change the password behind a credential.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn PatchCredentialUseCase>`. `Box` keeps the future on the heap at
    /// a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
        command: PatchCredentialCommand,
    ) -> Pin<Box<dyn Future<Output = Result<(), PatchCredentialError>> + Send + 'future>>;
}
