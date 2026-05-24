# Tauri

- **Tauri 2.x** (latest stable as of 2026-05). v2 has stable cross-platform notification, FS, dialog, and shell plugins; sidecar/process management is well-supported.
- Why Tauri (vs Electron): smaller footprint (NFR-6: <200 MB RAM idle), native FS and process control from Rust, no bundled Chromium — matches NFR-2 (no exposed local server beyond loopback IPC).
