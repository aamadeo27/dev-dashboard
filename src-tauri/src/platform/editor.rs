use std::path::PathBuf;

use tauri::Emitter;

use crate::error::{AppError, AppResult};

/// Look up a project path from the registry by id.
///
/// Returns `AppError::NotFound` if no project has the given id.
/// This is a pure helper extracted for testability (no `AppHandle` dependency).
pub fn lookup_project_path(
    registry: &crate::projects::ProjectRegistry,
    id: &str,
) -> AppResult<PathBuf> {
    // get_project_path is a synchronous lookup of the stored path; is_missing
    // (which requires the async list_projects) is irrelevant here.
    registry
        .get_project_path(id)
        .ok_or_else(|| AppError::NotFound(format!("project id: {id}")))
}

/// Open the project directory in an editor.
///
/// If `$EDITOR` is set, spawns `$EDITOR <path>` as a detached child process.
/// Otherwise falls back to `tauri_plugin_opener::open_path` for the OS default
/// file association.
///
/// On failure: emits a `toast:show` error event via `app` and returns
/// `AppError::Io`.
pub async fn open_in_editor_impl(id: &str, path: PathBuf, app: &tauri::AppHandle) -> AppResult<()> {
    tracing::info!(component = "platform", id = ?id, path = ?path, "open_in_editor invoked");

    let editor = std::env::var("EDITOR").ok();

    let result = if let Some(editor_cmd) = editor {
        tokio::process::Command::new(&editor_cmd)
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(false)
            .spawn()
            .map(|_| ())
            .map_err(AppError::Io)
    } else {
        tauri_plugin_opener::open_path(&path, None::<&str>)
            .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))
    };

    if let Err(ref e) = result {
        tracing::warn!(component = "platform", id = ?id, error = ?e, "open_in_editor failed");
        app.emit(
            crate::ipc::events::TOAST_SHOW,
            serde_json::json!({
                "kind": "error",
                "title": "Cannot open editor",
                "body": e.to_string(),
            }),
        )
        .ok();
    }

    result
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::ProjectRegistry;
    use tempfile::TempDir;

    async fn make_registry_with_project() -> (TempDir, TempDir, ProjectRegistry, String) {
        let config = TempDir::new().expect("config tempdir");
        let proj_dir = TempDir::new().expect("proj tempdir");
        let mut registry = ProjectRegistry::load(config.path());
        let project = registry
            .add_project(proj_dir.path().to_path_buf())
            .await
            .expect("add project");
        let id = project.id.clone();
        (config, proj_dir, registry, id)
    }

    #[tokio::test]
    async fn lookup_project_path_returns_path_for_known_id() {
        let (_config, proj_dir, registry, id) = make_registry_with_project().await;
        let canonical = proj_dir.path().canonicalize().expect("canonicalize");

        let path = lookup_project_path(&registry, &id).expect("lookup");

        assert_eq!(path, canonical);
    }

    #[tokio::test]
    async fn lookup_project_path_returns_not_found_for_unknown_id() {
        let (_config, _proj_dir, registry, _id) = make_registry_with_project().await;

        let err = lookup_project_path(&registry, "no-such-id").unwrap_err();

        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn lookup_project_path_returns_not_found_for_empty_id() {
        let (_config, _proj_dir, registry, _id) = make_registry_with_project().await;

        let err = lookup_project_path(&registry, "").unwrap_err();

        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound for empty id, got: {err:?}"
        );
    }

    #[test]
    fn lookup_project_path_returns_not_found_on_empty_registry() {
        let config = TempDir::new().expect("config tempdir");
        let registry = ProjectRegistry::load(config.path());

        let err = lookup_project_path(&registry, "any-id").unwrap_err();

        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound on empty registry, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn lookup_project_path_finds_correct_project_among_multiple() {
        let config = TempDir::new().expect("config tempdir");
        let dir_a = TempDir::new().expect("dir_a");
        let dir_b = TempDir::new().expect("dir_b");
        let mut registry = ProjectRegistry::load(config.path());

        let proj_a = registry
            .add_project(dir_a.path().to_path_buf())
            .await
            .expect("add a");
        let proj_b = registry
            .add_project(dir_b.path().to_path_buf())
            .await
            .expect("add b");

        let canonical_b = dir_b.path().canonicalize().expect("canonicalize b");

        let path = lookup_project_path(&registry, &proj_b.id).expect("lookup b");

        assert_eq!(path, canonical_b);
        assert_ne!(path, dir_a.path().canonicalize().expect("canonicalize a"));
        let _ = proj_a;
    }

    /// `lookup_project_path` must return the stored path even when the
    /// directory no longer exists on disk (is_missing=true at list_projects
    /// time). The T2.8 spec says "is_missing=true does not prevent the
    /// attempt — the OS handles the path". The lookup must not filter out
    /// missing projects.
    #[tokio::test]
    async fn lookup_project_path_returns_path_for_is_missing_project() {
        let config = TempDir::new().expect("config tempdir");
        // Create and immediately drop the project directory so it no longer
        // exists when list_projects() is called. list_projects() sets
        // is_missing=true on any project whose path.exists() is false.
        let canonical = {
            let proj_dir = TempDir::new().expect("proj tempdir");
            let path = proj_dir.path().canonicalize().expect("canonicalize");
            let mut registry = ProjectRegistry::load(config.path());
            registry
                .add_project(proj_dir.path().to_path_buf())
                .await
                .expect("add project");
            // proj_dir is dropped here — directory is deleted.
            path
        };

        // Reload the registry from disk; the stored path still refers to the
        // now-deleted directory (is_missing=true when listed).
        let registry = ProjectRegistry::load(config.path());
        let listed = registry.list_projects().await;
        assert_eq!(listed.len(), 1);
        assert!(
            listed[0].is_missing,
            "project with deleted path must be reported as is_missing=true"
        );

        // lookup_project_path must still return the stored path.
        let path = lookup_project_path(&registry, &listed[0].id)
            .expect("lookup must succeed even when is_missing=true");

        assert_eq!(path, canonical);
    }
}
