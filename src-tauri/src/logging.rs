// Called from lib.rs::run() via init_logging(). Reads DEV_DASHBOARD_LOG env var.
// Writes JSON-formatted logs to <config_dir>/dev-dashboard/logs/app-YYYY-MM-DD.log
// via tracing-appender daily rotation. See KB §7 and docs/monitoring.md §2 for full schema.

use std::path::Path;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize the global tracing subscriber.
///
/// The returned `WorkerGuard` must be retained by the caller (e.g. stored in
/// `AppState`) for the lifetime of the application; dropping it will flush and
/// stop the non-blocking writer, after which log lines will be silently lost.
pub fn init_logging(log_dir: &Path) -> tracing_appender::non_blocking::WorkerGuard {
    std::fs::create_dir_all(log_dir).ok();

    let file_appender = rolling::daily(log_dir, "app");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_env("DEV_DASHBOARD_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(non_blocking)
        .with_filter(filter);

    let stderr_layer = fmt::layer()
        .with_target(true)
        .with_filter(EnvFilter::new("warn"));

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .init();

    tracing::info!(component = "app", "logging initialized");

    guard
}
