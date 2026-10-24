use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::fs::Permissions;
use std::fs::read_to_string;
use std::fs::rename;
use std::future::Future;
use std::future::ready;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::path::PathBuf;

use yaml_serde::Mapping;
use yaml_serde::Value;

use crate::application::port::load_instance_state::JWT_SECRET_VARIABLE;
use crate::application::port::load_instance_state::PEPPER_VARIABLE;
use crate::domain::model::configuration::Configuration;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::configuration_source::LoadedConfiguration;
use crate::domain::port::error::ConfigurationSourceError;

/// Path to the user configuration file.
pub(crate) const USER_CONFIG_PATH: &str = "config.yaml";

/// Default configuration file written when none exists.
const DEFAULT_CONFIG_TEMPLATE: &str = include_str!("default_config.yaml");

/// Mode of the generated configuration file.
const CONFIG_FILE_MODE: u32 = 0o644;

/// Prefix every environment override carries.
const ENVIRONMENT_PREFIX: &str = "MNEMORIUM_";

/// Environment variables that seed the identity instead of configuring it.
///
/// The names come from the identity port, so a rename cannot leave the resolver
/// accepting a variable the composition root no longer reads.
const IDENTITY_SEED_VARIABLES: [&str; 2] = [PEPPER_VARIABLE, JWT_SECRET_VARIABLE];

/// Separator between the key segments of one setting.
const KEY_SEPARATOR: char = '.';

/// The documented environment overrides, one name per setting.
const ENVIRONMENT_SETTINGS: &[EnvironmentSetting] = &[
    EnvironmentSetting {
        key: "logging.level",
        kind: EnvironmentKind::Text,
        name: "MNEMORIUM_LOG_LEVEL",
    },
    EnvironmentSetting {
        key: "logging.ansi",
        kind: EnvironmentKind::Boolean,
        name: "MNEMORIUM_LOG_ANSI",
    },
    EnvironmentSetting {
        key: "logging.max_files",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_LOG_MAX_FILES",
    },
    EnvironmentSetting {
        key: "logging.rotation",
        kind: EnvironmentKind::Text,
        name: "MNEMORIUM_LOG_ROTATION",
    },
    EnvironmentSetting {
        key: "persistence.sqlite3.max_connections",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_DATABASE_MAX_CONNECTIONS",
    },
    EnvironmentSetting {
        key: "asset.storage.root",
        kind: EnvironmentKind::Text,
        name: "MNEMORIUM_ASSET_STORAGE_ROOT",
    },
    EnvironmentSetting {
        key: "asset.upload.chunk_size_bytes",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_ASSET_UPLOAD_CHUNK_SIZE_BYTES",
    },
    EnvironmentSetting {
        key: "asset.upload.expiry_seconds",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_ASSET_UPLOAD_EXPIRY_SECONDS",
    },
    EnvironmentSetting {
        key: "asset.upload.max_file_size_bytes",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_ASSET_UPLOAD_MAX_FILE_SIZE_BYTES",
    },
    EnvironmentSetting {
        key: "security.jwt.ttl",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_JWT_TTL",
    },
    EnvironmentSetting {
        key: "security.log_root_admin_password",
        kind: EnvironmentKind::Boolean,
        name: "MNEMORIUM_LOG_ROOT_ADMIN_PASSWORD",
    },
    EnvironmentSetting {
        key: "security.rate_limit.burst_size",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_RATE_LIMIT_BURST_SIZE",
    },
    EnvironmentSetting {
        key: "security.rate_limit.client_ip_header",
        kind: EnvironmentKind::Text,
        name: "MNEMORIUM_RATE_LIMIT_CLIENT_IP_HEADER",
    },
    EnvironmentSetting {
        key: "security.rate_limit.period_seconds",
        kind: EnvironmentKind::Integer,
        name: "MNEMORIUM_RATE_LIMIT_PERIOD_SECONDS",
    },
    EnvironmentSetting {
        key: "security.rate_limit.trusted_proxies",
        kind: EnvironmentKind::Text,
        name: "MNEMORIUM_RATE_LIMIT_TRUSTED_PROXIES",
    },
];

/// Declared type of an environment override.
#[derive(Debug, Clone, Copy)]
enum EnvironmentKind {
    /// A boolean value.
    Boolean,
    /// An unsigned integer value.
    Integer,
    /// A string value.
    Text,
}

