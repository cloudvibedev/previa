import { create } from "zustand";

export type StepViewMode = "graph" | "list";

interface StepViewState {
  mode: StepViewMode;
  setMode: (mode: StepViewMode) => void;
}

const STORAGE_KEY = "previa-step-view-mode";

export const useStepViewStore = create<StepViewState>((set) => ({
  mode: (localStorage.getItem(STORAGE_KEY) as StepViewMode) || "graph",
  setMode: (mode) => {
    localStorage.setItem(STORAGE_KEY, mode);
    set({ mode });
  },
}));
