# `Project`

All entities are defined in Rust and exported to TS via `ts-rs`. The Rust struct is the source of truth.

```rust
struct Project {
    id: String,             // UUID v7
    name: String,           // basename of path, user-editable
    path: PathBuf,          // absolute, canonicalized
    tags: Vec<String>,      // lowercased, trimmed, deduped
    language: Option<String>,    // detected: "rust", "ts", "python", ...
    package_manager: Option<String>, // "cargo", "pnpm", "npm", "uv", ...
    added_at: DateTime<Utc>,
    last_modified: Option<DateTime<Utc>>, // mtime of project root
    is_missing: bool,       // computed; not persisted
}
```

**Storage**: `<os_config_dir>/dev-dashboard/projects.json`.
