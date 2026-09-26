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
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::Logging;
    use super::LoggingError;
    use super::Rotation;

    #[test]
    fn try_new_rejects_empty_level() {
        // Act
        let result = Logging::try_new(false, String::new(), 7, Rotation::Daily);

        // Assert
        assert!(matches!(result, Err(LoggingError::LevelEmpty)));
    }

    #[test]
    fn try_new_exposes_validated_settings() -> Result<(), Box<dyn Error>> {
        // Act
        let logging = Logging::try_new(true, "info,sqlx=trace".to_owned(), 3, Rotation::Hourly)?;

        // Assert
        assert!(logging.ansi());
        assert_eq!(logging.level(), "info,sqlx=trace");
        assert_eq!(logging.max_files(), 3);
        assert_eq!(logging.rotation(), Rotation::Hourly);
        Ok(())
    }

    #[test]
    fn rotation_deserializes_its_persisted_representation() -> Result<(), Box<dyn Error>> {
        // Act & Assert
        assert_eq!(
            serde_json::from_str::<Rotation>("\"DAILY\"")?,
            Rotation::Daily
        );
        assert_eq!(
            serde_json::from_str::<Rotation>("\"NEVER\"")?,
            Rotation::Never
        );
        Ok(())
    }
}
