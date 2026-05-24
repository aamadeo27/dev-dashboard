# `Run`

```rust
struct Run {
    id: String,                  // UUID v7
    project_id: String,
    project_path: PathBuf,       // captured at launch time (project may move)
    sequence_name: String,
    attached_md_path: Option<PathBuf>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    status: RunStatus,           // pending|running|completed|failed|stopped
    exit_code: Option<i32>,
    pid: Option<u32>,
    note: Option<String>,        // e.g. "Terminated (app restarted)"
}

enum RunStatus { Pending, Running, Completed, Failed, Stopped }
```

**Storage**: `<project>/.claude/runs/<run-id>/meta.json`. Written on state change.
