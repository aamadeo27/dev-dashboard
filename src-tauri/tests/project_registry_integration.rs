/// Integration tests for `ProjectRegistry` (T2.1).
///
/// These tests exercise the save → load cycle and path-canonicalization logic
/// against a real temporary directory using `tempfile::TempDir`.  They run
/// across the crate boundary (importing `dev_dashboard_lib` as an external
/// crate) and therefore complement the inline unit tests in
/// `src/projects/mod.rs` by verifying the public API contract as callers see it.
///
/// # What is NOT tested here
///
/// - Tauri command wiring (`add_project` / `list_projects` / etc.) — those
///   require a running Tauri runtime and are verified by the TypeScript
///   IPC-wrapper unit tests in `src/ipc/commands.test.ts`.
/// - Unit-level logic already covered by `#[cfg(test)]` in `mod.rs`:
///   duplicate path rejection, file-path rejection, remove/rename/relocate
///   basic happy paths, is_missing live check.
///
/// # How to run
///
/// ```sh
/// cargo test -p dev-dashboard-lib --test project_registry_integration
/// ```
///
/// (Cargo is not available in the CI sandbox that generates this file; tests
/// are authored to be structurally correct and must be verified at runtime
/// with `cargo test`.)
use dev_dashboard_lib::projects::ProjectRegistry;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("create temp dir")
}

// ---------------------------------------------------------------------------
// TC-PR-INT-01: projects.json is created at the correct path inside config_dir
// ---------------------------------------------------------------------------

