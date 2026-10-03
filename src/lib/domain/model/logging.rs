/// Error returned when initialising or updating a `Logging` value object.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoggingError {
    /// The verbosity filter directives are empty.
    #[error("logging level must not be empty")]
    LevelEmpty,
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
    /// # Errors
    ///
    /// Returns [`LoggingError::LevelEmpty`] when `level` is empty.
    pub fn try_new(
        ansi: bool,
        level: String,
        max_files: u32,
        rotation: Rotation,
    ) -> Result<Self, LoggingError> {
        if level.is_empty() {
            return Err(LoggingError::LevelEmpty);
        }
        Ok(Self {
            ansi,
            level,
            max_files,
            rotation,
        })
    }
}
