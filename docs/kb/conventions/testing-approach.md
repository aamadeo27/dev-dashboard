# Testing approach

- **Rust**: unit tests inline (`#[cfg(test)]`) for parser, retention, orphan detection, settings (de)serialization. Integration tests in `src-tauri/tests/` for filesystem-touching flows using a `tempdir`.
- **Frontend**: Vitest for utility functions and event-block components (snapshot + interaction). React Testing Library for components. Avoid testing TanStack Query plumbing — trust the lib.
- **End-to-end**: deferred until v1.1. The Tauri E2E story (WebDriver) is fragile; manual smoke checklist suffices for v1.
