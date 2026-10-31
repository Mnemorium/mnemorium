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

use crate::domain::model::partial_configuration::PartialConfiguration;
use crate::domain::port::configuration_source::ConfigurationSource;
use crate::domain::port::configuration_source::LoadedFileConfiguration;
use crate::domain::port::error::ConfigurationSourceError;

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
pub struct FileConfigurationSource;

impl FileConfigurationSource {
    /// Create a new source.
    #[must_use]
    pub fn new() -> Self {
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
    use std::error::Error;
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;
    use yaml_serde::Value;

    use crate::domain::model::configuration::Configuration;
    use crate::domain::model::configuration::Rotation;
    use crate::domain::model::partial_configuration::PartialAsset;
    use crate::domain::model::partial_configuration::PartialConfiguration;
    use crate::domain::port::error::ConfigurationSourceError;

    use super::DEFAULT_CONFIG_TEMPLATE;
    use super::Resolver;

    /// Build a resolution over `path`.
    fn resolver(path: &Path) -> Resolver<'_> {
        Resolver { path }
    }

    #[test]
    fn configuration_source_absent_file_writes_the_defaults() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let resolver = resolver(&path);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(
            loaded.file_created(),
            "an absent configuration file must be generated"
        );
        assert_eq!(
            loaded.partial().asset.upload.chunk_size_bytes,
            Some(5_242_880),
            "the generated file names every setting of the default document"
        );
        assert_eq!(
            fs::read_to_string(&path)?,
            DEFAULT_CONFIG_TEMPLATE,
            "the generated file must carry the embedded defaults"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_empty_file_names_no_setting() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "")?;
        let resolver = resolver(&path);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(!loaded.file_created(), "an existing file is not rewritten");
        assert_eq!(loaded.partial(), &PartialConfiguration::default());
        Ok(())
    }

    #[test]
    fn configuration_source_comment_only_file_names_no_setting() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        fs::write(&path, "# nothing configured yet\n")?;
        let resolver = resolver(&path);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(!loaded.file_created(), "an existing file is not rewritten");
        assert_eq!(
            loaded.partial(),
            &PartialConfiguration::default(),
            "a document that sets nothing must name no setting"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_existing_file_is_not_rewritten() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let original = "logging:\n  level: info\n";
        fs::write(&path, original)?;
        let resolver = resolver(&path);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(!loaded.file_created(), "an existing file is not rewritten");
        assert_eq!(fs::read_to_string(&path)?, original);
        assert_eq!(
            loaded.partial().logging.level.as_deref(),
            Some("info"),
            "the file value must be named by this layer"
        );
        assert_eq!(
            loaded.partial().asset,
            PartialAsset::default(),
            "an omitted section names no setting"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_partial_section_names_only_its_keys() -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let original = "logging:\n  max_files: 3\n  rotation: HOURLY\n";
        fs::write(&path, original)?;
        let resolver = resolver(&path);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        assert!(!loaded.file_created(), "an existing file is not rewritten");
        assert_eq!(
            fs::read_to_string(&path)?,
            original,
            "the file must stay untouched"
        );
        let logging = &loaded.partial().logging;
        assert_eq!(logging.max_files, Some(3), "a present value is named");
        assert_eq!(logging.rotation, Some(Rotation::Hourly));
        assert_eq!(logging.level, None, "an omitted key stays unnamed");
        assert_eq!(logging.ansi, None, "a second omitted key stays unnamed");
        Ok(())
    }

    #[test]
    fn configuration_source_partial_nested_mapping_names_only_its_keys()
    -> Result<(), Box<dyn Error>> {
        // Arrange
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let original = "security:\n  jwt:\n    ttl: 60\n";
        fs::write(&path, original)?;
        let resolver = resolver(&path);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        let security = &loaded.partial().security;
        assert_eq!(security.jwt.ttl, Some(60));
        assert_eq!(
            security.rate_limit.burst_size, None,
            "an omitted sibling mapping stays unnamed"
        );
        Ok(())
    }

    #[test]
    fn configuration_source_library_provider_list_is_parsed() -> Result<(), Box<dyn Error>> {
        // Arrange: the file names one provider entry with its key.
        let directory = tempdir()?;
        let path = directory.path().join("config.yaml");
        let original = "library:\n  metadata:\n    video:\n      providers:\n        - name: tmdb\n          index: 1\n          api_key: sentinel-tmdb\n";
        fs::write(&path, original)?;
        let resolver = resolver(&path);

        // Act
        let loaded = resolver.resolve()?;

        // Assert
        let providers = loaded
            .partial()
            .library
            .metadata
            .video
            .providers
            .as_ref()
            .ok_or("the provider list must be named by the file")?;
        assert_eq!(providers.len(), 1);
        assert_eq!(
            providers
                .first()
                .and_then(|provider| provider.name.as_deref()),
            Some("tmdb")
        );
        assert_eq!(
            providers
                .first()
                .and_then(|provider| provider.api_key.as_deref()),
            Some("sentinel-tmdb")
        );
        Ok(())
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