/// After `add_project`, the file must appear at
/// `<config_dir>/projects.json` — not at a sub-path or with a different name.
/// A stray `.tmp` file must not remain.
#[tokio::test]
async fn projects_json_created_at_correct_location() {
    let config = temp_dir();
    let project_dir = temp_dir();

    assert!(
        !config.path().join("projects.json").exists(),
        "projects.json must not exist before the first mutation"
    );

    let mut registry = ProjectRegistry::load(config.path());
    registry
        .add_project(project_dir.path().to_path_buf())
        .await
        .expect("add_project must succeed");

    let expected_path = config.path().join("projects.json");
    assert!(
        expected_path.exists(),
        "projects.json must exist at <config_dir>/projects.json after add_project"
    );

    // No deterministic tmp filename remains (we use UUID-based tmp names now).
    // Verify no file matching "projects.*.tmp" exists.
    let entries: Vec<_> = std::fs::read_dir(config.path())
        .expect("read config dir")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with("projects.")
                && e.file_name().to_string_lossy().ends_with(".tmp")
        })
        .collect();
    assert!(
        entries.is_empty(),
        "temp file(s) must be cleaned up after atomic rename: {:?}",
        entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-02: full round-trip — all Project fields survive save and reload
// ---------------------------------------------------------------------------

/// Every field in `Project` that IS persisted must deserialize back to the
/// same value.  This guards against accidental field omissions in the serde
/// derive or a typo in a field name.
#[tokio::test]
async fn round_trip_all_project_fields_survive_save_and_reload() {
    let config = temp_dir();
    let project_dir = temp_dir();

    let (saved_id, saved_name, saved_tags, saved_added_at, saved_last_modified) = {
        let mut registry = ProjectRegistry::load(config.path());
        let proj = registry
            .add_project(project_dir.path().to_path_buf())
            .await
            .expect("add_project must succeed");

        let id = proj.id.clone();
        let name = proj.name.clone();
        let added_at = proj.added_at;

        let updated = registry
            .set_project_tags(&id, vec!["rust".to_string(), "tauri".to_string()])
            .await
            .expect("set_project_tags must succeed");

        (
            id,
            name,
            updated.tags.clone(),
            added_at,
            updated.last_modified,
        )
    };

    // Reload from disk into a fresh registry.
    let registry2 = ProjectRegistry::load(config.path());
    let list = registry2.list_projects().await;

    assert_eq!(list.len(), 1, "exactly one project must survive reload");

    let p = &list[0];
    assert_eq!(p.id, saved_id, "id must survive round-trip");
    assert_eq!(p.name, saved_name, "name must survive round-trip");
    assert_eq!(p.tags, saved_tags, "tags must survive round-trip");
    assert_eq!(
        p.added_at, saved_added_at,
        "added_at must survive round-trip"
    );
    assert_eq!(
        p.last_modified, saved_last_modified,
        "last_modified must survive round-trip"
    );
    assert_eq!(p.language, None, "language (None) must survive round-trip");
    assert_eq!(
        p.package_manager, None,
        "package_manager (None) must survive round-trip"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-03: is_missing is NOT written to the JSON file
// ---------------------------------------------------------------------------

/// `is_missing` is computed live on every `list_projects()` call and must
/// never be serialized to disk.  Persisting it would cause stale values to
/// be read back on reload — a project deleted while the app is closed would
/// not show as missing on the next launch.
#[tokio::test]
async fn is_missing_not_written_to_json() {
    let config = temp_dir();
    let project_dir = temp_dir();

    let mut registry = ProjectRegistry::load(config.path());
    registry
        .add_project(project_dir.path().to_path_buf())
        .await
        .expect("add_project must succeed");

    let json_path = config.path().join("projects.json");
    let json_text =
        std::fs::read_to_string(&json_path).expect("projects.json must exist after add_project");

    assert!(
        !json_text.contains("\"is_missing\""),
        "projects.json must not contain the key \"is_missing\" (it is computed on read, \
         not persisted); found in raw JSON: {json_text}"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-04: is_missing computed live after directory is deleted
// ---------------------------------------------------------------------------

/// After a project directory is dropped (deleted), `list_projects()` must
/// report `is_missing: true` for that project.  The field must read `false`
/// while the directory still exists.
///
/// This complements the unit test in `mod.rs` by verifying the behavior
/// across the crate boundary.
#[tokio::test]
async fn list_projects_is_missing_true_after_directory_deleted() {
    let config = temp_dir();
    let project_dir = temp_dir();

    let mut registry = ProjectRegistry::load(config.path());
    let proj = registry
        .add_project(project_dir.path().to_path_buf())
        .await
        .expect("add_project must succeed");

    // is_missing must be false while the directory exists.
    assert!(
        !proj.is_missing,
        "is_missing must be false immediately after add"
    );

    let list_before = registry.list_projects().await;
    assert!(
        !list_before[0].is_missing,
        "is_missing must be false before deletion"
    );

    // Delete the directory.
    drop(project_dir);

    let list_after = registry.list_projects().await;
    assert_eq!(list_after.len(), 1);
    assert!(
        list_after[0].is_missing,
        "is_missing must be true after directory is deleted"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-05: empty registry on missing projects.json (first run)
// ---------------------------------------------------------------------------

/// Loading from a config directory that contains no `projects.json` must
/// return an empty registry — not an error.  This is the normal first-run
/// path.
#[tokio::test]
async fn load_with_no_projects_json_returns_empty_registry() {
    let config = temp_dir();

    // Verify no file exists to start with.
    assert!(!config.path().join("projects.json").exists());

    let registry = ProjectRegistry::load(config.path());
    assert!(
        registry.list_projects().await.is_empty(),
        "registry loaded from empty config dir must have no projects"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-06: corrupt projects.json yields empty registry (no panic)
// ---------------------------------------------------------------------------

/// If `projects.json` is malformed, `load()` must fall back to an empty
/// registry rather than panicking or propagating the parse error to the
/// caller.
#[tokio::test]
async fn corrupt_projects_json_returns_empty_registry() {
    let config = temp_dir();
    let json_path = config.path().join("projects.json");

    std::fs::write(&json_path, b"{ invalid json [[[").expect("write corrupt file");

    let registry = ProjectRegistry::load(config.path());
    assert!(
        registry.list_projects().await.is_empty(),
        "corrupt projects.json must yield empty registry, not a panic or error"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-07: multiple projects all round-trip correctly
// ---------------------------------------------------------------------------

/// Two projects added in the same session must both survive save/load, with
/// the same ids and in the same order.
#[tokio::test]
async fn multiple_projects_all_survive_round_trip() {
    let config = temp_dir();
    let dir_a = temp_dir();
    let dir_b = temp_dir();

    let (id_a, id_b) = {
        let mut registry = ProjectRegistry::load(config.path());
        let a = registry
            .add_project(dir_a.path().to_path_buf())
            .await
            .expect("add project A");
        let b = registry
            .add_project(dir_b.path().to_path_buf())
            .await
            .expect("add project B");
        (a.id.clone(), b.id.clone())
    };

    let registry2 = ProjectRegistry::load(config.path());
    let list = registry2.list_projects().await;

    assert_eq!(list.len(), 2, "both projects must survive reload");
    assert_eq!(list[0].id, id_a, "first project id must survive reload");
    assert_eq!(list[1].id, id_b, "second project id must survive reload");
}

// ---------------------------------------------------------------------------
// TC-PR-INT-08: remove_project is persisted — project absent after reload
// ---------------------------------------------------------------------------

/// After removing a project and reloading, the removed project must not
/// appear in the list.
#[tokio::test]
async fn remove_project_persisted_across_reload() {
    let config = temp_dir();
    let dir_a = temp_dir();
    let dir_b = temp_dir();

    let id_a = {
        let mut registry = ProjectRegistry::load(config.path());
        let a = registry
            .add_project(dir_a.path().to_path_buf())
            .await
            .expect("add project A");
        registry
            .add_project(dir_b.path().to_path_buf())
            .await
            .expect("add project B");
        registry
            .remove_project(&a.id)
            .await
            .expect("remove project A");
        a.id.clone()
    };

    let registry2 = ProjectRegistry::load(config.path());
    let list = registry2.list_projects().await;

    assert_eq!(list.len(), 1, "only one project must remain after reload");
    assert_ne!(
        list[0].id, id_a,
        "removed project must not appear after reload"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-09: path canonicalization — Windows trailing backslash
// ---------------------------------------------------------------------------

/// On Windows a path with a trailing backslash (e.g. `C:\foo\bar\`)
/// canonicalizes to the same path as `C:\foo\bar`.  Adding the same
/// directory via two different string representations must yield an error
/// (AlreadyExists), not two separate entries.
///
/// This test is Windows-only because POSIX systems handle trailing slashes
/// differently (the kernel strips them, but canonicalize() behavior varies).
///
/// Note: `AppError` is `pub(crate)` so we cannot match the variant directly
/// from an integration test.  Instead we check that the call errors and that
/// the error message contains "already" — a stable substring of the
/// `AlreadyExists` message format.
#[cfg(target_os = "windows")]
#[tokio::test]
async fn add_project_trailing_separator_is_same_canonical_path() {
    let config = temp_dir();
    let project_dir = temp_dir();

    let base = project_dir.path().to_path_buf();

    // Construct a path with a trailing backslash by appending to the string.
    let mut path_str = base.to_string_lossy().into_owned();
    if !path_str.ends_with('\\') {
        path_str.push('\\');
    }
    let path_with_slash = std::path::PathBuf::from(path_str);

    let mut registry = ProjectRegistry::load(config.path());
    registry
        .add_project(base.clone())
        .await
        .expect("first add must succeed");

    let err = registry
        .add_project(path_with_slash)
        .await
        .expect_err("second add with trailing separator must fail");

    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("already"),
        "expected an AlreadyExists error for same canonical path via trailing separator, \
         got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-10: relocate_project to same canonical path as another project
// ---------------------------------------------------------------------------

/// `relocate_project` must return an AlreadyExists error when the new path
/// canonicalizes to the path already held by a different project.  This test
/// drives the same logic exercised by the inline unit test but verifies it
/// from the cross-crate integration perspective.
///
/// Note: `AppError` is `pub(crate)` so we verify the error's Display message
/// contains "already" rather than pattern-matching the variant.
#[tokio::test]
async fn relocate_project_to_path_of_another_project_returns_already_exists() {
    let config = temp_dir();
    let dir_a = temp_dir();
    let dir_b = temp_dir();

    let mut registry = ProjectRegistry::load(config.path());
    let proj_a = registry
        .add_project(dir_a.path().to_path_buf())
        .await
        .expect("add project A");
    let proj_b = registry
        .add_project(dir_b.path().to_path_buf())
        .await
        .expect("add project B");

    // Attempt to relocate B to A's path.
    let err = registry
        .relocate_project(&proj_b.id, dir_a.path().to_path_buf())
        .await
        .expect_err("relocate to another project's canonical path must fail");

    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("already"),
        "expected AlreadyExists when relocating to another project's path, got: {err:?}"
    );

    // Sanity: A's path is unchanged.
    let list = registry.list_projects().await;
    let a_entry = list
        .iter()
        .find(|p| p.id == proj_a.id)
        .expect("proj A must still exist");
    assert_eq!(
        a_entry.path,
        dir_a.path().canonicalize().unwrap(),
        "project A's path must be unchanged"
    );
}

// ---------------------------------------------------------------------------
// TC-PR-INT-11: set_project_tags round-trip — tags survive save and reload
// ---------------------------------------------------------------------------

/// Tags set via `set_project_tags` must survive the full save → reload cycle,
/// lowercased and deduplicated as documented.
#[tokio::test]
async fn set_project_tags_survives_round_trip() {
    let config = temp_dir();
    let project_dir = temp_dir();

    {
        let mut registry = ProjectRegistry::load(config.path());
        let proj = registry
            .add_project(project_dir.path().to_path_buf())
            .await
            .expect("add_project must succeed");
        registry
            .set_project_tags(
                &proj.id,
                vec![
                    "Rust".to_string(),
                    "rust".to_string(), // duplicate — must be dropped
                    "Tauri".to_string(),
                    "  ".to_string(), // whitespace-only — must be dropped
                ],
            )
            .await
            .expect("set_project_tags must succeed");
    }

    let registry2 = ProjectRegistry::load(config.path());
    let list = registry2.list_projects().await;
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].tags,
        vec!["rust", "tauri"],
        "normalized tags must survive round-trip"
    );
}
