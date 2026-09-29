#![forbid(unsafe_code)]

use crate::mode::AppMode;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

pub fn init_central_logging(
    mode: AppMode,
    log_dir: impl AsRef<Path>,
    app_name: &str,
) -> Option<WorkerGuard> {
    let log_dir_ref = log_dir.as_ref();

    let guard = match mode {
        AppMode::Developer => {
            let filter = EnvFilter::builder()
                .with_default_directive(LevelFilter::DEBUG.into())
                .from_env_lossy();

            let stdout_layer = fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true)
                .with_target(true)
                .with_line_number(true)
                .pretty()
                .boxed();

            let file_appender =
                tracing_appender::rolling::never(log_dir_ref, format!("{app_name}-dev.log"));
            let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);

            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .boxed();

            tracing_subscriber::registry()
                .with(filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();

            tracing::info!(
                app = app_name,
                mode = "developer",
                "Pencatatan log aktif dalam mode DEVELOPER"
            );

            Some(file_guard)
        }
        AppMode::Production => {
            // Mode Production: Hening di terminal, hanya mencatat ERROR fatal ke file
            let file_appender =
                tracing_appender::rolling::never(log_dir_ref, format!("{app_name}-prod.log"));
            let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);

            let filter = LevelFilter::ERROR;
            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .boxed();

            tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .init();

            Some(file_guard)
        }
    };

    guard
}
