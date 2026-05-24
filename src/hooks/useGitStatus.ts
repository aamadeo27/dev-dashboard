import { useEffect, useRef } from "react";
import { create } from "zustand";
import type { GitStatus } from "../ipc/bindings";
import { setVisibleProjects } from "../ipc/commands";
import { GIT_UPDATED, subscribe } from "../ipc/events";

interface GitStatusState {
  statuses: Record<string, GitStatus>;
  setStatus: (id: string, status: GitStatus) => void;
}

const useGitStatusStore = create<GitStatusState>((set) => ({
  statuses: {},
  setStatus: (id, status) => set((s) => ({ statuses: { ...s.statuses, [id]: status } })),
}));

export function useGitStatusListener(): void {
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let active = true;
    subscribe<{ id: string; status: GitStatus }>(GIT_UPDATED, ({ id, status }) => {
      useGitStatusStore.getState().setStatus(id, status);
    }).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);
}

export function useGitStatus(id: string): GitStatus | null {
  return useGitStatusStore((s) => s.statuses[id] ?? null);
}

export function useVisibleProjects(ids: string[]): void {
  const idsRef = useRef(ids);
  useEffect(() => {
    idsRef.current = ids;
  }, [ids]);

  const idsKey = ids.join(",");
  // biome-ignore lint/correctness/useExhaustiveDependencies: idsKey is a sentinel trigger derived from ids; callback reads idsRef.current for the latest value
  useEffect(() => {
    const t = setTimeout(() => setVisibleProjects(idsRef.current), 300);
    return () => clearTimeout(t);
  }, [idsKey]);

  useEffect(() => {
    const onBlur = () => setVisibleProjects([]);
    const onFocus = () => setVisibleProjects(idsRef.current);
    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
    };
  }, []);
}