/// One documented environment override.
#[derive(Debug)]
struct EnvironmentSetting {
    /// Configuration key the variable overrides.
    key: &'static str,
    /// Declared type of the value.
    kind: EnvironmentKind,
    /// Environment variable name.
    name: &'static str,
}

/// Source resolving defaults, the configuration file and the environment.
///
/// The defaults are the serde defaults of the domain models, so an absent,
/// empty or partial document is still a complete configuration; the file
/// overrides them; the explicit environment table overrides both. The file and
/// the overrides are merged in one YAML document, and the merged document is
/// validated by `yaml_serde`, so `deny_unknown_fields` and the per-field
/// defaults still do the work. No layer reads the datastore.
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
    ) -> impl Future<Output = Result<LoadedConfiguration, ConfigurationSourceError>> + Send {
        let resolver = Resolver {
            environment: env::vars().collect::<Vec<(String, String)>>(),
            path: Path::new(USER_CONFIG_PATH),
        };
        ready(resolver.resolve())
    }
}

/// One configuration resolution over an injectable path and environment.
///
/// The path and the environment are constructor arguments so the adapter tests
/// never mutate process-global state.
struct Resolver<'environment> {
    /// Environment variables visible to this resolution.
    environment: Vec<(String, String)>,
    /// Path of the configuration file.
    path: &'environment Path,
}

