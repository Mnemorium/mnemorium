use config::Config;
use config::ConfigError;
use config::Environment;
use config::File;

use crate::application::use_case::load_configuration::DEFAULT_SQLITE3_PATH;
use crate::application::use_case::load_configuration::default_sqlite3_max_conn;
use crate::domain::model::sqlite3::Sqlite3;
use crate::infrastructure::outbound::config::configuration_source::USER_CONFIG_PATH;

// TODO(refactor): the bootstrap reads the file and the environment here because
// the datastore path is needed before the configuration singleton (which lives
// in the datastore) can be read. If the bootstrap grows beyond the persistence
// settings, promote it to a use case (a `LoadBootstrapConfiguration` port +
// adapter run by `main` before the pool is opened), or expose it as a method on
// the `ConfigurationSource` port. Either would move this layering behind a port
// and let `main` stop touching the `config` crate entirely. The file and
// environment layering is intentionally duplicated with
// `ConfigConfigurationSource` (see the extraction rule in `docs/development/TechnicalDesign.md` § 1); keep
// the source list and order identical.
/// Read the persistence settings from the user configuration file and the
/// environment, before the datastore is reachable.
///
/// The datastore path is needed to open the pool, but the configuration
/// singleton row lives behind that pool: this bootstrap reads only the file and
/// the environment, so the pool can be opened before `LoadConfiguration` runs.
///
/// # Errors
///
/// Returns an error when the layered sources cannot be read or parsed, or when
/// the resolved settings are invalid. A setting that is absent falls back to
/// its default; a setting that is present but cannot be read or is invalid
/// fails startup.
pub fn bootstrap_sqlite3() -> anyhow::Result<Sqlite3> {
    let settings = Config::builder()
        .add_source(File::with_name(USER_CONFIG_PATH).required(false))
        .add_source(
            Environment::with_prefix("mnemorium")
                .separator("__")
                .try_parsing(true)
                .ignore_empty(true),
        )
        .build()?;
    resolve_sqlite3(&settings)
}

/// Resolve the persistence settings from a layered configuration.
///
/// This is the seam the bootstrap exposes for tests: [`bootstrap_sqlite3`]
/// builds the file-and-environment [`Config`] and this function applies the
/// absent-versus-malformed rule to it, so the rule can be driven in-process
/// with an in-memory [`Config`] instead of mutating process-global state.
///
/// # Errors
///
/// Returns an error when a setting is present but cannot be read, is invalid,
/// or is out of range. A setting that is absent falls back to its default.
#[cfg_attr(
    not(test),
    expect(
        clippy::single_call_fn,
        reason = "the resolution rule is named after the configuration rule it enforces"
    )
)]
fn resolve_sqlite3(settings: &Config) -> anyhow::Result<Sqlite3> {
    let path = match settings.get_string("persistence.sqlite3.path") {
        Ok(path) => path,
        Err(ConfigError::NotFound(_)) => DEFAULT_SQLITE3_PATH.to_owned(),
        Err(error) => return Err(error.into()),
    };
    let max_connections = match settings.get_int("persistence.sqlite3.max_connections") {
        Ok(value) => u32::try_from(value)?,
        Err(ConfigError::NotFound(_)) => default_sqlite3_max_conn(),
        Err(error) => return Err(error.into()),
    };
    Ok(Sqlite3::try_new(path, max_connections)?)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use config::Config;
    use config::File;
    use config::FileFormat;

    use crate::application::use_case::load_configuration::DEFAULT_SQLITE3_PATH;
    use crate::application::use_case::load_configuration::default_sqlite3_max_conn;

    use super::resolve_sqlite3;

    /// Build an in-memory layered configuration from a JSON object.
    fn settings(json: &str) -> Result<Config, config::ConfigError> {
        Config::builder()
            .add_source(File::from_str(json, FileFormat::Json))
            .build()
    }

    #[test]
    fn resolve_sqlite3_absent_settings_uses_defaults() -> Result<(), Box<dyn Error>> {
        // Arrange
        let settings = settings("{}")?;

        // Act
        let sqlite3 = resolve_sqlite3(&settings)?;

        // Assert
        assert_eq!(sqlite3.path(), DEFAULT_SQLITE3_PATH);
        assert_eq!(sqlite3.max_connections(), default_sqlite3_max_conn());
        Ok(())
    }

    #[test]
    fn resolve_sqlite3_valid_settings_returns_configured_values() -> Result<(), Box<dyn Error>> {
        // Arrange
        let settings =
            settings(r#"{"persistence":{"sqlite3":{"path":"custom.db","max_connections":4}}}"#)?;

        // Act
        let sqlite3 = resolve_sqlite3(&settings)?;

        // Assert
        assert_eq!(sqlite3.path(), "custom.db");
        assert_eq!(sqlite3.max_connections(), 4);
        Ok(())
    }

    #[test]
    fn resolve_sqlite3_present_max_connections_zero_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let settings =
            settings(r#"{"persistence":{"sqlite3":{"path":"mnemorium.db","max_connections":0}}}"#)?;

        // Act
        let result = resolve_sqlite3(&settings);

        // Assert
        assert!(result.is_err(), "a zero max_connections must be rejected");
        Ok(())
    }

    #[test]
    fn resolve_sqlite3_present_path_empty_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let settings = settings(r#"{"persistence":{"sqlite3":{"path":"","max_connections":4}}}"#)?;

        // Act
        let result = resolve_sqlite3(&settings);

        // Assert
        assert!(result.is_err(), "an empty path must be rejected");
        Ok(())
    }

    #[test]
    fn resolve_sqlite3_present_max_connections_wrong_type_is_rejected() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let settings = settings(
            r#"{"persistence":{"sqlite3":{"path":"mnemorium.db","max_connections":"many"}}}"#,
        )?;

        // Act
        let result = resolve_sqlite3(&settings);

        // Assert
        assert!(
            result.is_err(),
            "a wrongly typed max_connections must be rejected"
        );
        Ok(())
    }

    #[test]
    fn resolve_sqlite3_present_max_connections_out_of_range_is_rejected()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let settings = settings(
            r#"{"persistence":{"sqlite3":{"path":"mnemorium.db","max_connections":5000000000}}}"#,
        )?;

        // Act
        let result = resolve_sqlite3(&settings);

        // Assert
        assert!(
            result.is_err(),
            "a max_connections above u32::MAX must be rejected"
        );
        Ok(())
    }
}
