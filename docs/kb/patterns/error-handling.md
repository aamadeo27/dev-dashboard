# Error handling

- One `AppError` enum in Rust with concrete variants:
  - `NotFound`, `AlreadyExists`, `Io`, `Git`, `Cli`, `ParseError`, `InvalidInput`, `PermissionDenied`, `Internal`.
- Each variant serializes to `{ code: "NOT_FOUND" | ..., message, details? }`.
- The frontend has a single error-translation utility (`utils/errors.ts`) that maps codes to user-facing strings. Components never display raw `error.message` directly — always through the translator.
- Background tasks (poller, pruner, parser) **never panic**. Errors are logged via `tracing` and surfaced as `system` events or toasts where user-relevant.
