# Conventions

- [File Layout (Codebase)](file-layout.md) — full directory tree for src-tauri/ and src/ with Rust crate layout notes
- [File encoding and line endings](file-encoding-and-line-endings.md) — UTF-8 no BOM, LF everywhere; paths always absolute across IPC
- [Naming](naming.md) — snake_case Rust, camelCase TS, PascalCase components, kebab-case non-component files
- [Testing approach](testing-approach.md) — inline Rust unit tests, Vitest + RTL for frontend, E2E deferred to v1.1
- [Branching and PR Pattern](branching-and-pr-pattern.md) — feat/<task-id>-slug branches, squash-merge to main, Conventional Commits
- [Secrets](secrets.md) — this repo has zero secrets; no .env, no credentials, no tokens anywhere
