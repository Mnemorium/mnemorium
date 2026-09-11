use std::future::Future;
use std::pin::Pin;

use crate::domain::model::configuration::Configuration;

/// Response of a successful configuration load.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoadConfigurationResponse {
    /// The loaded configuration.
    configuration: Configuration,
}

impl LoadConfigurationResponse {
    /// Return the loaded configuration.
    #[must_use]
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    /// Create a new load response.
    #[must_use]
    pub fn new(configuration: Configuration) -> Self {
        Self { configuration }
    }
}

/// Error returned when loading the configuration.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadConfigurationError {
    /// The merged configuration failed validation.
    #[error("the merged configuration is invalid: {0}")]
    InvalidConfiguration(#[source] anyhow::Error),
    /// An unexpected or unmapped error occurred.
    #[error("an unknown error occurred: {0}")]
    Unknown(#[source] anyhow::Error),
}

/// Use case for loading the application configuration.
#[cfg_attr(test, mockall::automock)]
pub trait LoadConfigurationUseCase: Send + Sync {
    /// Load the configuration.
    ///
    /// The configuration is merged following the persistence → file →
    /// environment precedence, and missing secrets are generated.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn LoadConfigurationUseCase>`. `Box` keeps the future on the heap
    /// at a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing: `async` state machines may hold
    /// self-referential references across `await` points, and `Future::poll`
    /// takes `Pin<&mut Self>` precisely because moving a polled future would
    /// invalidate those references.
    fn execute<'future>(
        &'future self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<LoadConfigurationResponse, LoadConfigurationError>>
                + Send
                + 'future,
        >,
    >;
}
