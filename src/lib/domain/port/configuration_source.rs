use std::future::Future;

use crate::domain::model::partial_configuration::PartialConfiguration;
use crate::domain::port::error::ConfigurationSourceError;

/// Response of a successful file-layer load.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LoadedFileConfiguration {
    /// Whether this load generated the default configuration file.
    file_created: bool,
    /// The settings the file names.
    partial: PartialConfiguration,
}

impl LoadedFileConfiguration {
    /// Return whether this load generated the default configuration file.
    #[must_use]
    pub fn file_created(&self) -> bool {
        self.file_created
    }

    /// Split the result into its partial configuration and its file-created
    /// flag.
    #[must_use]
    pub fn into_parts(self) -> (PartialConfiguration, bool) {
        (self.partial, self.file_created)
    }

    /// Create a new load result.
    #[must_use]
    pub fn new(partial: PartialConfiguration, file_created: bool) -> Self {
        Self {
            file_created,
            partial,
        }
    }

    /// Return the settings the file names.
    #[must_use]
    pub fn partial(&self) -> &PartialConfiguration {
        &self.partial
    }
}

/// Port for loading the configuration file layer.
///
/// The source reads `config.yaml`, writes the built-in default document when
/// none exists, and returns the settings the file *names* — it does not merge
/// them. The load configuration use case merges this layer over the built-in
/// defaults, so the precedence lives in one place.
#[cfg_attr(test, mockall::automock)]
pub trait ConfigurationSource: Send + Sync {
    /// Load the configuration file layer.
    ///
    /// # Errors
    ///
    /// Returns an error when the default file cannot be written, when the
    /// configuration file cannot be read or parsed, or when it names an
    /// unknown key.
    fn load(
        &self,
    ) -> impl Future<Output = Result<LoadedFileConfiguration, ConfigurationSourceError>> + Send;
}

/// Port for loading the environment configuration layer.
///
/// The source maps the `MNEMORIUM_*` variables onto the settings they name and
/// returns them as the same partial document the file layer produces; the load
/// configuration use case merges it over the file result. A prefixed name that
/// matches no setting is ignored, never rejected.
#[cfg_attr(test, mockall::automock)]
pub trait EnvironmentConfiguration: Send + Sync {
    /// Load the environment configuration layer.
    ///
    /// # Errors
    ///
    /// Returns an error when a prefixed variable does not hold a value its
    /// setting accepts.
    fn load(
        &self,
    ) -> impl Future<Output = Result<PartialConfiguration, ConfigurationSourceError>> + Send;
}
