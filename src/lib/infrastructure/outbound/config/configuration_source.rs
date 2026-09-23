use std::future::Future;
use std::future::ready;

use config::Config;
use config::Environment;
use config::File;

use crate::domain::model::configuration::Configuration;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::error::ConfigurationSourceError;
use crate::infrastructure::outbound::config::dbsqlite3::DbSqlite3Source;

/// Path to the user configuration file.
const USER_CONFIG_PATH: &str = "config.yaml";

/// Source layering the configuration file and the environment over a base
/// configuration snapshot read from the datastore by the caller.
///
/// The snapshot is the base layer; the configuration file overrides it; the
/// environment overrides both. The layering is delegated to the `config` crate,
/// which merges its sources in the order they are added.
pub struct ConfigConfigurationSource;

impl ConfigConfigurationSource {
    /// Create a new source.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConfigConfigurationSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationSource for ConfigConfigurationSource {
    fn load(
        &self,
        base: Configuration,
    ) -> impl Future<Output = Result<Configuration, ConfigurationSourceError>> + Send {
        let result = (|| {
            let settings = Config::builder()
                .add_source(DbSqlite3Source::new(base))
                .add_source(File::with_name(USER_CONFIG_PATH).required(false))
                .add_source(
                    Environment::with_prefix("mnemorium")
                        .separator("__")
                        .try_parsing(true)
                        .ignore_empty(true),
                )
                .build()
                .map_err(|error| ConfigurationSourceError::Unknown(anyhow::anyhow!(error)))?;

            settings
                .try_deserialize::<Configuration>()
                .map_err(|error| {
                    ConfigurationSourceError::InvalidConfiguration(anyhow::anyhow!(error))
                })
        })();

        ready(result)
    }
}
