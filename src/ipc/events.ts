import { type UnlistenFn, listen } from "@tauri-apps/api/event";

// Tauri event name constants. Backend counterparts are in src-tauri/src/ipc/events.rs.
export const CLI_LOST = "cli:lost" as const;

/** Event emitted by the backend to display a toast notification in the frontend.
 *  Payload shape: `{ kind: "error"|"warning"|"info"|"success", title: string, body: string }`.
 *  See KB §5.5 and src-tauri/src/ipc/events.rs TOAST_SHOW. */
export const TOAST_SHOW = "toast:show" as const;

/** Emitted after each git poll cycle for a single project.
 *  Payload shape: `{ id: string, status: GitStatus }`.
 *  Backend counterpart: `GIT_UPDATED` in src-tauri/src/ipc/events.rs. */
export const GIT_UPDATED = "git:updated" as const;

/** Emitted once when a run transitions from Pending to Running.
 *  Payload shape: `{ run_id: string, project_id: string }`.
 *  Backend counterpart: `RUN_STARTED` in src-tauri/src/ipc/events.rs. */
export const RUN_STARTED = "run:started" as const;

/** Emitted for each structured event parsed from the child's stdout.
 *  Payload shape: `{ run_id: string, event: RunEvent }`.
 *  Backend counterpart: `RUN_EVENT` in src-tauri/src/ipc/events.rs. */
export const RUN_EVENT = "run:event" as const;

/** Emitted once when a run reaches a terminal state (Completed, Failed, or Stopped).
 *  Payload shape: `{ run_id: string, status: RunStatus, exit_code: number | null }`.
 *  Backend counterpart: `RUN_FINISHED` in src-tauri/src/ipc/events.rs. */
export const RUN_FINISHED = "run:finished" as const;

export function subscribe<T>(
  eventName: string,
  handler: (payload: T) => void
): Promise<UnlistenFn> {
  return listen<T>(eventName, (event) => handler(event.payload));
}
