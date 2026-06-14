// useRunHistory hook — wraps listRuns IPC command via TanStack Query.
// Returns runs newest-first; data is undefined while loading, [] when project has no runs.
import { useQuery } from "@tanstack/react-query";
import type { Run } from "../ipc/bindings";
import { listRuns } from "../ipc/commands";

export const RUN_HISTORY_QUERY_KEY = (projectId: string) => ["runs", projectId] as const;

export function useRunHistory(projectId: string) {
  return useQuery<Run[]>({
    queryKey: RUN_HISTORY_QUERY_KEY(projectId),
    queryFn: () => listRuns(projectId),
    enabled: !!projectId,
  });
}

export type { Run };
