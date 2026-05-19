use tracing_appender::non_blocking::WorkerGuard;

pub struct AppState {
    /// Keeps the non-blocking log writer alive for the application lifetime.
    pub log_guard: WorkerGuard,
}
