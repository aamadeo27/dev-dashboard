# File encoding and line endings

- All transcripts, logs, settings, registry: **UTF-8, no BOM, LF line endings**, regardless of platform.
- Sequence `.md` files: read as UTF-8; fall back to UTF-8 lossy if invalid bytes (log a warning).
- Paths: `PathBuf` in Rust, `string` (absolute) in TS. Never relative across the IPC boundary.
