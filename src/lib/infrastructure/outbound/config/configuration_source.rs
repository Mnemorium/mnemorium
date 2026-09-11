use std::sync::Arc;

use config::Config;
use config::Environment;
use config::File;

use crate::domain::model::configuration::Configuration;
use crate::domain::port::configuration_repository::ConfigurationRepository;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::error::RepositoryError;
use crate::infrastructure::outbound::config::dbsqlite3::DbSqlite3Source;

/// Path to the user configuration file.
const USER_CONFIG_PATH: &str = "config.yaml";

/// Source loading the configuration from the database row, the configuration
/// file, and the environment.
///
/// The database row is the base layer; the configuration file overrides it;
/// the environment overrides both. The layering is delegated to the `config`
/// crate, which merges its sources in the order they are added.
pub struct ConfigConfigurationSource<R: ConfigurationRepository> {
    /// Repository persisting the configuration singleton.
    configuration_repository: Arc<R>,
}

impl<R: ConfigurationRepository> ConfigConfigurationSource<R> {
    /// Create a new source bound to `configuration_repository`.
    #[must_use]
    pub fn new(configuration_repository: Arc<R>) -> Self {
        Self {
            configuration_repository,
        }
    }
}

impl<R: ConfigurationRepository> ConfigurationSource for ConfigConfigurationSource<R> {
    async fn load(&self) -> Result<Configuration, RepositoryError> {
        let row = self
            .configuration_repository
            .search()
            .await?
            .ok_or(RepositoryError::OperationFailed)?;

        let settings = Config::builder()
            .add_source(DbSqlite3Source::new(row))
            .add_source(File::with_name(USER_CONFIG_PATH).required(false))
            .add_source(
                Environment::with_prefix("mnemorium")
                    .separator("__")
                    .try_parsing(true)
                    .ignore_empty(true),
            )
            .build()
            .map_err(|error| RepositoryError::Unknown(anyhow::anyhow!(error)))?;

        settings
            .try_deserialize::<Configuration>()
            .map_err(|error| RepositoryError::Unknown(anyhow::anyhow!(error)))
    }
}
