import { create } from "zustand";

export interface AppEvent {
  id: string;
  uid?: string;
  timestamp: string;
  type: "error" | "warning" | "info" | "success";
  title: string;
  message: string;
  details?: {
    method?: string;
    url?: string;
    statusCode?: number;
  };
  action?: {
    label: string;
    onClick: () => void;
  };
  actionUrl?: string;
  actionLabel?: string;
}

interface EventState {
  events: AppEvent[];
  addEvent: (event: Omit<AppEvent, "id" | "timestamp">) => void;
  clearEvents: () => void;
  dismissEvent: (id: string) => void;
  dismissByUid: (uid: string) => void;
}

const MAX_EVENTS = 200;

export const useEventStore = create<EventState>((set) => ({
  events: [],
  addEvent: (event) =>
    set((state) => {
      const filtered = event.uid
        ? state.events.filter((e) => e.uid !== event.uid)
        : state.events;
      return {
        events: [
          {
            ...event,
            id: crypto.randomUUID(),
            timestamp: new Date().toISOString(),
          },
          ...filtered,
        ].slice(0, MAX_EVENTS),
      };
    }),
  clearEvents: () => set({ events: [] }),
  dismissEvent: (id) =>
    set((state) => ({
      events: state.events.filter((e) => e.id !== id),
    })),
  dismissByUid: (uid) =>
    set((state) => ({
      events: state.events.filter((e) => e.uid !== uid),
    })),
}));
