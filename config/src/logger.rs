#![forbid(unsafe_code)]

use crate::{mode::AppMode, settings::LoggingSettings};
use std::path::{Path, PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

pub fn init_central_logging(
    mode: AppMode,
    log_dir: impl AsRef<Path>,
    app_name: &str,
) -> Option<WorkerGuard> {
    let settings = LoggingSettings {
        log_dir: PathBuf::from(log_dir.as_ref()),
        ..LoggingSettings::default()
    };
    init_central_logging_with_settings(mode, &settings, app_name)
}

pub fn init_central_logging_with_settings(
    mode: AppMode,
    settings: &LoggingSettings,
    app_name: &str,
) -> Option<WorkerGuard> {
    let filter = match mode {
        AppMode::Developer => EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env_lossy(),
        AppMode::Production => EnvFilter::new("error"),
    };

    let stdout_layer = settings.log_to_stdout.then(|| {
        fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(mode == AppMode::Developer)
            .with_target(true)
            .with_line_number(mode == AppMode::Developer)
            .pretty()
            .boxed()
    });

    let (file_layer, guard) = if settings.log_to_file {
        let suffix = match mode {
            AppMode::Developer => "dev",
            AppMode::Production => "prod",
        };
        let file_appender =
            tracing_appender::rolling::never(&settings.log_dir, format!("{app_name}-{suffix}.log"));
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true)
            .boxed();
        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    tracing::info!(app = app_name, mode = ?mode, "Pencatatan log terpusat aktif");
    guard
}
