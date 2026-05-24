# Secrets

Full details in `.claude/devops.md` §1.

**This repo has no secrets.** The application manages zero credentials. The `claude` CLI handles its own authentication externally and independently of this codebase.

- No `.env` files, no secret manager, no credentials in CI.
- Nothing to inject at build time or runtime.
- No tokens, API keys, or passwords are stored, referenced, or passed through this app.
- Hard rules (apply even if this changes in future):
  - No secrets in git (not even gitignored `.env` with real values committed once).
  - No secrets in log output.
  - No secrets in client bundles or IPC payloads.
- Local dev story: not applicable — there is nothing to configure. Run `pnpm dev` and the app works.
