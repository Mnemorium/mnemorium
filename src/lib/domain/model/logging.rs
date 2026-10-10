/// Reserved target carrying security events; a verbosity directive must never
/// target it, or it could suppress a catalog event (`OBS-007`).
const SECURITY_TARGET: &str = "security";

/// Error returned when initialising or updating a `Logging` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoggingError {
    /// The verbosity filter disables logging as a whole (`off`/`none`).
    #[error("logging level must not disable logging")]
    LevelDisablesLogging,
    /// The verbosity filter directives are empty.
    #[error("logging level must not be empty")]
    LevelEmpty,
    /// The verbosity filter targets the reserved `security` target.
    #[error("logging level must not target the reserved security target")]
    LevelTargetsSecurity,
}

/// Rotation period of the log files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
#[non_exhaustive]
pub enum Rotation {
    /// Rotate the log file once a day.
    Daily,
    /// Rotate the log file once an hour.
    Hourly,
    /// Rotate the log file once a minute.
    Minutely,
    /// Never rotate the log file.
    Never,
}

/// Logging settings.
///
/// Deserialization is routed through `LoggingConfig` and [`TryFrom`] so that the
/// layered configuration cannot bypass the validation performed by
/// [`Logging::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(try_from = "LoggingConfig")]
#[non_exhaustive]
pub struct Logging {
    /// Whether to colour the standard-output sink with ANSI escape codes.
    ansi: bool,
    /// Verbosity filter applied to the `tracing` instrumentation.
    level: String,
    /// Maximum number of rotated log files to keep; `0` keeps every file.
    max_files: u32,
    /// Rotation period of the file sink.
    rotation: Rotation,
}

/// Unchecked deserialization mirror of [`Logging`].
///
/// Serde builds this snapshot and hands it to the [`TryFrom`] implementation,
/// which validates it through [`Logging::try_new`].
#[derive(serde::Deserialize)]
struct LoggingConfig {
    /// Whether to colour the standard-output sink with ANSI escape codes.
    ansi: bool,
    /// Verbosity filter applied to the `tracing` instrumentation.
    level: String,
    /// Maximum number of rotated log files to keep; `0` keeps every file.
    max_files: u32,
    /// Rotation period of the file sink.
    rotation: Rotation,
}

impl TryFrom<LoggingConfig> for Logging {
    type Error = LoggingError;

    fn try_from(config: LoggingConfig) -> Result<Self, Self::Error> {
        Self::try_new(config.ansi, config.level, config.max_files, config.rotation)
    }
}

impl Logging {
    /// Return whether to colour the standard-output sink with ANSI escape
    /// codes.
    #[must_use]
    pub fn ansi(&self) -> bool {
        self.ansi
    }

    /// Return the verbosity filter applied to the `tracing` instrumentation.
    #[must_use]
    pub fn level(&self) -> &str {
        &self.level
    }

    /// Return the maximum number of rotated log files to keep; `0` keeps every
    /// file.
    #[must_use]
    pub fn max_files(&self) -> u32 {
        self.max_files
    }

    /// Return the rotation period of the file sink.
    #[must_use]
    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    /// Initialise a new `Logging`, validating `level`.
    ///
    /// The level is rejected when it is empty, when it disables logging as a
    /// whole (`off`/`none`), or when it carries a directive for the reserved
    /// `security` target; either would let `logging.level` suppress a catalog
    /// event (`OBS-007`).
    ///
    /// # Errors
    ///
    /// Returns [`LoggingError::LevelEmpty`] when `level` is empty,
    /// [`LoggingError::LevelDisablesLogging`] when `level` is a bare `off` or
    /// `none`, and [`LoggingError::LevelTargetsSecurity`] when a directive
    /// targets the reserved `security` target.
    pub fn try_new(
        ansi: bool,
        level: String,
        max_files: u32,
        rotation: Rotation,
    ) -> Result<Self, LoggingError> {
        if level.is_empty() {
            return Err(LoggingError::LevelEmpty);
        }
        let directives = level.trim();
        if matches!(directives.to_ascii_lowercase().as_str(), "off" | "none") {
            return Err(LoggingError::LevelDisablesLogging);
        }
        if directives
            .split(',')
            .any(|directive| directive.trim().starts_with(SECURITY_TARGET))
        {
            return Err(LoggingError::LevelTargetsSecurity);
        }
        Ok(Self {
            ansi,
            level,
            max_files,
            rotation,
        })
    }
}
