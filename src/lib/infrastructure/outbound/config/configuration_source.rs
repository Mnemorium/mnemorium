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
pub(crate) const USER_CONFIG_PATH: &str = "config.yaml";

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
            .map_err(|error| ConfigurationSourceError::Unknown(anyhow::anyhow!(error)));

        ready(settings.and_then(deserialize))
    }
}

/// Deserialize a layered configuration snapshot, validating it through the
/// domain models.
///
/// Extracted from [`ConfigConfigurationSource::load`] as the test seam: the
/// file-and-environment wiring stays in `load`, and the validation mapping can
/// be driven in-process with an in-memory [`Config`].
///
/// # Errors
///
/// Returns [`ConfigurationSourceError::InvalidConfiguration`] when a value is
/// present but fails the domain validation.
#[cfg_attr(
    not(test),
    expect(
        clippy::single_call_fn,
        reason = "the deserialization rule is named after the configuration rule it enforces"
    )
)]
fn deserialize(settings: Config) -> Result<Configuration, ConfigurationSourceError> {
    settings
        .try_deserialize::<Configuration>()
        .map_err(|error| ConfigurationSourceError::InvalidConfiguration(anyhow::anyhow!(error)))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use config::Config;
    use config::File;
    use config::FileFormat;

    use crate::domain::model::asset::Asset;
    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::jwt::Jwt;
    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::Rotation;
    use crate::domain::model::persistence::Persistence;
    use crate::domain::model::security::Security;
    use crate::domain::model::sqlite3::Sqlite3;
    use crate::domain::port::error::ConfigurationSourceError;

    use super::deserialize;

    /// A valid configuration used to drive the deserialization seam.
    fn configuration() -> Result<Configuration, Box<dyn Error>> {
        let jwt = Jwt::try_new("a".repeat(64), 3600)?;
        let security = Security::try_new(jwt, "b".repeat(64), true)?;
        let sqlite3 = Sqlite3::try_new("mnemorium.db".to_owned(), 1)?;
        let persistence = Persistence::new(sqlite3);
        let logging = Logging::try_new(false, "debug,sqlx=warn".to_owned(), 7, Rotation::Daily)?;
        Ok(Configuration::new(
            persistence,
            security,
            logging,
            Asset::default(),
        ))
    }

    /// Build an in-memory layered configuration from a JSON object.
    fn settings(json: &str) -> Result<Config, config::ConfigError> {
        Config::builder()
            .add_source(File::from_str(json, FileFormat::Json))
            .build()
    }

    #[test]
    fn deserialize_valid_configuration_returns_configuration() -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = configuration()?;
        let settings = settings(&serde_json::to_string(&expected)?)?;

        // Act
        let actual = deserialize(settings)?;

        // Assert
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn deserialize_present_invalid_pepper_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        // The valid snapshot carries a 64-character pepper; make it malformed
        // so the domain validation, not the layout, is what fails.
        let valid = serde_json::to_string(&configuration()?)?;
        let settings = settings(&valid.replace(&"b".repeat(64), "not-a-pepper"))?;

        // Act
        let result = deserialize(settings);

        // Assert
        assert!(matches!(
            result,
            Err(ConfigurationSourceError::InvalidConfiguration(_))
        ));
        Ok(())
    }
}
