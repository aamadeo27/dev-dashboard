# IPC: Projects

All commands return `Result<T, AppError>`. `AppError` is a typed enum serialized as `{ code: string, message: string, details?: any }`.

```rust
list_projects() -> Vec<Project>
add_project(path: PathBuf) -> Project
remove_project(id: String) -> ()
relocate_project(id: String, new_path: PathBuf) -> Project
set_project_tags(id: String, tags: Vec<String>) -> Project
rename_project(id: String, name: String) -> Project
get_git_status(id: String) -> GitStatus
refresh_git_status(id: String) -> GitStatus     // forces immediate poll
set_visible_projects(ids: Vec<String>) -> ()    // GitPoller visible-set update (debounced)
open_in_editor(id: String) -> ()
open_in_terminal(id: String) -> ()
```

> `rename_project` and `delete_run` are **v1: internal-only. Not exposed via `commands.ts` frontend wrappers. No UI entry point in v1.** They are kept in the Rust surface to support future UI work without a contract change. (`delete_run` is defined in ipc-runs.md.)

`GitStatus`:

```rust
struct GitStatus {
    branch: Option<String>,
    is_clean: bool,
    dirty_files: u32,
    ahead: u32,
    behind: u32,
    last_polled: DateTime<Utc>,
    error: Option<String>,
}
```
