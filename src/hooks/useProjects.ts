import { useQuery } from "@tanstack/react-query";
import { listProjects } from "../ipc/commands";

export const PROJECTS_QUERY_KEY = ["projects"] as const;

export function useProjects() {
  const { data, isLoading, error, refetch } = useQuery({
    queryKey: PROJECTS_QUERY_KEY,
    queryFn: listProjects,
  });
  return { projects: data ?? [], isLoading, error: error as Error | null, refetch };
}
