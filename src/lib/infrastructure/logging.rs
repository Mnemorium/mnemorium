use std::io::stdout;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt as _, util::SubscriberInitExt as _};

pub fn setup() {
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "mnemorium.log");

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer().with_writer(non_blocking).with_ansi(false);

    let stdout_layer = fmt::layer().with_writer(stdout).with_ansi(false);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();

    tracing::info!("Logging initialized");
}