impl Resolver<'_> {
    /// Read the configuration file into a YAML document.
    ///
    /// An absent or empty file yields an empty mapping, so every setting keeps
    /// its default. A document that is not valid YAML fails startup instead of
    /// falling back to the defaults.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or is not valid YAML.
    fn document(&self) -> Result<Value, ConfigurationSourceError> {
        let contents = match read_to_string(self.path) {
            Ok(contents) => contents,
            Err(failure) if failure.kind() == ErrorKind::NotFound => {
                return Ok(Value::Mapping(Mapping::new()));
            }
            Err(failure) => {
                return Err(ConfigurationSourceError::Unknown(anyhow::anyhow!(
                    "cannot read {}: {failure}",
                    self.path.display()
                )));
            }
        };
        if contents.trim().is_empty() {
            return Ok(Value::Mapping(Mapping::new()));
        }
        let document = yaml_serde::from_str::<Value>(&contents).map_err(|failure| {
            ConfigurationSourceError::InvalidConfiguration(anyhow::Error::new(failure))
        })?;
        // An explicitly empty document is the same as an absent one: the
        // per-field defaults cover a missing key, not a non-mapping root.
        if document.is_null() {
            return Ok(Value::Mapping(Mapping::new()));
        }
        Ok(document)
    }

    /// Write the embedded default template when the file is absent.
    ///
    /// A file that already exists is never rewritten, including an empty or
    /// partial one. The write goes through a sibling temp file and a rename, so
    /// a crash never leaves a half-written document in place.
    fn ensure_default_file(&self) -> Result<bool, ConfigurationSourceError> {
        if self.path.exists() {
            return Ok(false);
        }

        let temporary = self.sibling(".tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(CONFIG_FILE_MODE)
            .open(&temporary)
            .map_err(|failure| self.write_failure(&failure))?;
        file.set_permissions(Permissions::from_mode(CONFIG_FILE_MODE))
            .map_err(|failure| self.write_failure(&failure))?;
        file.write_all(DEFAULT_CONFIG_TEMPLATE.as_bytes())
            .map_err(|failure| self.write_failure(&failure))?;
        file.sync_all()
            .map_err(|failure| self.write_failure(&failure))?;
        drop(file);
        rename(&temporary, self.path).map_err(|failure| self.write_failure(&failure))?;
        Ok(true)
    }

    /// Resolve the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when the default file cannot be written, the
    /// configuration file cannot be read or parsed, an `MNEMORIUM_*` variable
    /// matches no setting, a declared variable cannot be parsed, or the merged
    /// document is not a valid configuration.
    fn resolve(&self) -> Result<LoadedConfiguration, ConfigurationSourceError> {
        let file_created = self.ensure_default_file()?;
        let mut document = self.document()?;

        for (name, raw) in self
            .environment
            .iter()
            .map(|pair| (pair.0.as_str(), pair.1.as_str()))
        {
            if !name.starts_with(ENVIRONMENT_PREFIX) {
                continue;
            }
            if IDENTITY_SEED_VARIABLES.contains(&name) {
                continue;
            }
            let Some(setting) = ENVIRONMENT_SETTINGS.iter().find(|entry| entry.name == name) else {
                return Err(ConfigurationSourceError::InvalidConfiguration(
                    anyhow::anyhow!(
                        "the environment variable {name} matches no configuration setting"
                    ),
                ));
            };
            let value = match setting.kind {
                EnvironmentKind::Boolean => raw.parse::<bool>().map_or_else(
                    |_| {
                        Err(ConfigurationSourceError::InvalidConfiguration(
                            anyhow::anyhow!(
                                "the environment variable {} must be a boolean value",
                                setting.name
                            ),
                        ))
                    },
                    |parsed| Ok(Value::Bool(parsed)),
                )?,
                EnvironmentKind::Integer => raw.parse::<u64>().map_or_else(
                    |_| {
                        Err(ConfigurationSourceError::InvalidConfiguration(
                            anyhow::anyhow!(
                                "the environment variable {} must be an unsigned integer",
                                setting.name
                            ),
                        ))
                    },
                    |parsed| Ok(Value::Number(parsed.into())),
                )?,
                EnvironmentKind::Text => Value::String(raw.to_owned()),
            };

            // Descend the document one key per `.`-segment, creating a missing
            // mapping on the way down. A parent key holding a value that is
            // neither a mapping nor absent cannot be descended into, so it is
            // reported instead of aborting the process.
            let mut target = &mut document;
            for segment in setting.key.split(KEY_SEPARATOR) {
                if target.is_null() {
                    *target = Value::Mapping(Mapping::new());
                }
                let Some(mapping) = target.as_mapping_mut() else {
                    return Err(ConfigurationSourceError::InvalidConfiguration(
                        anyhow::anyhow!(
                            "the environment variable {} cannot override {} because the configuration file sets a parent key to a non-mapping value",
                            setting.name,
                            setting.key
                        ),
                    ));
                };
                target = mapping
                    .entry(Value::String(segment.to_owned()))
                    .or_insert(Value::Null);
            }
            *target = value;
        }

        let configuration =
            yaml_serde::from_value::<Configuration>(document).map_err(|failure| {
                ConfigurationSourceError::InvalidConfiguration(anyhow::Error::new(failure))
            })?;

        Ok(LoadedConfiguration::new(configuration, file_created))
    }

    /// Build the sibling path carrying `suffix` after the file name.
    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut name = self
            .path
            .file_name()
            .map_or_else(OsString::new, OsStr::to_os_string);
        name.push(suffix);
        self.path.with_file_name(name)
    }

    /// Build the error returned when the default file cannot be written.
    fn write_failure(&self, failure: &Error) -> ConfigurationSourceError {
        ConfigurationSourceError::FileWriteFailed(anyhow::anyhow!(
            "cannot write {}: {failure}",
            self.path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::iter::repeat_n;
    use std::path::Path;

    use tempfile::tempdir;
    use yaml_serde::Value;

    use crate::domain::model::configuration::Configuration;
    use crate::domain::port::error::ConfigurationSourceError;

    use super::DEFAULT_CONFIG_TEMPLATE;
    use super::Resolver;

    /// Build a resolution over `path` and a synthetic environment.
    fn resolver<'environment>(
        path: &'environment Path,
        environment: &[(&str, &str)],
    ) -> Resolver<'environment> {
        Resolver {
            environment: environment
                .iter()
                .map(|pair| (pair.0.to_owned(), pair.1.to_owned()))
                .collect(),
            path,
        }
    }

    /// A 64-character hexadecimal string.
    fn hex64(character: char) -> String {
        repeat_n(character, 64).collect()
    }

    #[test]
    fn configuration_source_absent_file_writes_the_defaults() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let resolver = resolver(&path, &[]);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(
            loaded.file_created(),
            "an absent configuration file must be generated"
        );
        assert_eq!(loaded.configuration(), &Configuration::default());
        assert_eq!(
            fs::read_to_string(&path)?,
            DEFAULT_CONFIG_TEMPLATE,
            "the generated file must carry the embedded defaults"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_empty_file_yields_the_defaults() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "")?;
        let resolver = resolver(&path, &[]);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(!loaded.file_created(), "an existing file is not rewritten");
        assert_eq!(loaded.configuration(), &Configuration::default());
        Ok(())
    }

    #[test]
    fn configuration_source_existing_file_is_not_rewritten() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let original = "logging:\n  level: info\n";
        fs::write(&path, original)?;
        let resolver = resolver(&path, &[]);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(!loaded.file_created(), "an existing file is not rewritten");
        assert_eq!(fs::read_to_string(&path)?, original);
        assert_eq!(
            loaded.configuration().logging().level(),
            "info",
            "the file value must be effective"
        );
        assert_eq!(
            loaded.configuration().security().jwt().ttl(),
            Configuration::default().security().jwt().ttl(),
            "a setting the file omits must keep its default"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_environment_overrides_the_file() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "logging:\n  level: info\n")?;
        let resolver = resolver(&path, &[("MNEMORIUM_LOG_LEVEL", "warn")]);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert_eq!(
            loaded.configuration().logging().level(),
            "warn",
            "the environment must win over the file"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_environment_creates_a_missing_section() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "logging:\n  level: info\n")?;
        let resolver = resolver(&path, &[("MNEMORIUM_RATE_LIMIT_BURST_SIZE", "9")]);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert_eq!(
            loaded.configuration().security().rate_limit().burst_size(),
            9,
            "an override must create the missing sections on its way down"
        );
        assert_eq!(
            loaded
                .configuration()
                .security()
                .rate_limit()
                .period_seconds(),
            Configuration::default()
                .security()
                .rate_limit()
                .period_seconds(),
            "the sibling settings must keep their defaults"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_unknown_variable_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let resolver = resolver(&path, &[("MNEMORIUM_NOPE", "1")]);

        // Act
        let message = resolver
            .resolve()
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("MNEMORIUM_NOPE"),
            "an unknown variable must fail naming it, got: {message:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_malformed_value_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let resolver = resolver(&path, &[("MNEMORIUM_LOG_MAX_FILES", "many")]);

        // Act
        let message = resolver
            .resolve()
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("MNEMORIUM_LOG_MAX_FILES"),
            "a malformed value must fail naming it, got: {message:?}"
        );
        assert!(
            !message.contains('7'),
            "no default may be substituted for a malformed value, got: {message:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_override_of_a_scalar_parent_is_rejected() -> Result<(), Box<dyn Error>>
    {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "logging: info\n")?;
        let resolver = resolver(&path, &[("MNEMORIUM_LOG_LEVEL", "warn")]);

        // Act
        let message = resolver
            .resolve()
            .map_err(|error| error.to_string())
            .err()
            .unwrap_or_default();

        // Assert
        assert!(
            message.contains("MNEMORIUM_LOG_LEVEL"),
            "a non-mapping parent must fail naming the variable, got: {message:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_unknown_file_key_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "nope: 1\n")?;
        let resolver = resolver(&path, &[]);

        // Act
        let result = resolver.resolve();

        // Assert
        assert!(
            matches!(
                result,
                Err(ConfigurationSourceError::InvalidConfiguration(_))
            ),
            "an unknown key must fail instead of being ignored, got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_identity_seed_is_allowed() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let pepper = hex64('a');
        let jwt_secret = hex64('b');
        let resolver = resolver(
            &path,
            &[
                ("MNEMORIUM_PEPPER", pepper.as_str()),
                ("MNEMORIUM_JWT_SECRET", jwt_secret.as_str()),
            ],
        );

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert_eq!(
            loaded.configuration(),
            &Configuration::default(),
            "the identity seeds are not configuration settings"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_unwritable_path_fails() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("missing").join("config.yaml");
        let resolver = resolver(&path, &[]);

        // Act
        let result = resolver.resolve();

        // Assert
        assert!(
            matches!(result, Err(ConfigurationSourceError::FileWriteFailed(_))),
            "an unwritable location must fail startup, got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_default_template_matches_the_domain_defaults()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let expected = yaml_serde::to_value(Configuration::default())?;

        // Act
        let rendered = yaml_serde::from_str::<Value>(DEFAULT_CONFIG_TEMPLATE)?;

        // Assert
        assert_eq!(
            rendered, expected,
            "the checked-in template must carry exactly the built-in defaults"
        );
        Ok(())
    }
}
