# Sequence: Project bootstrap

Goal: stand up the project skeleton — layout, boilerplate, dev infra — so feature work can begin.

## When to use
- After pre-project definition.
- No code exists yet, or only stub.

## Inputs
- Requirements, UI/UX, Knowledge Base, Epics/Tasks from pre-project sequence.

## Steps

1. **gf_devops-engineer** → confirms dev infra plan (repo layout, local dev environment, dev-only services, CI skeleton)
2. **coder** → scaffolds project per Knowledge Base (folders, package manifests, base configs, lint/format, base routing, base styles, base test setup)
3. **coder** → wires CI pipeline skeleton per DevOps plan (lint, build, test stages; no deploy yet)
4. **coder** → sets up dev-only infra locally (docker-compose, dev DB, env vars from `.env.local`)
5. **tester** → verifies test framework runs (one smoke test per level — unit, medium, e2e)
6. **monitor** → wires local logging + error tracking sink (cheap/free tier; dev env only)
7. **Review + fix loop** (see `11-review-fix-loop.md`)
8. **coder** → initializes a git repo at the project root:
   - `git init`
   - Adds a `.gitignore` aligned with the stack (build artifacts, `node_modules/`, `.env.local`, secrets, OS junk)
   - Stages everything except ignored paths
   - First commit: `chore: project bootstrap`
   - Follows the branching/PR pattern from the Knowledge Base (default branch name, etc.)
   - Does **not** add or push a remote — that is the user's choice

## Output
- Runnable, empty-but-correct project skeleton
- CI runs green
- Local dev environment works end-to-end
- Smoke tests pass at all 3 levels
- Git repo initialized with first commit

## Done when
- A new dev can clone, run install, start app locally in one command sequence.
- CI green on a no-op commit.
- `git log` shows the bootstrap commit; working tree is clean.
