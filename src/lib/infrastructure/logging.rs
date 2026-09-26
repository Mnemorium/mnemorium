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

/// Configure and install the global `tracing` subscriber from `logging`.
///
/// The file sink always writes plain text; `logging.ansi()` only colours the
/// standard-output sink. The returned [`WorkerGuard`] must be held for the
/// lifetime of the process: dropping it flushes and stops the non-blocking file
/// writer.
///
/// # Errors
///
/// Returns an error when the verbosity directives are invalid or the file
/// appender cannot be created.
pub fn setup(logging: &Logging) -> anyhow::Result<WorkerGuard> {
    let filter = if logging.level().is_empty() {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_LEVEL))
    } else {
        EnvFilter::try_new(logging.level()).context("invalid logging level directives")?
    };

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
