# Sequence: Project bootstrap

Goal: stand up the project skeleton — layout, boilerplate, dev infra — so feature work can begin.

## When to use
- After pre-project definition.
- No code exists yet, or only stub.

## Inputs
- Requirements, UI/UX, Knowledge Base, Epics/Tasks from pre-project sequence.

## Steps

1. **gf_devops-engineer** → confirms dev infra plan (repo layout, local dev environment, dev-only services, CI skeleton)
2. **coder** → creates GitHub remote and minimal passing CI before any code is written:
   - Adds `.gitignore` aligned with the stack per DevOps plan (build artifacts, `node_modules/`, `.env.local`, secrets, OS junk, `.claude/`)
   - Writes `.github/workflows/ci.yml`: single-job stub (`run: echo "CI placeholder"`) triggered on `push` and `pull_request` — passes immediately with no code or tests present
   - Commit: `chore: initialize repo`
   - Ask user: repo name (default = directory name), visibility (**always ask** — `public` or `private`; note branch protection on private repos requires a paid GitHub plan), org (optional)
   - `gh repo create <name> [--public|--private] [--org <org>] --source . --remote origin --push`
3. **coder** → scaffolds project per Knowledge Base (folders, package manifests, base configs, lint/format, base routing, base styles, base test setup)
4. **coder** → replaces the CI stub with the real pipeline per DevOps plan (lint, build, test stages; no deploy yet)
5. **coder** → sets up dev-only infra locally (docker-compose, dev DB, env vars from `.env.local`)
6. **tester** → verifies test framework runs (one smoke test per level — unit, medium, e2e)
7. **monitor** → wires local logging + error tracking sink (cheap/free tier; dev env only)
8. **Review + fix loop** (see `11-review-fix-loop.md`)
9. **coder** → final commit:
   - Stages everything except ignored paths
   - Commit: `chore: project bootstrap`
   - Push to `origin`

## Output
- GitHub remote exists; pre-project and bootstrap commits pushed
- Runnable, empty-but-correct project skeleton
- CI runs green (real pipeline replacing the stub)
- Local dev environment works end-to-end
- Smoke tests pass at all 3 levels

## Done when
- A new dev can clone, run install, start app locally in one command sequence.
- CI green on a no-op commit.
- `git log` shows at minimum the initialize and bootstrap commits; working tree is clean.
