use std::future::Future;

use crate::domain::model::configuration::Configuration;
use crate::domain::port::error::RepositoryError;

/// Port for loading the application configuration from external sources.
///
/// The loaded configuration is a complete domain object: the source is
/// responsible for layering (persistence row, configuration file,
/// environment) before returning it.
#[cfg_attr(test, mockall::automock)]
pub trait ConfigurationSource: Send + Sync {
    /// Load the application configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when a source cannot be read or parsed, or when the
    /// persistence row does not exist yet.
    fn load(&self) -> impl Future<Output = Result<Configuration, RepositoryError>> + Send;
}
