pub mod git;
pub(crate) mod scanner;

use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// A registered project entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "export-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-bindings", ts(export))]
pub struct Project {
    pub id: String,
    pub name: String,
    #[cfg_attr(feature = "export-bindings", ts(type = "string"))]
    pub path: PathBuf,
    pub tags: Vec<String>,
    pub language: Option<String>,
    pub package_manager: Option<String>,
    pub added_at: chrono::DateTime<chrono::Utc>,
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
    /// Computed live in `list_projects()`; sent to the frontend over IPC but
    /// never persisted to `projects.json` (stripped in `save`) and ignored when
    /// loading (recomputed). `skip_deserializing` keeps it out of the loaded
    /// value while still serializing it for IPC responses.
    #[serde(skip_deserializing)]
    pub is_missing: bool,
}

/// In-memory registry of registered projects, backed by `projects.json` in the
/// application config directory. All mutations persist atomically (write to
/// `.tmp`, then rename).
pub struct ProjectRegistry {
    projects: Vec<Project>,
    config_dir: PathBuf,
}

impl ProjectRegistry {
    /// Loads the project registry from `config_dir/projects.json`.
    ///
    /// Uses synchronous `std::fs` I/O intentionally: `load()` is called once at
    /// startup before the Tokio runtime is initialised. All post-construction
    /// mutations use `tokio::fs` (see `save()`).
    ///
    /// If the file does not exist, returns an empty registry (normal first-run).
    /// If the file exists but cannot be parsed, logs a warning and returns an
    /// empty registry (data loss is better than a hard crash at startup).
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join("projects.json");
        let projects = match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(
                    component = "project_registry",
                    path = %path.display(),
                    "projects.json not found; starting with empty registry"
                );
                Vec::new()
            }
            Err(e) => {
                tracing::warn!(
                    component = "project_registry",
                    path = %path.display(),
                    error = %e,
                    "failed to read projects.json; starting with empty registry"
                );
                Vec::new()
            }
            Ok(text) => match serde_json::from_str::<Vec<Project>>(&text) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        component = "project_registry",
                        path = %path.display(),
                        error = %e,
                        "projects.json parse error; starting with empty registry"
                    );
                    Vec::new()
                }
            },
        };

        Self {
            projects,
            config_dir: config_dir.to_path_buf(),
        }
    }

    /// Persist the current in-memory state atomically to
    /// `<config_dir>/projects.json`.
    ///
    /// Writes to a uniquely-named `.tmp` file first, then renames to avoid
    /// corruption on interrupted writes. The unique tmp name prevents symlink
    /// attacks from a local attacker pre-creating a fixed filename.
    async fn save(&self) -> AppResult<()> {
        tokio::fs::create_dir_all(&self.config_dir).await?;
        let tmp_name = format!("projects.{}.tmp", Uuid::new_v4());
        let tmp_path = self.config_dir.join(&tmp_name);
        let final_path = self.config_dir.join("projects.json");
        // `is_missing` serializes for IPC but must not be persisted — strip it
        // from each entry before writing the file (it is recomputed on load).
        let mut value = serde_json::to_value(&self.projects)
            .map_err(|e| AppError::Internal(format!("serialize projects: {e}")))?;
        if let Some(entries) = value.as_array_mut() {
            for entry in entries {
                if let Some(obj) = entry.as_object_mut() {
                    obj.remove("is_missing");
                }
            }
        }
        let json = serde_json::to_string_pretty(&value)
            .map_err(|e| AppError::Internal(format!("serialize projects: {e}")))?;
        tokio::fs::write(&tmp_path, json).await?;
        tokio::fs::rename(&tmp_path, &final_path).await?;
        Ok(())
    }

    /// Canonicalize a path, returning `AppError::InvalidInput` if the path
    /// does not exist, is not a directory, or cannot be canonicalized.
    async fn canonicalize_dir(path: &Path) -> AppResult<PathBuf> {
        // tokio::fs::canonicalize resolves symlinks and verifies existence.
        let canonical = tokio::fs::canonicalize(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::InvalidInput(format!("path does not exist: {}", path.display()))
            } else {
                AppError::Io(std::io::Error::new(
                    e.kind(),
                    format!("failed to canonicalize {}: {e}", path.display()),
                ))
            }
        })?;

        // Verify it is a directory via metadata (second async call after canonicalize).
        let meta = tokio::fs::metadata(&canonical).await.map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!("failed to stat {}: {e}", canonical.display()),
            ))
        })?;

        if !meta.is_dir() {
            return Err(AppError::InvalidInput(format!(
                "path is not a directory: {}",
                path.display()
            )));
        }

        Ok(canonical)
    }

    /// Normalize tags: lowercase, trim whitespace, deduplicate (preserving
    /// first occurrence order), drop empty strings.
    fn normalize_tags(tags: &[String]) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        tags.iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .filter(|t| seen.insert(t.clone()))
            .collect()
    }

    // -----------------------------------------------------------------------
    // CRUD operations
    // -----------------------------------------------------------------------

    /// Register a new project at `path`.
    ///
    /// - Validates the path exists and is a directory.
    /// - Canonicalizes the path before storing.
    /// - Rejects duplicate canonicalized paths with `AppError::AlreadyExists`.
    /// - Assigns a UUID v7 id and uses the path basename as the initial name.
    pub async fn add_project(&mut self, path: PathBuf) -> AppResult<Project> {
        let canonical = Self::canonicalize_dir(&path).await?;

        // Reject duplicate canonicalized paths.
        if self.projects.iter().any(|p| p.path == canonical) {
            return Err(AppError::AlreadyExists(format!(
                "project already registered at {}",
                canonical.display()
            )));
        }

        let name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| canonical.to_string_lossy().into_owned());

        let canonical_clone = canonical.clone();
        let (language, package_manager) =
            tokio::task::spawn_blocking(move || crate::projects::scanner::detect(&canonical_clone))
                .await
                .map_err(|e| AppError::Internal(format!("scanner task panicked: {e}")))?;

        let project = Project {
            id: Uuid::now_v7().to_string(),
            name,
            path: canonical.clone(),
            tags: Vec::new(),
            language,
            package_manager,
            added_at: Utc::now(),
            last_modified: None,
            is_missing: false,
        };

        self.projects.push(project.clone());
        self.save().await?;

        tracing::info!(
            component = "project_registry",
            project_id = %project.id,
            path = ?canonical,
            "project added"
        );

        Ok(project)
    }

    /// Remove a project by id.
    ///
    /// Returns `AppError::NotFound` if no project has the given id.
    pub async fn remove_project(&mut self, id: &str) -> AppResult<()> {
        let pos = self.projects.iter().position(|p| p.id == id);
        match pos {
            None => {
                tracing::warn!(
                    component = "project_registry",
                    project_id = %id,
                    "project not found"
                );
                Err(AppError::NotFound(format!("project id: {id}")))
            }
            Some(i) => {
                self.projects.remove(i);
                self.save().await?;
                tracing::info!(
                    component = "project_registry",
                    project_id = %id,
                    "project removed"
                );
                Ok(())
            }
        }
    }

    /// Update the path of a project.
    ///
    /// - Errors with `AppError::NotFound` if the id does not exist.
    /// - Errors with `AppError::AlreadyExists` if `new_path` (canonicalized)
    ///   is already registered to a *different* project.
    pub async fn relocate_project(&mut self, id: &str, new_path: PathBuf) -> AppResult<Project> {
        let canonical = Self::canonicalize_dir(&new_path).await?;

        // Check for duplicate (must not belong to a different project).
        if self
            .projects
            .iter()
            .any(|p| p.path == canonical && p.id != id)
        {
            return Err(AppError::AlreadyExists(format!(
                "path already registered: {}",
                canonical.display()
            )));
        }

        let project = self
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| {
                tracing::warn!(
                    component = "project_registry",
                    project_id = %id,
                    "project not found"
                );
                AppError::NotFound(format!("project id: {id}"))
            })?;

        project.path = canonical;
        project.last_modified = Some(Utc::now());
        let result = project.clone();
        self.save().await?;
        Ok(result)
    }

    /// Replace the tags of a project.
    ///
    /// Normalization order: trim → lowercase → 32-character limit check → dedup.
    /// Returns `AppError::InvalidInput` if any tag exceeds 32 characters (after
    /// trim + lowercase), or `AppError::NotFound` if the id does not exist.
    pub async fn set_project_tags(&mut self, id: &str, tags: Vec<String>) -> AppResult<Project> {
        // FR-1.6.4 (T2.9): enforce a 32-character per-tag limit on the backend
        // (the UI also sets maxLength=32). Checked on the trimmed + lowercased
        // form so the limit matches what is actually stored.
        for tag in &tags {
            let candidate = tag.trim().to_lowercase();
            if candidate.chars().count() > 32 {
                return Err(AppError::InvalidInput(format!(
                    "tag exceeds 32-character limit: \"{candidate}\""
                )));
            }
        }

        let normalized = Self::normalize_tags(&tags);

        let project = self
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| {
                tracing::warn!(
                    component = "project_registry",
                    project_id = %id,
                    "project not found"
                );
                AppError::NotFound(format!("project id: {id}"))
            })?;

        project.tags = normalized;
        project.last_modified = Some(Utc::now());
        let result = project.clone();
        self.save().await?;
        Ok(result)
    }

    /// Not exported as a TypeScript binding per KB §5.1; debug-build IPC only.
    ///
    /// Returns `AppError::NotFound` if the id does not exist.
    pub async fn rename_project(&mut self, id: &str, name: String) -> AppResult<Project> {
        let project = self
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| {
                tracing::warn!(
                    component = "project_registry",
                    project_id = %id,
                    "project not found"
                );
                AppError::NotFound(format!("project id: {id}"))
            })?;

        project.name = name;
        project.last_modified = Some(Utc::now());
        let result = project.clone();
        self.save().await?;
        Ok(result)
    }

    /// Re-scan a project's language and package manager.
    ///
    /// Calls `scanner::detect` on the stored path, updates the fields in
    /// memory, persists via `save()`, and returns the updated project.
    ///
    /// Returns `AppError::NotFound` if no project has the given id.
    pub async fn scan_project(&mut self, id: &str) -> AppResult<Project> {
        // Clone the path first so we can release the mutable borrow before
        // awaiting the spawn_blocking future (cannot hold &mut self across await).
        let path = self
            .projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.path.clone())
            .ok_or_else(|| {
                tracing::warn!(
                    component = "project_registry",
                    project_id = %id,
                    "project not found"
                );
                AppError::NotFound(format!("project id: {id}"))
            })?;

        let (language, package_manager) =
            tokio::task::spawn_blocking(move || crate::projects::scanner::detect(&path))
                .await
                .map_err(|e| AppError::Internal(format!("scanner task panicked: {e}")))?;

        // Re-find by id after the await point — the mutable borrow was released above.
        let project = self
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::NotFound(format!("project not found: {id}")))?;
        project.language = language;
        project.package_manager = package_manager;
        project.last_modified = Some(Utc::now());
        let result = project.clone();
        self.save().await?;

        tracing::info!(
            component = "project_registry",
            project_id = %id,
            language = result.language.as_deref().unwrap_or("none"),
            package_manager = result.package_manager.as_deref().unwrap_or("none"),
            "project re-scanned"
        );

        Ok(result)
    }

    /// Return the filesystem path of the project with the given id, or `None`
    /// if no project has that id.
    ///
    /// Used by the git poller to resolve paths without holding the lock during I/O.
    pub fn get_project_path(&self, id: &str) -> Option<std::path::PathBuf> {
        self.projects
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.path.clone())
    }

    /// Return all projects. `is_missing` is computed live via an async
    /// filesystem existence check (not persisted).
    pub async fn list_projects(&self) -> Vec<Project> {
        let mut result = Vec::with_capacity(self.projects.len());
        for p in self.projects.iter() {
            let mut p = p.clone();
            p.is_missing = !tokio::fs::try_exists(&p.path).await.unwrap_or(false);
            result.push(p);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a temporary directory that is suitable as a project root (exists
    /// + is a directory). Returns both the `TempDir` guard and the path.
    fn make_project_dir() -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    /// Build a `ProjectRegistry` backed by a fresh temp config dir.
    fn make_registry() -> (TempDir, ProjectRegistry) {
        let config = TempDir::new().expect("config tempdir");
        let registry = ProjectRegistry::load(config.path());
        (config, registry)
    }

    // -----------------------------------------------------------------------
    // add_project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn add_project_returns_project_and_list_contains_it() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path.clone()).await.unwrap();

        assert!(!project.id.is_empty(), "id must be assigned");
        assert!(!project.name.is_empty(), "name must be set from basename");
        assert_eq!(project.language, None);
        assert_eq!(project.package_manager, None);
        assert!(
            !project.is_missing,
            "is_missing must be false immediately after add"
        );

        let list = registry.list_projects().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, project.id);
    }

    #[tokio::test]
    async fn add_project_duplicate_path_returns_already_exists() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        registry.add_project(proj_path.clone()).await.unwrap();
        let err = registry.add_project(proj_path).await.unwrap_err();

        assert!(
            matches!(err, AppError::AlreadyExists(_)),
            "expected AlreadyExists, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn add_project_nonexistent_path_returns_invalid_input() {
        let (_config, mut registry) = make_registry();
        let err = registry
            .add_project(PathBuf::from("/nonexistent_path_99999"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::InvalidInput(_)),
            "expected InvalidInput, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn add_project_file_path_returns_invalid_input() {
        let (_config, mut registry) = make_registry();
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("file.txt");
        std::fs::write(&file_path, b"content").unwrap();

        let err = registry.add_project(file_path).await.unwrap_err();
        assert!(
            matches!(err, AppError::InvalidInput(_)),
            "expected InvalidInput for file path, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn add_project_persists_across_reload() {
        let config = TempDir::new().expect("config tempdir");
        let (_proj_dir, proj_path) = make_project_dir();

        {
            let mut registry = ProjectRegistry::load(config.path());
            registry.add_project(proj_path.clone()).await.unwrap();
        }

        let registry2 = ProjectRegistry::load(config.path());
        let list = registry2.list_projects().await;
        assert_eq!(list.len(), 1, "project must survive reload");
    }

    // -----------------------------------------------------------------------
    // remove_project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn remove_project_removes_it_from_list() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        registry.remove_project(&project.id).await.unwrap();

        let list = registry.list_projects().await;
        assert!(list.is_empty(), "list must be empty after remove");
    }

    #[tokio::test]
    async fn remove_project_nonexistent_id_returns_not_found() {
        let (_config, mut registry) = make_registry();

        let err = registry.remove_project("no-such-id").await.unwrap_err();
        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // set_project_tags
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn set_project_tags_lowercases_trims_deduplicates() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        let tags = vec![
            "  Rust  ".to_string(),
            "rust".to_string(),
            "TYPESCRIPT".to_string(),
            "  typescript  ".to_string(),
            "  ".to_string(),
        ];
        let updated = registry.set_project_tags(&project.id, tags).await.unwrap();

        assert_eq!(updated.tags, vec!["rust", "typescript"]);
    }

    #[tokio::test]
    async fn set_project_tags_nonexistent_id_returns_not_found() {
        let (_config, mut registry) = make_registry();

        let err = registry
            .set_project_tags("no-such-id", vec!["tag".to_string()])
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    // FR-1.6.4 (T2.9): per-tag 32-character limit, enforced on the backend.

    #[tokio::test]
    async fn set_project_tags_rejects_tag_over_32_chars() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        let over_limit = "a".repeat(33);
        let err = registry
            .set_project_tags(&project.id, vec![over_limit])
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::InvalidInput(_)),
            "expected InvalidInput for tag > 32 chars, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn set_project_tags_accepts_tag_exactly_32_chars() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        let at_limit = "a".repeat(32);
        let updated = registry
            .set_project_tags(&project.id, vec![at_limit.clone()])
            .await
            .unwrap();
        assert_eq!(updated.tags, vec![at_limit]);
    }

    #[tokio::test]
    async fn set_project_tags_rejects_over_limit_in_mixed_list() {
        // Mixed valid + over-limit list must still reject (per-tag check,
        // not just first/single-element).
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        let err = registry
            .set_project_tags(&project.id, vec!["ok".to_string(), "a".repeat(33)])
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::InvalidInput(_)),
            "expected InvalidInput for mixed list with over-limit tag, got: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // is_missing (live check)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn list_projects_is_missing_true_when_path_deleted() {
        let (_config, mut registry) = make_registry();
        let (proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        assert!(!project.is_missing);

        // Drop the TempDir to delete the directory.
        drop(proj_dir);

        let list = registry.list_projects().await;
        assert_eq!(list.len(), 1);
        assert!(
            list[0].is_missing,
            "is_missing must be true after directory is deleted"
        );
    }

    // -----------------------------------------------------------------------
    // relocate_project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn relocate_project_updates_path() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir_a, proj_path_a) = make_project_dir();
        let (_proj_dir_b, proj_path_b) = make_project_dir();

        let project = registry.add_project(proj_path_a).await.unwrap();
        let canonical_b = proj_path_b.canonicalize().unwrap();
        let updated = registry
            .relocate_project(&project.id, proj_path_b)
            .await
            .unwrap();

        assert_eq!(updated.path, canonical_b);
        assert!(updated.last_modified.is_some());
    }

    #[tokio::test]
    async fn relocate_project_duplicate_new_path_returns_already_exists() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir_a, proj_path_a) = make_project_dir();
        let (_proj_dir_b, proj_path_b) = make_project_dir();

        registry.add_project(proj_path_a.clone()).await.unwrap();
        let proj_b = registry.add_project(proj_path_b.clone()).await.unwrap();

        // Try to relocate proj_b to proj_a's path (already taken).
        let err = registry
            .relocate_project(&proj_b.id, proj_path_a)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::AlreadyExists(_)),
            "expected AlreadyExists, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn relocate_project_nonexistent_id_returns_not_found() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let err = registry
            .relocate_project("no-such-id", proj_path)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // rename_project
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn rename_project_updates_name() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        let updated = registry
            .rename_project(&project.id, "My Project".to_string())
            .await
            .unwrap();

        assert_eq!(updated.name, "My Project");
        assert!(updated.last_modified.is_some());
    }

    #[tokio::test]
    async fn rename_project_nonexistent_id_returns_not_found() {
        let (_config, mut registry) = make_registry();

        let err = registry
            .rename_project("no-such-id", "New Name".to_string())
            .await
            .unwrap_err();
        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    // -----------------------------------------------------------------------
    // tag normalization edge cases (pure logic — no async needed)
    // -----------------------------------------------------------------------

    #[test]
    fn normalize_tags_empty_input_returns_empty() {
        let result = ProjectRegistry::normalize_tags(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn normalize_tags_whitespace_only_tags_dropped() {
        let tags = vec!["  ".to_string(), "\t".to_string()];
        let result = ProjectRegistry::normalize_tags(&tags);
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // T2.1 gap tests: rename_project empty string, is_missing not in JSON,
    // full round-trip field verification, case-insensitive dedup.
    // -----------------------------------------------------------------------

    /// `rename_project` with an empty string does NOT return an error — the
    /// implementation does not validate the name. This test documents the
    /// current behavior so that any future guard (e.g. InvalidInput on blank
    /// name) is a deliberate, visible breaking change.
    #[tokio::test]
    async fn rename_project_empty_string_succeeds_current_behavior() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path).await.unwrap();
        // The impl accepts an empty name — document this, not assert it is good.
        let result = registry.rename_project(&project.id, String::new()).await;
        assert!(
            result.is_ok(),
            "rename_project with empty string currently succeeds (no validation guard); \
             if this assertion fails the implementation now rejects empty names — update \
             this test and add a positive test for valid names"
        );
        let updated = result.unwrap();
        assert_eq!(updated.name, "", "name is stored as-is when empty");
    }

    /// `is_missing` is computed live and must NOT be serialised into the JSON
    /// file. After saving, the raw JSON text must not contain the key
    /// `"is_missing"`.
    #[tokio::test]
    async fn is_missing_not_written_to_json() {
        let config = TempDir::new().expect("config tempdir");
        let (_proj_dir, proj_path) = make_project_dir();

        let mut registry = ProjectRegistry::load(config.path());
        registry.add_project(proj_path).await.unwrap();

        // Read the raw JSON written by save().
        let json_path = config.path().join("projects.json");
        let json_text = std::fs::read_to_string(&json_path)
            .expect("projects.json must exist after add_project");

        assert!(
            !json_text.contains("\"is_missing\""),
            "projects.json must not contain the key \"is_missing\" (it is computed on read); \
             found in: {json_text}"
        );
    }

    /// Full round-trip: all `Project` fields survive `save()` → `load()`.
    ///
    /// The existing `add_project_persists_across_reload` test only checks the
    /// list length.  This test verifies that `id`, `name`, `path`, `tags`,
    /// `language`, `package_manager`, `added_at`, and `last_modified` all
    /// deserialize to their original values.
    #[tokio::test]
    async fn round_trip_all_project_fields_survive_save_and_reload() {
        let config = TempDir::new().expect("config tempdir");
        let (_proj_dir, proj_path) = make_project_dir();

        let (original_id, original_name, original_tags, original_added_at) = {
            let mut registry = ProjectRegistry::load(config.path());
            let proj = registry.add_project(proj_path.clone()).await.unwrap();
            let id = proj.id.clone();
            let name = proj.name.clone();
            let added_at = proj.added_at;

            // Set tags and trigger last_modified via set_project_tags.
            let updated = registry
                .set_project_tags(&id, vec!["rust".to_string(), "tauri".to_string()])
                .await
                .unwrap();

            (id, name, updated.tags.clone(), added_at)
        };

        // Reload from disk.
        let registry2 = ProjectRegistry::load(config.path());
        let list = registry2.list_projects().await;
        assert_eq!(list.len(), 1, "exactly one project must survive reload");

        let p = &list[0];
        assert_eq!(p.id, original_id, "id must survive round-trip");
        assert_eq!(p.name, original_name, "name must survive round-trip");
        assert_eq!(p.tags, original_tags, "tags must survive round-trip");
        assert_eq!(
            p.added_at, original_added_at,
            "added_at must survive round-trip"
        );
        assert!(
            p.last_modified.is_some(),
            "last_modified must be Some after set_project_tags"
        );
        assert_eq!(p.language, None, "language (None) must survive round-trip");
        assert_eq!(
            p.package_manager, None,
            "package_manager (None) must survive round-trip"
        );
        // is_missing is computed live; after reload the directory still exists.
        assert!(
            !p.is_missing,
            "is_missing must be false after reload while dir still exists"
        );
    }

    /// `normalize_tags` deduplication is case-insensitive: "Rust" and "rust"
    /// are the same tag; only the first occurrence (lowercased) is kept.
    ///
    /// This is a pure logic test — no async needed.
    #[test]
    fn normalize_tags_case_insensitive_dedup_first_occurrence_wins() {
        // "Rust" arrives first, "rust" arrives second — result must be ["rust"]
        // (the first occurrence lowercased, the duplicate dropped).
        let tags = vec!["Rust".to_string(), "rust".to_string()];
        let result = ProjectRegistry::normalize_tags(&tags);
        assert_eq!(
            result,
            vec!["rust"],
            "case variants must deduplicate to single entry"
        );

        // "TYPESCRIPT" first, then mixed-case duplicate.
        let tags2 = vec![
            "TYPESCRIPT".to_string(),
            "TypeScript".to_string(),
            "typescript".to_string(),
        ];
        let result2 = ProjectRegistry::normalize_tags(&tags2);
        assert_eq!(result2, vec!["typescript"]);
    }

    /// `relocate_project` to a path that is the canonicalized path of a
    /// *different* project must return `AlreadyExists`.  If the same project
    /// is relocated to its own current path the call succeeds (no false
    /// positive).  The former is already tested by
    /// `relocate_project_duplicate_new_path_returns_already_exists`; this
    /// companion test verifies the self-relocation no-false-positive branch.
    #[tokio::test]
    async fn relocate_project_to_own_current_path_succeeds() {
        let (_config, mut registry) = make_registry();
        let (_proj_dir, proj_path) = make_project_dir();

        let project = registry.add_project(proj_path.clone()).await.unwrap();
        // Relocating to the same canonical path (same id) must not be rejected.
        let result = registry.relocate_project(&project.id, proj_path).await;
        assert!(
            result.is_ok(),
            "relocate_project to the same path for the same project must succeed (not AlreadyExists)"
        );
    }
}
