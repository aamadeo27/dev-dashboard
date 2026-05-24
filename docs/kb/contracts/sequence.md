# `Sequence`

```rust
struct Sequence {
    name: String,        // filename minus ".md"
    description: String, // first non-heading paragraph; "(No description)" fallback
    path: PathBuf,       // absolute path to the .md file
    mtime: DateTime<Utc>,// for cache invalidation
}
```

**Storage**: filesystem only at `<project>/.claude/sequences/*.md`. Loaded on demand, cached in-memory keyed by project_id.
