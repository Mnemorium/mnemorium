use std::future::Future;
use std::pin::Pin;

use crate::domain::model::configuration::Configuration;

/// Response of a successful configuration load.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoadConfigurationResponse {
    /// The setting paths the environment layer applied, sorted.
    ///
    /// The composition root logs them once, after the subscriber is installed,
    /// so an operator can see which settings the environment took over. The
    /// list carries names only, never a value (`OBS-004`).
    applied: Vec<&'static str>,
    /// The loaded configuration.
    configuration: Configuration,
    /// Whether this call generated the default configuration file.
    ///
    /// The composition root warns naming the resolved storage root when the
    /// file was generated, because losing the file silently reverts settings.
    file_created: bool,
}

impl LoadConfigurationResponse {
    /// Return the setting paths the environment layer applied.
    #[must_use]
    pub fn applied(&self) -> &[&'static str] {
        &self.applied
    }

    /// Return the loaded configuration.
    #[must_use]
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    /// Return whether this call generated the default configuration file.
    #[must_use]
    pub fn file_created(&self) -> bool {
        self.file_created
    }

    /// Create a new load response.
    #[must_use]
    pub fn new(
        configuration: Configuration,
        file_created: bool,
        applied: Vec<&'static str>,
    ) -> Self {
        Self {
            applied,
            configuration,
            file_created,
        }
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
    /// The configuration is resolved as defaults → configuration file →
    /// environment, without touching the datastore.
    ///
    /// The future is returned erased (`dyn`, not `impl Future`), boxed and
    /// pinned. `dyn` erases the concrete future type, which is what makes this
    /// method object-safe so the use case can be stored as
    /// `Arc<dyn LoadConfigurationUseCase>`. `Box` keeps the future on the heap
    /// at a stable address. `Pin` encodes the guarantee that the future is not
    /// moved once it has started executing.
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
