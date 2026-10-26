use std::env::vars;
use std::future::Future;
use std::future::ready;

use crate::domain::model::configuration::Rotation;
use crate::domain::model::partial_configuration::PartialAsset;
use crate::domain::model::partial_configuration::PartialAssetStorage;
use crate::domain::model::partial_configuration::PartialAssetUpload;
use crate::domain::model::partial_configuration::PartialConfiguration;
use crate::domain::model::partial_configuration::PartialJwt;
use crate::domain::model::partial_configuration::PartialLogging;
use crate::domain::model::partial_configuration::PartialPersistence;
use crate::domain::model::partial_configuration::PartialRateLimit;
use crate::domain::model::partial_configuration::PartialSecurity;
use crate::domain::model::partial_configuration::PartialSqlite3;
use crate::domain::port::configuration_source::EnvironmentConfiguration;
use crate::domain::port::error::ConfigurationSourceError;

/// Prefix every server environment variable carries.
const ENVIRONMENT_PREFIX: &str = "MNEMORIUM_";

/// The settings the environment names, in its flat variable shape.
///
/// The field name is the setting path in `snake_case`: `envy` strips
/// [`ENVIRONMENT_PREFIX`], lowercases the remainder and matches it against a
/// field, so `MNEMORIUM_LOGGING_LEVEL` lands on `logging_level` and
/// `MNEMORIUM_SECURITY_RATE_LIMIT_BURST_SIZE` on
/// `security_rate_limit_burst_size`. A name no field matches is ignored, never
/// rejected.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default)]
struct EnvConfiguration {
    /// `asset.storage.root`.
    asset_storage_root: Option<String>,
    /// `asset.upload.chunk_size_bytes`.
    asset_upload_chunk_size_bytes: Option<u64>,
    /// `asset.upload.expiry_seconds`.
    asset_upload_expiry_seconds: Option<u64>,
    /// `asset.upload.max_file_size_bytes`.
    asset_upload_max_file_size_bytes: Option<u64>,
    /// `logging.ansi`.
    logging_ansi: Option<bool>,
    /// `logging.level`.
    logging_level: Option<String>,
    /// `logging.max_files`.
    logging_max_files: Option<u32>,
    /// `logging.rotation`.
    logging_rotation: Option<Rotation>,
    /// `persistence.sqlite3.max_connections`.
    persistence_sqlite3_max_connections: Option<u32>,
    /// `security.jwt.ttl`.
    security_jwt_ttl: Option<u64>,
    /// `security.log_root_admin_password`.
    security_log_root_admin_password: Option<bool>,
    /// `security.rate_limit.burst_size`.
    security_rate_limit_burst_size: Option<u32>,
    /// `security.rate_limit.client_ip_header`.
    security_rate_limit_client_ip_header: Option<String>,
    /// `security.rate_limit.period_seconds`.
    security_rate_limit_period_seconds: Option<u64>,
    /// `security.rate_limit.trusted_proxies`.
    security_rate_limit_trusted_proxies: Option<String>,
}

impl From<EnvConfiguration> for PartialConfiguration {
    fn from(env: EnvConfiguration) -> Self {
        Self {
            asset: PartialAsset {
                storage: PartialAssetStorage {
                    root: env.asset_storage_root,
                },
                upload: PartialAssetUpload {
                    chunk_size_bytes: env.asset_upload_chunk_size_bytes,
                    expiry_seconds: env.asset_upload_expiry_seconds,
                    max_file_size_bytes: env.asset_upload_max_file_size_bytes,
                },
            },
            logging: PartialLogging {
                ansi: env.logging_ansi,
                level: env.logging_level,
                max_files: env.logging_max_files,
                rotation: env.logging_rotation,
            },
            persistence: PartialPersistence {
                sqlite3: PartialSqlite3 {
                    max_connections: env.persistence_sqlite3_max_connections,
                },
            },
            security: PartialSecurity {
                jwt: PartialJwt {
                    ttl: env.security_jwt_ttl,
                },
                log_root_admin_password: env.security_log_root_admin_password,
                rate_limit: PartialRateLimit {
                    burst_size: env.security_rate_limit_burst_size,
                    client_ip_header: env.security_rate_limit_client_ip_header,
                    period_seconds: env.security_rate_limit_period_seconds,
                    trusted_proxies: env.security_rate_limit_trusted_proxies,
                },
            },
        }
    }
}

