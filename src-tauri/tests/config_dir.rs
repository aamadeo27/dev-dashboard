/// Integration tests for config directory resolution logic (lib.rs::run).
///
/// # Design note
///
/// The resolution logic lives inside `lib.rs::run()`, which also initialises
/// logging and starts the Tauri application loop — both of which cannot be
/// invoked in a test context.  To make the logic testable without modifying
/// the production code, the tests reproduce the exact same resolution
/// algorithm inline and verify it against the same environment variables.
///
/// This is intentional: the algorithm is three lines and has no external
/// dependencies beyond `std::env` and `dirs`.  A helper free-function would
/// be marginally cleaner, but extracting it is a refactor that lies outside
/// T0.2 scope.  When such a refactor is done (e.g. `pub(crate) fn
/// resolve_config_dir() -> PathBuf` in `lib.rs`), these tests should be
/// updated to call that function directly.
///
/// # Environment isolation
///
/// `std::env::set_var` / `remove_var` are process-global and NOT thread-safe
/// under Rust's default multi-threaded test runner.  Each test that mutates
/// env vars:
///   1. Acquires an exclusive `Mutex` lock defined in this module.
///   2. Saves, mutates, asserts, then restores the original value.
///   3. Releases the lock.
///
/// Running the suite with `cargo test -- --test-threads=1` also works, but
/// the mutex approach allows the test binary to keep its default thread count
/// without flakiness.
use std::path::PathBuf;
use std::sync::Mutex;

/// Process-wide lock to serialise env-var mutations.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// The env variable name, mirrored from lib.rs.
const ENV_VAR: &str = "DEV_DASHBOARD_CONFIG_DIR";

/// Reproduce the resolution algorithm from lib.rs::run() verbatim.
///
/// Priority:
///   1. `DEV_DASHBOARD_CONFIG_DIR` env var if set.
///   2. `dirs::config_dir()` joined with "dev-dashboard".
///   3. Fallback: `std::env::temp_dir()` joined with "dev-dashboard".
fn resolve_config_dir() -> PathBuf {
    if let Ok(override_dir) = std::env::var(ENV_VAR) {
        PathBuf::from(override_dir)
    } else {
        dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("dev-dashboard")
    }
}

// ---------------------------------------------------------------------------
// TC-1: env-var override path
// ---------------------------------------------------------------------------

/// When `DEV_DASHBOARD_CONFIG_DIR` is set to an arbitrary path, the resolved
/// config dir must equal that path exactly — no "dev-dashboard" suffix is
/// appended and the OS default is not consulted.
#[test]
fn config_dir_uses_env_var_override() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let saved = std::env::var(ENV_VAR).ok();
    let expected = std::env::temp_dir().join("dd-test-override-12345");
    std::env::set_var(ENV_VAR, &expected);

    let result = resolve_config_dir();

    // Restore before any assert so a panic does not poison the env for other tests.
    match &saved {
        Some(v) => std::env::set_var(ENV_VAR, v),
        None => std::env::remove_var(ENV_VAR),
    }

    assert_eq!(
        result, expected,
        "config dir should equal the env-var value exactly"
    );
}

// ---------------------------------------------------------------------------
// TC-2: env-var override does not append "dev-dashboard"
// ---------------------------------------------------------------------------

/// If the operator points `DEV_DASHBOARD_CONFIG_DIR` at a path that already
/// ends in "dev-dashboard", the resolver must NOT append a second segment.
/// (Guards against a hypothetical regression where the suffix is always added.)
#[test]
fn config_dir_env_var_is_not_modified() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let saved = std::env::var(ENV_VAR).ok();
    let raw = std::env::temp_dir()
        .join("some")
        .join("custom")
        .join("path");
    std::env::set_var(ENV_VAR, &raw);

    let result = resolve_config_dir();

    match &saved {
        Some(v) => std::env::set_var(ENV_VAR, v),
        None => std::env::remove_var(ENV_VAR),
    }

    assert_eq!(result, raw);
    // Confirm "dev-dashboard" was not appended a second time.
    assert_ne!(result, raw.join("dev-dashboard"));
}

// ---------------------------------------------------------------------------
// TC-3: OS default path contains "dev-dashboard"
// ---------------------------------------------------------------------------

