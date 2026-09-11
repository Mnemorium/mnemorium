use std::future::Future;

use crate::domain::model::configuration::Configuration;
use crate::domain::port::error::RepositoryError;

/// Port for persisting and querying the `Configuration` singleton.
#[cfg_attr(test, mockall::automock)]
pub trait ConfigurationRepository: Send + Sync {
    /// Insert the configuration, returning the persisted configuration.
    ///
    /// Fails with [`RepositoryError::AlreadyExist`] when the configuration
    /// already exists: the singleton row cannot be created twice.
    fn create(
        &self,
        configuration: Configuration,
    ) -> impl Future<Output = Result<Configuration, RepositoryError>> + Send;

    /// Insert or update the configuration, returning the persisted
    /// configuration.
    fn save(
        &self,
        configuration: Configuration,
    ) -> impl Future<Output = Result<Configuration, RepositoryError>> + Send;

    /// Search the configuration.
    ///
    /// Returns `Ok(None)` when the configuration does not exist yet; a missing
    /// configuration is a valid outcome, not an error.
    fn search(&self)
    -> impl Future<Output = Result<Option<Configuration>, RepositoryError>> + Send;
}
