use std::future::Future;
use std::pin::Pin;

/// Command for initializing the Root Admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct InitializeRootAdminCommand {
    /// Whether the generated password may be revealed to standard output.
    reveal_password: bool,
}

impl InitializeRootAdminCommand {
    /// Create a new command deciding whether the generated password may be revealed.
    #[must_use]
    pub(crate) fn new(reveal_password: bool) -> Self {
        Self { reveal_password }
    }

    /// Return whether the generated password may be revealed.
    #[must_use]
    pub(crate) fn reveal_password(self) -> bool {
        self.reveal_password
    }
}

/// Response of a successful root admin initialization.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) struct InitializeRootAdminResponse {
    /// Default password generated for the root admin.
    default_password: String,
}

impl InitializeRootAdminResponse {
    /// Return the default password generated for the root admin.
    #[must_use]
    pub(crate) fn default_password(&self) -> &str {
        &self.default_password
    }

    /// Create a new initialization response.
    #[must_use]
    pub(crate) fn new(default_password: String) -> Self {
        Self { default_password }
    }
}

/// Error returned when initializing the root admin.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum InitializeRootAdminError {
    /// The configuration would create the Root Admin while its only password may not be revealed.
    #[error(
        "cannot create the Root Admin because security.log_root_admin_password is false; enable it for the first start"
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
