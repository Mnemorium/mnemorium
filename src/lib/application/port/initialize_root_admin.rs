use std::future::Future;
use std::pin::Pin;

/// Command for initializing the Root Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct InitializeRootAdminCommand {
    /// Whether the setup token may be revealed to standard output.
    reveal: bool,
    /// Lifetime of an issued setup token, in seconds.
    ttl_seconds: u64,
}

impl InitializeRootAdminCommand {
    /// Create a new command deciding whether the setup token may be revealed.
    #[must_use]
    pub(crate) fn new(reveal: bool, ttl_seconds: u64) -> Self {
        Self {
            reveal,
            ttl_seconds,
        }
    }

    /// Return whether the setup token may be revealed.
    #[must_use]
    pub(crate) fn reveal(self) -> bool {
        self.reveal
    }

    /// Return the lifetime of an issued setup token, in seconds.
    #[must_use]
    pub(crate) fn ttl_seconds(self) -> u64 {
        self.ttl_seconds
    }
}

/// Response of a successful root admin initialization.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct InitializeRootAdminResponse {
    /// Setup token the operator exchanges for the Root Admin credential.
    setup_token: String,
}

impl InitializeRootAdminResponse {
    /// Create a new initialization response.
    #[must_use]
    pub(crate) fn new(setup_token: String) -> Self {
        Self { setup_token }
    }

    /// Return the setup token the operator exchanges for the Root Admin
    /// credential.
    #[must_use]
    pub(crate) fn setup_token(&self) -> &str {
        &self.setup_token
    }
}

/// Error returned when initializing the root admin.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum InitializeRootAdminError {
    /// The configuration would issue the setup token while its reveal is disabled.
    #[error(
        "cannot create or resume the Root Admin because security.root_admin_setup.reveal is false; enable it to reveal the setup token"
    )]
    RevealDisabled,
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for initializing the root admin.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait InitializeRootAdminUseCase: Send + Sync {
    /// Initialize the root admin.
    fn execute<'future>(
        &'future self,
        command: InitializeRootAdminCommand,
    ) -> Pin<
        Box<
            dyn Future<
                    Output = Result<Option<InitializeRootAdminResponse>, InitializeRootAdminError>,
                > + Send
                + 'future,
        >,
    >;
}
