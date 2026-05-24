// Hook for reading and updating Settings via TanStack Query. See KB §4.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { SettingsPatch } from "../ipc/bindings";
import { getSettings, updateSettings } from "../ipc/commands";

export const SETTINGS_QUERY_KEY = ["settings"] as const;

export function useSettings() {
  const queryClient = useQueryClient();

  const query = useQuery({
    queryKey: SETTINGS_QUERY_KEY,
    queryFn: getSettings,
  });

  const mutation = useMutation({
    mutationFn: (patch: SettingsPatch) => updateSettings(patch),
    onSuccess: (updated) => {
      queryClient.setQueryData(SETTINGS_QUERY_KEY, updated);
    },
  });

  return {
    settings: query.data,
    isLoading: query.isLoading,
    error: query.error,
    updateSettings: mutation.mutateAsync,
    isSaving: mutation.isPending,
  };
}
