# Refactor Plan — Dev Dashboard adoption

**Status**: PENDING USER APPROVAL (executed in step 6, not before).
Destructive moves of `.claude/*.md` originals require explicit sign-off.

| # | Source | Target | Action | Notes |
|---|--------|--------|--------|-------|
| 1 | `.claude/requirements.md` | `docs/requirements.md` | rewrite+archive src | Canonical written by gf_requirement-engineer (step 3, done). Archive original. |
| 2 | `.claude/ui-ux-spec.md` | `docs/ui-ux.md` | rewrite+archive src | Canonical written by gf_ui-ux-designer (step 4, done). Archive original. |
| 3 | `.claude/monitoring.md` | `docs/monitoring.md` | move/rewrite | monitor agent (step 8) confirms current-state; then relocate. |
| 4 | `.claude/devops.md` | `docs/devops.md` | move | gf_devops-engineer (step 7) audits; keep full plan at docs/devops.md. |
| 5 | `docs/kb.monolith.bak.20260524.md` | `docs/_archive/` | archive | pre-split backup |
| 6 | `docs/kb/{contracts,conventions,patterns,tech-stack}.bak.20260524.md` | `docs/_archive/kb/` | archive | pre-itemize backups |
| 7 | `docs/T4.3-sec-fixes-2.md` | `docs/tasks/` | move | stray review artifact |
| 8 | `docs/epics/README.md`, `docs/kb/conventions/file-layout.md` | in place | rewrite refs | `requirements.md`→`docs/requirements.md`, `ui-ux-spec.md`→`docs/ui-ux.md` |
| 9 | `docs/kb/`, `docs/epics/` (canonical bodies) | in place | leave | already conform; architect validates in step 5 |
| 10 | `docs/_adoption/` | — | leave (or archive at end) | discovery report + refactor plan working files |

## Status bootstrap (step 9) deviations
- No `.claude/templates/` dir exists → synthesize `PROJECT_STATUS.md` from KB system-design + epics instead of copying a template.
- Hooks already present in `.claude/hooks/` (`update-status.sh`, `validate-task-footer.sh`); wire to `.git/hooks/` (post-merge, commit-msg). `.githooks/pre-commit` already active via `core.hooksPath`.
- PR template + validate-task-footer workflow: no template source; create only if user wants.
