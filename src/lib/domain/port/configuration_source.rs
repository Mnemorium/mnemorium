use std::future::Future;

use crate::domain::model::configuration::Configuration;
use crate::domain::port::error::ConfigurationSourceError;

/// Response of a successful configuration load.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoadedConfiguration {
    /// The resolved configuration.
    configuration: Configuration,
    /// Whether this load generated the default configuration file.
    file_created: bool,
}

impl LoadedConfiguration {
    /// Return the resolved configuration.
    #[must_use]
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    /// Return whether this load generated the default configuration file.
    #[must_use]
    pub fn file_created(&self) -> bool {
        self.file_created
    }

    /// Split the result into its configuration and its file-created flag.
    #[must_use]
    pub fn into_parts(self) -> (Configuration, bool) {
        (self.configuration, self.file_created)
    }

    /// Create a new load result.
    #[must_use]
    pub fn new(configuration: Configuration, file_created: bool) -> Self {
        Self {
            configuration,
            file_created,
        }
    }
}

/// Port for loading the application configuration from external sources.
///
/// The resolution is total: built-in defaults are the lowest layer, the
/// configuration file overrides them, and the environment overrides both. The
/// source never touches the datastore.
#[cfg_attr(test, mockall::automock)]
pub trait ConfigurationSource: Send + Sync {
    /// Load the application configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when a source cannot be read or parsed, when an
    /// environment override is unknown or malformed, when the default file
    /// cannot be written, or when the layered settings do not form a valid
    /// configuration.
    fn load(
        &self,
    ) -> impl Future<Output = Result<LoadedConfiguration, ConfigurationSourceError>> + Send;
}
