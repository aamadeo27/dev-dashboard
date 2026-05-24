# `UsageSnapshot`

```rust
struct UsageSnapshot {
    fetched_at: DateTime<Utc>,
    parsed: BTreeMap<String, String>, // ordered key-value parse of `claude /usage` stdout
    raw_stdout: String,
    available: bool,                  // false if subprocess failed
}
```

**Storage**: in-memory only. Re-fetched on app start; not persisted.
