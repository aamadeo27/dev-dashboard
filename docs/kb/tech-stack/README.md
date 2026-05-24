# Tech Stack

- [Tauri](tauri.md) — Tauri 2.x desktop framework; chosen over Electron for RAM footprint and native Rust process control
- [Frontend: React + TypeScript + Vite](frontend-react-ts-vite.md) — React 18 + TS + Vite; largest training corpus, component model matches UI spec
- [State management](state-management.md) — Zustand for client state, TanStack Query for server state, Zustand store per live run
- [IPC pattern](ipc-pattern.md) — commands (request/response) + events (push from Rust); event names always constants, never inline strings
- [Build tooling](build-tooling.md) — Vite, Cargo, pnpm, Biome, rustfmt+clippy
- [Key Rust crates](rust-crates.md) — tauri, tokio, git2, serde, uuid, tracing, ts-rs, sysinfo; plugin-opener not plugin-shell
- [Key JS packages](js-packages.md) — react, @tauri-apps/api, zustand, react-query, react-router-dom, lucide-react, react-markdown, diff2html
