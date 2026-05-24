// UI state store (Zustand). See KB §2.3.
import { create } from "zustand";
import type { ViewMode } from "../ipc/bindings";

interface UiState {
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
}

export const useUiStore = create<UiState>((set) => ({
  viewMode: "Grid",
  setViewMode: (mode) => set({ viewMode: mode }),
}));
