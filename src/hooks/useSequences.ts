// useSequences hook — wraps listSequences IPC command via TanStack Query.
// See docs/tasks/T3.2.md.
import { useQuery } from "@tanstack/react-query";
import type { Sequence } from "../ipc/bindings";
import { listSequences } from "../ipc/commands";

export const SEQUENCES_QUERY_KEY = (projectId: string) => ["sequences", projectId] as const;

export function useSequences(projectId: string) {
  return useQuery({
    queryKey: SEQUENCES_QUERY_KEY(projectId),
    queryFn: () => listSequences(projectId),
    enabled: !!projectId,
  });
}

export type { Sequence };
