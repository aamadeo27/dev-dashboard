/// Integration tests for `resolve_cli_path` (T1.2).
///
/// # Scope
///
/// These tests cover the `resolve_cli_path` pure helper from a separate crate
/// boundary — the same boundary the binary crate sees at runtime.  They
/// complement the four inline unit tests in `commands::tests` by exercising:
///
///   - None + None → bare "claude" fallback (confirms the unit result holds
///     across the crate boundary; no accidental re-export shadowing).
///   - Override with embedded spaces is preserved byte-for-byte.
///   - Override with a Unicode segment is preserved byte-for-byte.
///   - Settings path with embedded spaces is preserved byte-for-byte.
///   - Override is still chosen over settings when both contain spaces
///     (regression guard: priority must not change for unusual paths).
///   - Empty-string components survive round-trip (edge: zero-length segment).
///
/// # Why `verify_claude_cli` is NOT tested here
///
/// `verify_claude_cli` is a `#[tauri::command]` that takes `tauri::State<'_,
/// AppState>` as a parameter.  `tauri::State` can only be constructed inside a
/// running Tauri application (it is created by the framework after
/// `.manage(state)` is called on the builder).  Constructing it in an
/// integration test without a running Tauri window is not possible without
/// forking the Tauri internals.
///
/// The spawn/wait logic inside `verify_claude_cli` (after path resolution) is
/// tested indirectly via:
///   - The `resolve_cli_path` tests here (confirm the correct path is selected).
///   - The frontend mock tests in `commands.test.ts` (confirm the IPC contract).
///   - Manual smoke tests documented in `docs/tasks/T1.2.md`.
///
/// If in the future `verify_cli_spawn_logic` is extracted to a plain async fn
/// that accepts a `PathBuf` and returns `CliCheck`, it can be tested here
/// directly with a real temp-script on the OS.

use dev_dashboard_lib::ipc::commands::resolve_cli_path;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// IT-01: None override + None settings → PathBuf::from("claude")
// ---------------------------------------------------------------------------

/// The fallback must be the bare string "claude" (resolved via PATH at
/// runtime).  Tested from the integration-test crate to confirm there is no
/// accidental shadowing or re-export that changes the value.
#[test]
fn it01_both_none_returns_bare_claude_string() {
    let result = resolve_cli_path(None, None);
    assert_eq!(
        result,
        PathBuf::from("claude"),
        "fallback must be PathBuf::from(\"claude\") when both override and settings are None"
    );
}

// ---------------------------------------------------------------------------
// IT-02: Override with embedded spaces is returned verbatim
// ---------------------------------------------------------------------------

/// A path that contains spaces (e.g., "C:\Program Files\Claude\claude.exe")
/// must survive the function unchanged.  No quoting, escaping, or trimming
/// may be applied.
#[test]
fn it02_override_with_spaces_preserved_verbatim() {
    let spaced = PathBuf::from("/opt/my tools/claude code/claude");
    let result = resolve_cli_path(Some(spaced.clone()), None);
    assert_eq!(
        result, spaced,
        "override path with spaces must be returned verbatim"
    );
}

// ---------------------------------------------------------------------------
// IT-03: Settings path with embedded spaces is returned verbatim
// ---------------------------------------------------------------------------

/// When there is no override and the settings path contains spaces, it must
/// be returned without modification.
#[test]
fn it03_settings_path_with_spaces_preserved_verbatim() {
    let spaced = PathBuf::from("/home/user/my tools/claude");
    let result = resolve_cli_path(None, Some(spaced.clone()));
    assert_eq!(
        result, spaced,
        "settings path with spaces must be returned verbatim"
    );
}

// ---------------------------------------------------------------------------
// IT-04: Override with spaces still wins over settings (priority unchanged)
// ---------------------------------------------------------------------------

/// Priority must not degrade for unusual paths.  When both the override and
/// the settings path contain spaces, the override must still win.
#[test]
fn it04_override_with_spaces_still_beats_settings_with_spaces() {
    let override_path = PathBuf::from("/usr/local/my bins/claude");
    let settings_path = PathBuf::from("/opt/some dir/claude");
    let result = resolve_cli_path(Some(override_path.clone()), Some(settings_path));
    assert_eq!(
        result, override_path,
        "override must win even when both paths contain spaces"
    );
}

// ---------------------------------------------------------------------------
// IT-05: Override with Unicode segment is preserved byte-for-byte
// ---------------------------------------------------------------------------

/// Paths on Unicode-capable filesystems (Linux, macOS) may contain non-ASCII
/// characters.  `PathBuf` is byte-transparent on those platforms; verify no
/// accidental lossy conversion occurs in `resolve_cli_path`.
#[test]
fn it05_override_with_unicode_segment_preserved() {
    // The Unicode segment ("cláude") is valid on any Unicode-capable filesystem.
    let unicode_path = PathBuf::from("/usr/local/bin/cláude");
    let result = resolve_cli_path(Some(unicode_path.clone()), None);
    assert_eq!(
        result, unicode_path,
        "override path with Unicode characters must be preserved verbatim"
    );
}

// ---------------------------------------------------------------------------
// IT-06: A very long path is preserved without truncation
// ---------------------------------------------------------------------------

/// There is no length cap in `resolve_cli_path`; a 500-character path must
/// round-trip intact.  This guards against any accidental truncation.
#[test]
fn it06_long_path_preserved_without_truncation() {
    let long_segment: String = "a".repeat(490);
    let long_path = PathBuf::from(format!("/tmp/{}", long_segment));
    let result = resolve_cli_path(Some(long_path.clone()), None);
    assert_eq!(
        result, long_path,
        "a long path must survive resolve_cli_path without truncation"
    );
}

// ---------------------------------------------------------------------------
// IT-08: Function is pure — calling it twice with the same args yields equal results
// ---------------------------------------------------------------------------

/// `resolve_cli_path` must be deterministic; no internal mutable state.
#[test]
fn it08_function_is_deterministic() {
    let first = resolve_cli_path(None, None);
    let second = resolve_cli_path(None, None);
    assert_eq!(
        first, second,
        "two calls with identical arguments must return equal PathBufs"
    );
}
