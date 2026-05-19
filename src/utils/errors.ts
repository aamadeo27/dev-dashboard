// Maps Tauri IPC error codes/strings to user-facing messages.
// All IPC error handling should go through this module.

export function toUserMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "An unexpected error occurred.";
}
