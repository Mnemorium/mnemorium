use std::ffi::{OsStr, OsString};
use std::fs::{OpenOptions, Permissions, read_to_string, rename};
use std::future::{Future, ready};
use std::io::{Error, ErrorKind, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use yaml_serde::{Mapping, Value};

use crate::domain::model::PartialConfiguration;
use crate::domain::port::{ConfigurationSource, ConfigurationSourceError, LoadedFileConfiguration};

/// Path to the user configuration file.
const USER_CONFIG_PATH: &str = "config.yaml";

/// Default configuration file written when none exists.
const DEFAULT_CONFIG_TEMPLATE: &str = include_str!("default_config.yaml");

/// Mode of the generated configuration file.
const CONFIG_FILE_MODE: u32 = 0o644;

/// Source loading the configuration file layer.
///
/// The source reads `config.yaml` into the partial configuration the file
/// names. It does not merge: the load configuration use case merges the layer
/// over the built-in defaults, and `yaml_serde` keeps `deny_unknown_fields`, so
/// an unknown key still fails startup. No layer reads the datastore.
pub(crate) struct FileConfigurationSource;

impl FileConfigurationSource {
    /// Create a new source.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for FileConfigurationSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigurationSource for FileConfigurationSource {
    fn load(
        &self,
    ) -> impl Future<Output = Result<LoadedFileConfiguration, ConfigurationSourceError>> + Send
    {
        let resolver = Resolver {
            path: Path::new(USER_CONFIG_PATH),
        };
        ready(resolver.resolve())
    }
}

/// One configuration file read over an injectable path.
///
/// The path is a constructor argument so the adapter tests use a temporary file
/// instead of the working directory.
struct Resolver<'path> {
    /// Path of the configuration file.
    path: &'path Path,
}

impl Resolver<'_> {
    /// Read the configuration file into a YAML document.
    ///
    /// An absent or empty file yields an empty mapping, so the layer names no
    /// setting and the use case keeps the values of the layer below. A document
    /// that is not valid YAML fails startup instead of falling back.
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

    /// Read the file layer.
    ///
    /// # Errors
    ///
    /// Returns an error when the default file cannot be written, the
    /// configuration file cannot be read or parsed, or it names an unknown key.
    fn resolve(&self) -> Result<LoadedFileConfiguration, ConfigurationSourceError> {
        let file_created = self.ensure_default_file()?;
        let document = self.document()?;
        let partial =
            yaml_serde::from_value::<PartialConfiguration>(document).map_err(|failure| {
                ConfigurationSourceError::InvalidConfiguration(anyhow::Error::new(failure))
            })?;
        Ok(LoadedFileConfiguration::new(partial, file_created))
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
    use super::*;
    use crate::domain::model::Configuration;

    use std::error::Error;
    use std::fs;
    use tempfile::tempdir;

    /// Build a resolution over `path`.
    fn resolver(path: &Path) -> Resolver<'_> {
        Resolver { path }
    }
    #[test]
    fn configuration_source_mistyped_provider_list_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange: the list is a scalar instead of a sequence of entries.
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(
            &path,
            "library:\n  metadata:\n    video:\n      providers: tmdb\n",
        )?;
        let resolver = resolver(&path);

        // Act
        let result = resolver.resolve();

        // Assert
        assert!(
            matches!(
                result,
                Err(ConfigurationSourceError::InvalidConfiguration(_))
            ),
            "a value of the wrong type must fail, got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_unknown_file_key_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "nope: 1\n")?;
        let resolver = resolver(&path);

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
    fn configuration_source_removed_storage_root_key_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange: the setting an earlier release published is now unknown, so
        // an operator who kept it in `config.yaml` must delete it.
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "asset:\n  storage:\n    root: data\n")?;
        let resolver = resolver(&path);

        // Act
        let result = resolver.resolve();

        // Assert
        let message = result
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(
            matches!(
                result,
                Err(ConfigurationSourceError::InvalidConfiguration(_))
            ),
            "the retired key must fail instead of being ignored, got: {result:?}"
        );
        assert!(
            message.contains("storage"),
            "the rejection must name the retired key, got: {message:?}"
        );
        Ok(())
    }
    #[cfg(not(feature = "test-seed"))]
    #[test]
    fn configuration_source_seed_key_is_rejected_without_the_feature() -> Result<(), Box<dyn Error>>
    {
        // Arrange: the seed section ships only in a test build.
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "seed:\n  profile: api\n")?;
        let resolver = resolver(&path);

        // Act
        let result = resolver.resolve();

        // Assert
        assert!(
            matches!(
                result,
                Err(ConfigurationSourceError::InvalidConfiguration(_))
            ),
            "a shipped build must reject the test-only key as unknown, got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_malformed_document_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "logging: [unclosed\n")?;
        let resolver = resolver(&path);

        // Act
        let result = resolver.resolve();

        // Assert
        assert!(
            matches!(
                result,
                Err(ConfigurationSourceError::InvalidConfiguration(_))
            ),
            "a malformed document must fail instead of reverting to the defaults, got: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_mistyped_file_value_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "logging:\n  max_files: many\n")?;
        let resolver = resolver(&path);

        // Act
        let result = resolver.resolve();

        // Assert
        assert!(
            matches!(
                result,
                Err(ConfigurationSourceError::InvalidConfiguration(_))
            ),
            "a value of the wrong type must fail, got: {result:?}"
        );
        Ok(())
    }

    // proves: SCN-CONFIGURATION-004-D — Error handling
    #[test]
    fn configuration_source_unreadable_path_is_rejected() -> Result<(), Box<dyn Error>> {
        // Arrange: a directory stands in for `config.yaml`, so the path exists but holds no readable document.
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::create_dir_all(&path)?;
        let resolver = resolver(&path);

        // Act
        let result = resolver.resolve();

        // Assert
        assert!(
            matches!(result, Err(ConfigurationSourceError::Unknown(_))),
            "a file that cannot be read must fail startup, got: {result:?}"
        );
        let message = result
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(
            message.contains(&path.display().to_string()),
            "the refusal must name the path it could not read, got: {message:?}"
        );
        assert!(
            !message.contains("the configuration is invalid"),
            "an unreadable file must not be reported as an invalid document, got: {message:?}"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_unwritable_path_fails() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("missing").join("config.yaml");
        let resolver = resolver(&path);

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
        let metadata = rendered
            .get("library")
            .and_then(|library| library.get("metadata"))
            .ok_or("the template must name the library metadata section")?;
        assert!(
            metadata.get("music").is_some(),
            "the template must name the music section"
        );
        assert!(
            metadata.get("video").is_some(),
            "the template must name the video section"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_partial_model_represents_the_default_document()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let document = yaml_serde::to_value(Configuration::default())?;

        // Act
        let partial = yaml_serde::from_value::<PartialConfiguration>(document);

        // Assert
        assert!(
            partial.is_ok(),
            "every setting the default document names must have a place in the partial model, got: {partial:?}"
        );
        Ok(())
    }
}
