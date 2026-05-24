# IPC: Events (Rust -> frontend)

| Event name        | Payload                                  | Frequency                       |
|---|---|---|
| `run:started`     | `{ run_id, project_id }`                 | once per run                    |
| `run:event`       | `{ run_id, event: RunEvent }`            | per parsed event                |
| `run:finished`    | `{ run_id, status, exit_code }`          | once per run                    |
| `run:step_failure`| `{ run_id, step, message }`              | when a step fails               |
| `git:updated`     | `{ project_id, status: GitStatus }`      | per poll cycle / on focus       |
| `project:missing` | `{ project_id }`                         | when scan detects missing dir   |
| `usage:updated`   | `{ snapshot: UsageSnapshot }`            | every 60s and on refresh        |
| `cli:lost`        | `{ error: string }`                      | when CLI disappears mid-session |
| `toast:show`      | `{ kind, title, body, run_id? }`         | run terminal events             |

Event names are constants in `src-tauri/src/ipc/events.rs` and `src/ipc/events.ts` — never inline strings.
