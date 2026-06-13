# Epics: Dev Dashboard v1

Index only. Each epic is a folder: `epic-*/DESCRIPTION.md` (overview + wave plan) plus one `T*.md` per task.
Cross-epic graph, parallelization tracks, and the requirement-coverage map live in [_planning.md](_planning.md).

| Epic | Goal | Status |
|---|---|---|
| [epic-0-bootstrap](epic-0-bootstrap/DESCRIPTION.md) | Project scaffold, IPC seam, tracing, CI/infra glue | done (T0.CI-1/3 open) |
| [epic-1-settings-cli](epic-1-settings-cli/DESCRIPTION.md) | Settings store + Claude CLI detection (S-01, S-07) | done |
| [epic-2-project-registry-git](epic-2-project-registry-git/DESCRIPTION.md) | Project registry, git status, tags (S-02) | done (T2.9 open) |
| [epic-3-sequences](epic-3-sequences/DESCRIPTION.md) | Sequence loading + listing | done |
| [epic-4-run-execution](epic-4-run-execution/DESCRIPTION.md) | Run core: parser, manager, transcript, orphan, retention, step-failure | in-progress |
| [epic-5-run-ui](epic-5-run-ui/DESCRIPTION.md) | Run UI live + historical (S-03/04/05/06) | planned |
| [epic-6-toasts](epic-6-toasts/DESCRIPTION.md) | Toasts on run terminal events (S-09) | planned |
| [epic-7-usage](epic-7-usage/DESCRIPTION.md) | Usage / rate-limit pill (FR-7) | planned |
| [epic-8-polish](epic-8-polish/DESCRIPTION.md) | Cross-cutting polish, a11y, NFR verification | planned |
| [epic-9-observability](epic-9-observability/DESCRIPTION.md) | Structured logging + in-app health signals | planned |
