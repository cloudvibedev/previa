import { create } from "zustand";

interface AutoScrollState {
  enabled: boolean;
  setEnabled: (enabled: boolean) => void;
}

const STORAGE_KEY = "previa-auto-scroll-steps";

export const useAutoScrollStore = create<AutoScrollState>((set) => ({
  enabled: localStorage.getItem(STORAGE_KEY) !== "false",
  setEnabled: (enabled) => {
    localStorage.setItem(STORAGE_KEY, String(enabled));
    set({ enabled });
  },
}));
