use std::io::stdout;

use anyhow::Context as _;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::RollingFileAppender;
use tracing_appender::rolling::Rotation as FileRotation;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

use crate::domain::model::configuration::Logging;
use crate::domain::model::configuration::Rotation;

/// Directory holding the log files, relative to the process working directory.
const LOG_DIRECTORY: &str = "logs";
/// Prefix of the log file names.
const LOG_FILE_NAME: &str = "mnemorium.log";
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
/// whole-level value and any directive that targets `security` are rejected
/// when the [`Logging`] value is built, so they never reach this function.
///
/// # Errors
///
/// Returns an error when the verbosity directives are invalid or when the file
/// appender cannot be created (`OBS-007`).
pub fn setup(logging: &Logging) -> anyhow::Result<WorkerGuard> {
    let with_floor = format!("{},{SECURITY_FLOOR}", logging.level().trim());
    let filter = EnvFilter::try_new(with_floor).context("invalid logging level directives")?;

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

    // The logging initialization is a lifecycle event (`OBS-006`): the catalog
    // declares it at `info` with fields `component` and `outcome`.
    info!(
        target: "security",
        event = "lifecycle",
        component = "logging",
        outcome = "initialized",
        "logging initialized"
    );

    Ok(guard)
}
