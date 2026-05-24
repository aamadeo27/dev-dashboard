# `Settings`

```rust
struct Settings {
    parent_dir: Option<PathBuf>,     // GAP-07: single configured parent dir
    claude_cli_path: Option<PathBuf>,// overrides PATH lookup
    git_poll_interval_secs: u32,     // default 10, min 5, max 3600
    usage_poll_interval_secs: u32,   // default 60, min 30, max 3600
    retention_days: u32,             // default 30, min 1
    retention_size_mb: u32,          // default 500, min 50
    view_mode: ViewMode,             // Grid | List
}
```

**Storage**: `<os_config_dir>/dev-dashboard/settings.json`.