/// Adapter loading the environment configuration layer.
///
/// The adapter only maps the variables onto the settings they name; the load
/// configuration use case merges the result over the file layer, so the
/// precedence never lives here.
#[derive(Debug, Default)]
pub struct EnvConfigurationSource;

impl EnvConfigurationSource {
    /// Create a new adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Read the settings from `variables`.
    ///
    /// The iterator is the seam the adapter tests use, so they never mutate the
    /// process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when a prefixed variable does not hold a value its
    /// setting accepts.
    pub fn read_from<I>(
        &self,
        variables: I,
    ) -> Result<PartialConfiguration, ConfigurationSourceError>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        envy::prefixed(ENVIRONMENT_PREFIX)
            .from_iter::<_, EnvConfiguration>(variables)
            .map(PartialConfiguration::from)
            .map_err(|error| {
                // envy renders the offending value inside its message, so only
                // the field name is kept: an environment value never reaches an
                // error chain (`OBS-004`).
                let variable = error.to_string().rsplit_once(" provided by ").map_or_else(
                    || format!("{ENVIRONMENT_PREFIX}*"),
                    |(_message, field)| format!("{ENVIRONMENT_PREFIX}{}", field.trim()),
                );
                ConfigurationSourceError::InvalidConfiguration(anyhow::anyhow!(
                    "the environment variable {variable} does not hold a value this setting accepts"
                ))
            })
    }
}

