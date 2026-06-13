/// Integration tests for `ipc::commands::log_frontend_error`.
///
/// # Design note
///
/// `log_frontend_error` is a fire-and-forget tracing call. It cannot return an
/// error, and capturing the tracing output in a unit test would require either
/// a custom `Subscriber` or a third-party crate — neither of which is in scope
/// for T0.6. The tests here focus on two guarantees:
///
///   1. The function compiles with the expected async signature.
///   2. Calling it with various argument combinations does NOT panic.
///
/// Correctness of the log output (JSON field names, log level) is verified
/// by reading the emitted log file during manual / integration test runs.
use dev_dashboard_lib::ipc::commands::log_frontend_error;

// ---------------------------------------------------------------------------
// TC-1: full arguments — message + stack + route
// ---------------------------------------------------------------------------

/// Calling `log_frontend_error` with all three arguments must complete without
/// panicking.
#[tokio::test]
async fn log_frontend_error_full_args_does_not_panic() {
    log_frontend_error(
        "TypeError: Cannot read properties of undefined".to_string(),
        Some("at Component (App.tsx:42)".to_string()),
        Some("/dashboard".to_string()),
    )
    .await;
}

// ---------------------------------------------------------------------------
// TC-2: message only — stack=None, route=None
// ---------------------------------------------------------------------------

/// Calling `log_frontend_error` with only a message (optional args as `None`)
/// must complete without panicking.
#[tokio::test]
async fn log_frontend_error_message_only_does_not_panic() {
    log_frontend_error("Unhandled promise rejection".to_string(), None, None).await;
}

// ---------------------------------------------------------------------------
// TC-3: empty message string
// ---------------------------------------------------------------------------

/// An empty message string is valid input and must not panic.
#[tokio::test]
async fn log_frontend_error_empty_message_does_not_panic() {
    log_frontend_error(String::new(), None, None).await;
}
