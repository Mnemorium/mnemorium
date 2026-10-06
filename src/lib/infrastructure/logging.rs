use std::env;
use std::io::stdout;

use anyhow::Context as _;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::RollingFileAppender;
use tracing_appender::rolling::Rotation as FileRotation;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

use crate::application::use_case::load_configuration::DEFAULT_LOG_LEVEL;
use crate::domain::model::logging::Logging;
use crate::domain::model::logging::Rotation;

/// Directory holding the log files, relative to the process working directory.
const LOG_DIRECTORY: &str = "logs";
/// Prefix of the log file names.
const LOG_FILE_NAME: &str = "mnemorium.log";
/// Reserved target carrying security events (`OBS-007`).
const SECURITY_TARGET: &str = "security";
/// Directive pinning the security target to its non-suppressible floor: the
/// catalog's lowest declared level, so no `logging.level` value can suppress a
/// catalog event (`OBS-006`, `OBS-007`).
const SECURITY_FLOOR: &str = "security=debug";

/// Configure and install the global `tracing` subscriber from `logging`.
///
/// The file sink always writes plain text; `logging.ansi()` only colours the
/// standard-output sink. The returned [`WorkerGuard`] must be held for the
/// lifetime of the process: dropping it flushes and stops the non-blocking file
/// writer.
///
/// The filter is built from the `logging.level` directives with the security
/// floor appended last (`OBS-007`): the reserved `security` target keeps the
/// catalog's lowest declared level (`debug`) no matter what `logging.level`
/// says, so a security event can never be suppressed. `off`/`none` as a
/// whole-level value and any directive that targets `security` are rejected,
/// because both would lift the floor.
///
/// # Errors
///
/// Returns an error when the verbosity directives are invalid, when they
/// disable logging as a whole (`off`/`none`), when they target the reserved
/// `security` target, or when the file appender cannot be created (`OBS-007`).
pub fn setup(logging: &Logging) -> anyhow::Result<WorkerGuard> {
    let filter = build_filter(logging)?;

    let rotation = match logging.rotation() {
        Rotation::Minutely => FileRotation::MINUTELY,
        Rotation::Hourly => FileRotation::HOURLY,
        Rotation::Daily => FileRotation::DAILY,
        Rotation::Never => FileRotation::NEVER,
    };

    let appender = RollingFileAppender::builder()
        .rotation(rotation)
        .filename_prefix(LOG_FILE_NAME)
        .max_log_files(usize::try_from(logging.max_files()).unwrap_or(usize::MAX))
        .build(LOG_DIRECTORY)
        .context("failed to create the log file appender")?;

    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let file_layer = fmt::layer().with_writer(non_blocking).with_ansi(false);
    let stdout_layer = fmt::layer().with_writer(stdout).with_ansi(logging.ansi());

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    tracing::info!("Logging initialized");

    Ok(guard)
}

/// Build the `EnvFilter` for `logging`, appending the non-suppressible security
/// floor (`OBS-007`).
///
/// # Errors
///
/// Returns an error when the directives are invalid, when `logging.level` is a
/// bare `off`/`none`, or when it carries a directive for the reserved `security`
/// target.
#[cfg_attr(
    not(test),
    expect(
        clippy::single_call_fn,
        reason = "the filter construction is named and unit-testable on its own"
    )
)]
fn build_filter(logging: &Logging) -> anyhow::Result<EnvFilter> {
    let level = if logging.level().is_empty() {
        env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_owned())
    } else {
        logging.level().to_owned()
    };
    let directives = level.trim();
    if matches!(directives.to_ascii_lowercase().as_str(), "off" | "none") {
        anyhow::bail!("logging level cannot disable application logging");
    }
    if directives
        .split(',')
        .any(|directive| directive.trim().starts_with(SECURITY_TARGET))
    {
        anyhow::bail!("logging level cannot target the reserved security target");
    }
    let with_floor = format!("{directives},{SECURITY_FLOOR}");
    EnvFilter::try_new(with_floor).context("invalid logging level directives")
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::domain::model::logging::Logging;
    use crate::domain::model::logging::Rotation;

    use super::SECURITY_FLOOR;
    use super::build_filter;

    /// Build a `Logging` value around `level`.
    fn logging(level: &str) -> Result<Logging, Box<dyn Error>> {
        Ok(Logging::try_new(
            false,
            level.to_owned(),
            7,
            Rotation::Daily,
        )?)
    }

    #[test]
    fn filter_appends_the_security_floor() -> Result<(), Box<dyn Error>> {
        // Act
        let filter = build_filter(&logging("error")?)?;

        // Assert
        assert!(
            filter.to_string().contains(SECURITY_FLOOR),
            "the security floor must be appended to the directives"
        );
        Ok(())
    }

    #[test]
    fn bare_off_is_rejected() -> Result<(), Box<dyn Error>> {
        // Act
        let result = build_filter(&logging("off")?);

        // Assert
        assert!(result.is_err(), "`off` must not disable logging entirely");
        Ok(())
    }

    #[test]
    fn bare_none_is_rejected() -> Result<(), Box<dyn Error>> {
        // Act
        let result = build_filter(&logging("none")?);

        // Assert
        assert!(result.is_err(), "`none` must not disable logging entirely");
        Ok(())
    }

    #[test]
    fn targeting_the_security_target_is_rejected() -> Result<(), Box<dyn Error>> {
        // Act
        let result = build_filter(&logging("security=error")?);

        // Assert
        assert!(
            result.is_err(),
            "a directive targeting the reserved security target must be rejected"
        );
        Ok(())
    }
}
