use std::future::Future;
use std::pin::Pin;

/// Response of a successful root admin initialization.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InitializeRootAdminResponse {
    /// Default password generated for the root admin.
    default_password: String,
}

impl InitializeRootAdminResponse {
    /// Return the default password generated for the root admin.
    #[must_use]
    pub fn default_password(&self) -> &str {
        &self.default_password
    }

    /// Create a new initialization response.
    #[must_use]
    pub fn new(default_password: String) -> Self {
        Self { default_password }
    }
}

/// Error returned when initializing the root admin.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InitializeRootAdminError {
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for initializing the root admin.
#[cfg_attr(test, mockall::automock)]
pub trait InitializeRootAdminUseCase: Send + Sync {
    /// Initialize the root admin.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn InitializeRootAdminUseCase>`. `Box` keeps the future on the heap
    /// at a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Option<InitializeRootAdminResponse>, InitializeRootAdminError>,
                > + Send
                + 'future,
        >,
    >;
}
