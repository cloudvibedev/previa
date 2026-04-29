import { create } from "zustand";
import { persist } from "zustand/middleware";

type ChatPosition = "right" | "left";

interface ChatPositionState {
  position: ChatPosition;
  collapsed: boolean;
  setPosition: (position: ChatPosition) => void;
  toggleCollapsed: () => void;
}

export const useChatPositionStore = create<ChatPositionState>()(
  persist(
    (set) => ({
      position: "right",
      collapsed: false,
      setPosition: (position) => set({ position }),
      toggleCollapsed: () => set((s) => ({ collapsed: !s.collapsed })),
    }),
    { name: "previa:chat-position" }
  )
);
