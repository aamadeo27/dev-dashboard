# Build tooling

- **Vite** for the web side. Dev server only used in `tauri dev`.
- **Cargo** for the Rust side, workspace not needed (single crate sufficient at this size; can split later if `RunManager` grows).
- **pnpm** for JS package management (faster than npm, deterministic).
- **Biome** for JS/TS lint + format (single tool, no ESLint+Prettier config churn).
- **rustfmt + clippy** on Rust, gated in CI/local pre-commit.