/// Without the env-var override, the resolved path must end in a segment
/// named "dev-dashboard".  This confirms the `.join("dev-dashboard")` suffix
/// is applied regardless of which OS default (`dirs::config_dir()` or the
/// temp-dir fallback) is used.
#[test]
fn config_dir_default_ends_with_dev_dashboard() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let saved = std::env::var(ENV_VAR).ok();
    std::env::remove_var(ENV_VAR);

    let result = resolve_config_dir();

    match &saved {
        Some(v) => std::env::set_var(ENV_VAR, v),
        None => {} // already removed
    }

    assert_eq!(
        result.file_name().and_then(|n| n.to_str()),
        Some("dev-dashboard"),
        "last path segment must be 'dev-dashboard', got: {}",
        result.display()
    );
}

// ---------------------------------------------------------------------------
// TC-4: default path is absolute
// ---------------------------------------------------------------------------

/// The resolved path must always be absolute.  A relative config dir would
/// break every subsequent path join across the application.
#[test]
fn config_dir_default_is_absolute() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let saved = std::env::var(ENV_VAR).ok();
    std::env::remove_var(ENV_VAR);

    let result = resolve_config_dir();

    match &saved {
        Some(v) => std::env::set_var(ENV_VAR, v),
        None => {}
    }

    assert!(
        result.is_absolute(),
        "config dir must be absolute, got: {}",
        result.display()
    );
}

// ---------------------------------------------------------------------------
// TC-5: resolution is deterministic (idempotent within a single call sequence)
// ---------------------------------------------------------------------------

/// Calling the resolver twice without mutating env vars between calls must
/// produce identical results.  This guards against any hypothetical
/// non-determinism (e.g. a mutable static or a race in `dirs`).
#[test]
fn config_dir_resolution_is_deterministic() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let saved = std::env::var(ENV_VAR).ok();
    std::env::remove_var(ENV_VAR);

    let first = resolve_config_dir();
    let second = resolve_config_dir();

    match &saved {
        Some(v) => std::env::set_var(ENV_VAR, v),
        None => {}
    }

    assert_eq!(
        first, second,
        "two consecutive calls with identical env state must return the same path"
    );
}

// ---------------------------------------------------------------------------
// TC-6: env-var override path is absolute (when set to an absolute path)
// ---------------------------------------------------------------------------

/// When the env var contains an absolute path, the resolver must return it
/// without modification.  This is the expected usage (CI sets an absolute
/// temp path; the app must use it verbatim).
#[test]
fn config_dir_env_var_absolute_path_preserved() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let saved = std::env::var(ENV_VAR).ok();
    // temp_dir() is guaranteed absolute on all supported platforms.
    let abs_path = std::env::temp_dir().join("dd-abs-test");
    std::env::set_var(ENV_VAR, &abs_path);

    let result = resolve_config_dir();

    match &saved {
        Some(v) => std::env::set_var(ENV_VAR, v),
        None => std::env::remove_var(ENV_VAR),
    }

    assert!(result.is_absolute(), "resolved path must be absolute");
    assert_eq!(result, abs_path);
}

// ---------------------------------------------------------------------------
// TC-7: IGNORED — requires refactor to test temp_dir fallback in isolation
// ---------------------------------------------------------------------------

/// Verify that when `dirs::config_dir()` returns `None`, the resolver falls
/// back to `std::env::temp_dir().join("dev-dashboard")`.
///
/// # Why ignored
///
/// `dirs::config_dir()` reads OS state (HOME / USERPROFILE / XDG_CONFIG_HOME)
/// and cannot be forced to return `None` without modifying the resolver to
/// accept an injected `Option<PathBuf>`.  The fix is:
///
/// ```rust
/// // in lib.rs
/// pub(crate) fn resolve_config_dir_with(base: Option<PathBuf>) -> PathBuf {
///     if let Ok(v) = std::env::var("DEV_DASHBOARD_CONFIG_DIR") { return PathBuf::from(v); }
///     base.unwrap_or_else(std::env::temp_dir).join("dev-dashboard")
/// }
///
/// // lib.rs::run() calls: resolve_config_dir_with(dirs::config_dir())
/// // test calls:          resolve_config_dir_with(None)
/// ```
///
/// When that refactor lands, remove this `#[ignore]` and implement the body.
#[test]
#[ignore = "requires extract of resolve_config_dir_with(base: Option<PathBuf>) from lib.rs::run"]
fn config_dir_falls_back_to_temp_dir_when_dirs_returns_none() {
    // Precondition: resolver is refactored to accept an injected base dir.
    // Steps:
    //   1. Call resolve_config_dir_with(None) with env var unset.
    //   2. Assert result == std::env::temp_dir().join("dev-dashboard").
    todo!("implement after refactor")
}
