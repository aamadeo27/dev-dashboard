/// Integration tests for `SettingsStore` (T1.1).
///
/// These tests exercise the full load → patch → save → reload cycle against a
/// real temporary directory, using `tempfile::TempDir` for isolation.  They
/// are distinct from the inline unit tests in `src/settings/mod.rs` and cover
/// scenarios that either require filesystem state to span multiple function
/// calls or verify boundary conditions not addressed by the unit suite.
///
/// # What is NOT tested here
///
/// - Unit-level concerns already covered by `#[cfg(test)]` in `mod.rs`:
///   default settings on missing file, basic patch/validate, corrupt-JSON
///   immediate return, atomic write integrity.
/// - Tauri command wiring (`get_settings` / `update_settings`) — those require
///   a running Tauri app context and are covered by the frontend smoke tests.
use dev_dashboard_lib::settings::{Settings, SettingsPatch, SettingsStore, ViewMode};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("create temp dir")
}

// ---------------------------------------------------------------------------
// TC-INT-01: settings.json is created at the correct path inside config_dir
// ---------------------------------------------------------------------------

/// After `save()`, the file must appear at `<config_dir>/settings.json` —
/// not at a sub-path or with a different name.
#[tokio::test]
async fn settings_file_created_at_correct_location() {
    let dir = temp_dir();
    let store = SettingsStore::load(dir.path());

    // File must not exist before the first save.
    assert!(
        !dir.path().join("settings.json").exists(),
        "settings.json must not exist before save"
    );

    store.save(dir.path()).await.expect("save must succeed");

    let expected_path = dir.path().join("settings.json");
    assert!(
        expected_path.exists(),
        "settings.json must exist at <config_dir>/settings.json after save"
    );

    // Confirm no stray `.tmp` is left behind.
    assert!(
        !dir.path().join("settings.json.tmp").exists(),
        "temp file must be cleaned up after atomic rename"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-02: full round-trip — all fields survive save → load intact
// ---------------------------------------------------------------------------

/// Every field in `Settings` must deserialize back to the same value that was
/// patched in.  This guards against accidental field omissions in the serde
/// derive or a typo in a field name.
#[tokio::test]
async fn round_trip_all_fields_survive_save_and_reload() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    // Create a real file so the is_file() check passes on all platforms.
    let cli_file = dir.path().join("claude");
    tokio::fs::write(&cli_file, b"").await.unwrap();
    // parent_dir points to dir itself (guaranteed to be a directory).
    let projects_dir = dir.path().join("projects");

    let patch = SettingsPatch {
        parent_dir: Some(projects_dir.clone()),
        claude_cli_path: Some(cli_file.clone()),
        git_poll_interval_secs: Some(300),
        usage_poll_interval_secs: Some(120),
        retention_days: Some(7),
        retention_size_mb: Some(200),
        view_mode: Some(ViewMode::List),
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("patch must succeed");

    // Reload from disk into a fresh store.
    let store2 = SettingsStore::load(dir.path());
    let s = store2.settings();

    assert_eq!(
        s.parent_dir,
        Some(projects_dir),
        "parent_dir must survive round-trip"
    );
    assert_eq!(
        s.claude_cli_path,
        Some(cli_file),
        "claude_cli_path must survive round-trip"
    );
    assert_eq!(
        s.git_poll_interval_secs, 300,
        "git_poll_interval_secs must survive round-trip"
    );
    assert_eq!(
        s.usage_poll_interval_secs, 120,
        "usage_poll_interval_secs must survive round-trip"
    );
    assert_eq!(
        s.retention_days, 7,
        "retention_days must survive round-trip"
    );
    assert_eq!(
        s.retention_size_mb, 200,
        "retention_size_mb must survive round-trip"
    );
    assert_eq!(
        s.view_mode,
        ViewMode::List,
        "view_mode must survive round-trip"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-03: git_poll_interval_secs — min boundary (5 passes, 4 fails)
// ---------------------------------------------------------------------------

/// The minimum accepted value for `git_poll_interval_secs` is 5.
#[tokio::test]
async fn git_poll_min_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        git_poll_interval_secs: Some(5),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("git_poll=5 must be accepted");
    assert_eq!(store.settings().git_poll_interval_secs, 5);
}

/// One below the minimum (4) must be rejected with `InvalidInput`.
#[tokio::test]
async fn git_poll_below_min_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        git_poll_interval_secs: Some(4),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("git_poll_interval_secs"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-04: git_poll_interval_secs — max boundary (3600 passes, 3601 fails)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn git_poll_max_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        git_poll_interval_secs: Some(3600),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("git_poll=3600 must be accepted");
    assert_eq!(store.settings().git_poll_interval_secs, 3600);
}

#[tokio::test]
async fn git_poll_above_max_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        git_poll_interval_secs: Some(3601),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("git_poll_interval_secs"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-05: usage_poll_interval_secs — min boundary (30 passes, 29 fails)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn usage_poll_min_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        usage_poll_interval_secs: Some(30),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("usage_poll=30 must be accepted");
    assert_eq!(store.settings().usage_poll_interval_secs, 30);
}

#[tokio::test]
async fn usage_poll_below_min_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        usage_poll_interval_secs: Some(29),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("usage_poll_interval_secs"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-06: usage_poll_interval_secs — max boundary (3600 passes, 3601 fails)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn usage_poll_max_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        usage_poll_interval_secs: Some(3600),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("usage_poll=3600 must be accepted");
    assert_eq!(store.settings().usage_poll_interval_secs, 3600);
}

#[tokio::test]
async fn usage_poll_above_max_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        usage_poll_interval_secs: Some(3601),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("usage_poll_interval_secs"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-07: retention_days — min boundary (1 passes, 0 fails)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retention_days_min_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_days: Some(1),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("retention_days=1 must be accepted");
    assert_eq!(store.settings().retention_days, 1);
}

/// `retention_days=0` must be rejected because the minimum is 1.
/// `u32` cannot be negative, so 0 is the only below-minimum value.
#[tokio::test]
async fn retention_days_zero_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_days: Some(0),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("retention_days"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-08: retention_size_mb — min boundary (50 passes, 49 fails)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retention_size_mb_min_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_size_mb: Some(50),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("retention_size_mb=50 must be accepted");
    assert_eq!(store.settings().retention_size_mb, 50);
}

#[tokio::test]
async fn retention_size_mb_below_min_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_size_mb: Some(49),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("retention_size_mb"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-08b: retention_days — max boundary (90 passes, 91 fails)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retention_days_max_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_days: Some(90),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("retention_days=90 must be accepted");
    assert_eq!(store.settings().retention_days, 90);
}

#[tokio::test]
async fn retention_days_above_max_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_days: Some(91),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("retention_days"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-08c: retention_size_mb — max boundary (10240 passes, 10241 fails)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retention_size_mb_max_boundary_passes() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_size_mb: Some(10_240),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("retention_size_mb=10240 must be accepted");
    assert_eq!(store.settings().retention_size_mb, 10_240);
}

#[tokio::test]
async fn retention_size_mb_above_max_rejected() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    let patch = SettingsPatch {
        retention_size_mb: Some(10_241),
        ..SettingsPatch::default()
    };
    let err = store.patch(patch, dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("retention_size_mb"),
        "error message must name the offending field"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-09: corrupt file archived; subsequent fresh load returns defaults
// ---------------------------------------------------------------------------

/// After a corrupt load produces `settings.json.broken`, a second
/// `SettingsStore::load()` call (with no file present) must return defaults —
/// verifying the two-step recovery path works end-to-end.
#[test]
fn corrupt_file_archived_and_fresh_load_returns_defaults() {
    let dir = temp_dir();
    let settings_path = dir.path().join("settings.json");
    let broken_path = dir.path().join("settings.json.broken");

    // Write a corrupt file.
    std::fs::write(&settings_path, b"{ bad json").expect("write corrupt file");

    // First load: should fall back to defaults and archive the file.
    let store1 = SettingsStore::load(dir.path());
    assert_eq!(
        store1.settings().git_poll_interval_secs,
        10,
        "first load after corruption must return defaults"
    );
    assert!(broken_path.exists(), "broken file must be archived");
    assert!(
        !settings_path.exists(),
        "original settings.json must have been renamed away"
    );

    // Second load: no settings.json exists; must return defaults again
    // (not fail or panic because the `.broken` file is sitting there).
    let store2 = SettingsStore::load(dir.path());
    assert_eq!(
        store2.settings().git_poll_interval_secs,
        10,
        "second load (no file) must return defaults"
    );
    assert_eq!(store2.settings().view_mode, ViewMode::Grid);
}

// ---------------------------------------------------------------------------
// TC-INT-10: None patch fields are no-ops — do not clear existing values
// ---------------------------------------------------------------------------

/// Setting `parent_dir` and `claude_cli_path` to `Some` values and then
/// applying an all-None patch must leave both fields unchanged.
/// This guards against a hypothetical regression where `None` in the patch
/// is treated as "clear to None" rather than "leave as-is".
#[tokio::test]
async fn none_patch_fields_are_no_ops_for_path_fields() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    // Create a real file so the is_file() check passes on all platforms.
    let cli_file = dir.path().join("claude");
    tokio::fs::write(&cli_file, b"").await.unwrap();
    // projects_dir does not need to exist per current validation (only checked if it exists).
    let projects_dir = dir.path().join("projects");

    // First patch: set the path fields.
    let set_patch = SettingsPatch {
        parent_dir: Some(projects_dir.clone()),
        claude_cli_path: Some(cli_file.clone()),
        ..SettingsPatch::default()
    };
    store
        .patch(set_patch, dir.path())
        .await
        .expect("first patch must succeed");

    assert_eq!(store.settings().parent_dir, Some(projects_dir.clone()));
    assert_eq!(store.settings().claude_cli_path, Some(cli_file.clone()));

    // Second patch: all None — must not clear the path fields.
    store
        .patch(SettingsPatch::default(), dir.path())
        .await
        .expect("empty patch must succeed");

    assert_eq!(
        store.settings().parent_dir,
        Some(projects_dir),
        "parent_dir must not be cleared by a None patch"
    );
    assert_eq!(
        store.settings().claude_cli_path,
        Some(cli_file),
        "claude_cli_path must not be cleared by a None patch"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-11: path fields round-trip through save/load
// ---------------------------------------------------------------------------

/// `parent_dir` and `claude_cli_path` are `PathBuf` on the Rust side and
/// serialized as strings.  Verify the serde round-trip preserves the exact
/// path on disk.
#[tokio::test]
async fn path_fields_survive_round_trip() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    // Create a real file so the is_file() check passes on all platforms.
    let cli_file = dir.path().join("claude");
    tokio::fs::write(&cli_file, b"").await.unwrap();
    // projects_dir does not need to exist per current validation.
    let projects_dir = dir.path().join("projects");

    let patch = SettingsPatch {
        parent_dir: Some(projects_dir.clone()),
        claude_cli_path: Some(cli_file.clone()),
        ..SettingsPatch::default()
    };
    store
        .patch(patch, dir.path())
        .await
        .expect("patch must succeed");

    let store2 = SettingsStore::load(dir.path());
    assert_eq!(
        store2.settings().parent_dir,
        Some(projects_dir),
        "parent_dir must survive save/load round-trip"
    );
    assert_eq!(
        store2.settings().claude_cli_path,
        Some(cli_file),
        "claude_cli_path must survive save/load round-trip"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-12: sequential patches accumulate correctly
// ---------------------------------------------------------------------------

/// Applying two sequential patches must leave the store in the state
/// produced by both patches combined.  The Tokio mutex serialises concurrent
/// access at runtime; this test confirms the sequential logic is correct.
#[tokio::test]
async fn sequential_patches_accumulate_state() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    // Patch 1: change git_poll and view_mode.
    store
        .patch(
            SettingsPatch {
                git_poll_interval_secs: Some(60),
                view_mode: Some(ViewMode::List),
                ..SettingsPatch::default()
            },
            dir.path(),
        )
        .await
        .expect("patch 1 must succeed");

    // Patch 2: change retention_days only.
    store
        .patch(
            SettingsPatch {
                retention_days: Some(14),
                ..SettingsPatch::default()
            },
            dir.path(),
        )
        .await
        .expect("patch 2 must succeed");

    // Reload and verify combined state.
    let store2 = SettingsStore::load(dir.path());
    assert_eq!(
        store2.settings().git_poll_interval_secs,
        60,
        "patch 1 value must persist"
    );
    assert_eq!(
        store2.settings().view_mode,
        ViewMode::List,
        "patch 1 value must persist"
    );
    assert_eq!(
        store2.settings().retention_days,
        14,
        "patch 2 value must persist"
    );
    // Fields not touched by either patch must keep their defaults.
    assert_eq!(
        store2.settings().usage_poll_interval_secs,
        60,
        "untouched field must retain default"
    );
    assert_eq!(
        store2.settings().retention_size_mb,
        500,
        "untouched field must retain default"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-13: failed patch does not mutate the store
// ---------------------------------------------------------------------------

/// `patch()` validates ALL fields before mutating any.  If validation fails,
/// the store must be identical to its state before the call.
#[tokio::test]
async fn failed_patch_does_not_mutate_store() {
    let dir = temp_dir();
    let mut store = SettingsStore::load(dir.path());

    // First: set a known value.
    store
        .patch(
            SettingsPatch {
                git_poll_interval_secs: Some(120),
                ..SettingsPatch::default()
            },
            dir.path(),
        )
        .await
        .expect("valid patch must succeed");

    assert_eq!(store.settings().git_poll_interval_secs, 120);

    // Now submit a patch that includes one invalid field alongside a valid one.
    let result = store
        .patch(
            SettingsPatch {
                git_poll_interval_secs: Some(999), // valid
                usage_poll_interval_secs: Some(1), // invalid — below min 30
                ..SettingsPatch::default()
            },
            dir.path(),
        )
        .await;

    assert!(result.is_err(), "patch with invalid field must fail");

    // The git_poll field must NOT have been updated.
    assert_eq!(
        store.settings().git_poll_interval_secs,
        120,
        "store must not be mutated when patch validation fails"
    );
}

// ---------------------------------------------------------------------------
// TC-INT-14: save creates config_dir if it does not exist yet
// ---------------------------------------------------------------------------

/// `save()` calls `tokio::fs::create_dir_all(config_dir)` before writing.
/// Pass a path to a not-yet-existing subdirectory and verify both the
/// directory and the file are created.
#[tokio::test]
async fn save_creates_config_dir_if_missing() {
    let root = temp_dir();
    let nested = root.path().join("a").join("b").join("c");

    assert!(!nested.exists(), "nested dir must not exist before save");

    let store = SettingsStore::load(&nested); // dir doesn't exist → returns defaults
    store
        .save(&nested)
        .await
        .expect("save into nested non-existent dir must succeed");

    assert!(nested.exists(), "save must have created the config_dir");
    assert!(
        nested.join("settings.json").exists(),
        "settings.json must exist after save"
    );
}