impl EnvironmentConfiguration for EnvConfigurationSource {
    fn load(
        &self,
    ) -> impl Future<Output = Result<PartialConfiguration, ConfigurationSourceError>> + Send {
        ready(self.read_from(vars()))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::iter::empty;

    use crate::domain::model::configuration::Rotation;
    use crate::domain::model::partial_configuration::PartialConfiguration;
    use crate::domain::port::error::ConfigurationSourceError;

    use super::EnvConfigurationSource;

    /// Build a (name, value) pair the way the process environment yields it.
    fn variable(name: &str, value: &str) -> (String, String) {
        (name.to_owned(), value.to_owned())
    }

    #[test]
    fn environment_configuration_absent_set_names_no_setting() -> Result<(), Box<dyn Error>> {
        // Arrange
        let adapter = EnvConfigurationSource::new();

        // Act
        let partial = adapter.read_from(empty())?;

        // Assert
        assert_eq!(partial, PartialConfiguration::default());
        Ok(())
    }

    #[test]
    fn environment_configuration_every_setting_maps_to_its_variable() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let variables = vec![
            variable("MNEMORIUM_ASSET_STORAGE_ROOT", "media"),
            variable("MNEMORIUM_ASSET_UPLOAD_CHUNK_SIZE_BYTES", "1024"),
            variable("MNEMORIUM_ASSET_UPLOAD_EXPIRY_SECONDS", "60"),
            variable("MNEMORIUM_ASSET_UPLOAD_MAX_FILE_SIZE_BYTES", "2048"),
            variable("MNEMORIUM_LOGGING_ANSI", "true"),
            variable("MNEMORIUM_LOGGING_LEVEL", "info"),
            variable("MNEMORIUM_LOGGING_MAX_FILES", "2"),
            variable("MNEMORIUM_LOGGING_ROTATION", "HOURLY"),
            variable("MNEMORIUM_PERSISTENCE_SQLITE3_MAX_CONNECTIONS", "2"),
            variable("MNEMORIUM_SECURITY_JWT_TTL", "120"),
            variable("MNEMORIUM_SECURITY_LOG_ROOT_ADMIN_PASSWORD", "false"),
            variable("MNEMORIUM_SECURITY_RATE_LIMIT_BURST_SIZE", "9"),
            variable(
                "MNEMORIUM_SECURITY_RATE_LIMIT_CLIENT_IP_HEADER",
                "x-real-ip",
            ),
            variable("MNEMORIUM_SECURITY_RATE_LIMIT_PERIOD_SECONDS", "30"),
            variable("MNEMORIUM_SECURITY_RATE_LIMIT_TRUSTED_PROXIES", "10.0.0.1"),
        ];

        // Act
        let partial = adapter(variables)?;

        // Assert
        assert_eq!(partial.asset.storage.root.as_deref(), Some("media"));
        assert_eq!(partial.asset.upload.chunk_size_bytes, Some(1024));
        assert_eq!(partial.asset.upload.expiry_seconds, Some(60));
        assert_eq!(partial.asset.upload.max_file_size_bytes, Some(2048));
        assert_eq!(partial.logging.ansi, Some(true));
        assert_eq!(partial.logging.level.as_deref(), Some("info"));
        assert_eq!(partial.logging.max_files, Some(2));
        assert_eq!(partial.logging.rotation, Some(Rotation::Hourly));
        assert_eq!(partial.persistence.sqlite3.max_connections, Some(2));
        assert_eq!(partial.security.jwt.ttl, Some(120));
        assert_eq!(partial.security.log_root_admin_password, Some(false));
        assert_eq!(partial.security.rate_limit.burst_size, Some(9));
        assert_eq!(
            partial.security.rate_limit.client_ip_header.as_deref(),
            Some("x-real-ip")
        );
        assert_eq!(partial.security.rate_limit.period_seconds, Some(30));
        assert_eq!(
            partial.security.rate_limit.trusted_proxies.as_deref(),
            Some("10.0.0.1")
        );
        Ok(())
    }

    #[test]
    fn environment_configuration_unknown_names_are_discarded() -> Result<(), Box<dyn Error>> {
        // Arrange
        let variables = [
            variable("MNEMORIUM_PEPPER", &"a".repeat(64)),
            variable("MNEMORIUM_NOT_A_SETTING", "1"),
            variable("PATH", "/usr/bin"),
        ];

        // Act
        let partial = adapter(variables)?;

        // Assert
        assert_eq!(
            partial,
            PartialConfiguration::default(),
            "a discarded name leaves no trace"
        );
        Ok(())
    }

    #[test]
    fn environment_configuration_badly_typed_value_names_the_variable() {
        // Arrange
        let adapter = EnvConfigurationSource::new();

        // Act
        let error = adapter
            .read_from([variable("MNEMORIUM_LOGGING_MAX_FILES", "many")])
            .err();

        // Assert
        let message = error.as_ref().map(ToString::to_string).unwrap_or_default();
        assert!(
            message.contains("MNEMORIUM_LOGGING_MAX_FILES"),
            "the rejection must name the variable, got: {message:?}"
        );
        assert!(
            !message.contains("many"),
            "the rejection must not carry the value, got: {message:?}"
        );
    }

    #[test]
    fn environment_configuration_rotation_parses_the_uppercase_form() -> Result<(), Box<dyn Error>>
    {
        // Act
        let partial = adapter([variable("MNEMORIUM_LOGGING_ROTATION", "MINUTELY")])?;

        // Assert
        assert_eq!(partial.logging.rotation, Some(Rotation::Minutely));
        Ok(())
    }

    /// Read a partial from the given pairs.
    fn adapter(
        variables: impl IntoIterator<Item = (String, String)>,
    ) -> Result<PartialConfiguration, ConfigurationSourceError> {
        EnvConfigurationSource::new().read_from(variables)
    }
}
