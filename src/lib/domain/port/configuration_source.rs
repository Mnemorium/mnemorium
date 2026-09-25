use std::future::Future;

use crate::domain::model::configuration::Configuration;
use crate::domain::port::error::ConfigurationSourceError;

/// Port for loading the application configuration from external sources.
///
/// The base layer is read from the datastore by the application service and
/// passed in; the source layers the configuration file and the environment on
/// top of it and returns the complete domain object.
#[cfg_attr(test, mockall::automock)]
pub trait ConfigurationSource: Send + Sync {
    /// Load the application configuration, layering the external sources on top
    /// of `base`.
    ///
    /// # Errors
    ///
    /// Returns an error when a source cannot be read or parsed, or when the
    /// layered settings do not form a valid configuration.
    fn load(
        &self,
        base: Configuration,
    ) -> impl Future<Output = Result<Configuration, ConfigurationSourceError>> + Send;
}
