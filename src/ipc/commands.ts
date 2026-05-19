// Typed wrappers for Tauri commands. See KB §5. Additional wrappers added per task.
import { invoke } from "@tauri-apps/api/core";

export function logFrontendError(message: string, stack?: string, route?: string): void {
  invoke("log_frontend_error", { message, stack, route }).catch(() => {});
}
