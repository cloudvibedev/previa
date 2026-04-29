import { create } from "zustand";
import {
  getContexts,
  saveContexts,
  getActiveContextId,
  setActiveContextId,
  getActiveContext,
  type OrchestratorContext,
} from "@/lib/orchestrator-url";
import { generateUUID } from "@/lib/uuid";

export type { OrchestratorContext } from "@/lib/orchestrator-url";

export interface OrchestratorInfo {
  context: string;
  totalRunners: number;
  activeRunners: number;
}

interface OrchestratorState {
  contexts: OrchestratorContext[];
  activeContextId: string | null;
  info: OrchestratorInfo | null;

  /** Convenience: derived active context */
  activeContext: OrchestratorContext | null;

  /** Legacy compat: returns active context URL or null */
  url: string | null;

  addContext: (name: string, url: string) => OrchestratorContext;
  removeContext: (id: string) => void;
  updateContext: (id: string, updates: Partial<Omit<OrchestratorContext, "id">>) => void;
  switchContext: (id: string) => void;
  setInfo: (info: OrchestratorInfo | null) => void;
  fetchInfo: () => Promise<OrchestratorInfo | null>;

  // Legacy compat
  setUrl: (url: string | null) => void;
}

function deriveActive(contexts: OrchestratorContext[], activeId: string | null) {
  if (!activeId) return contexts[0] || null;
  return contexts.find((c) => c.id === activeId) || contexts[0] || null;
}

export const useOrchestratorStore = create<OrchestratorState>((set, get) => {
  const initialContexts = getContexts();
  const initialActiveId = getActiveContextId();
  const initialActive = deriveActive(initialContexts, initialActiveId);

  return {
    contexts: initialContexts,
    activeContextId: initialActiveId,
    activeContext: initialActive,
    url: initialActive?.url || null,
    info: null,

    addContext: (name, url) => {
      const ctx: OrchestratorContext = { id: generateUUID(), name, url: url.replace(/\/+$/, "") };
      const updated = [...get().contexts, ctx];
      saveContexts(updated);
      // If first context, auto-activate
      if (updated.length === 1) {
        setActiveContextId(ctx.id);
        const active = ctx;
        set({ contexts: updated, activeContextId: ctx.id, activeContext: active, url: active.url });
        get().fetchInfo();
      } else {
        set({ contexts: updated });
      }
      return ctx;
    },

    removeContext: (id) => {
      const updated = get().contexts.filter((c) => c.id !== id);
      saveContexts(updated);
      const wasActive = get().activeContextId === id;
      if (wasActive) {
        const newActive = updated[0] || null;
        setActiveContextId(newActive?.id || null);
        set({ contexts: updated, activeContextId: newActive?.id || null, activeContext: newActive, url: newActive?.url || null, info: null });
        if (newActive) get().fetchInfo();
      } else {
        set({ contexts: updated });
      }
    },

    updateContext: (id, updates) => {
      const updated = get().contexts.map((c) =>
        c.id === id ? { ...c, ...updates, url: updates.url ? updates.url.replace(/\/+$/, "") : c.url } : c
      );
      saveContexts(updated);
      const active = deriveActive(updated, get().activeContextId);
      set({ contexts: updated, activeContext: active, url: active?.url || null });
    },

    switchContext: (id) => {
      setActiveContextId(id);
      const active = deriveActive(get().contexts, id);
      set({ activeContextId: id, activeContext: active, url: active?.url || null, info: null });
      if (active) get().fetchInfo();
    },

    setInfo: (info) => set({ info }),

    fetchInfo: async () => {
      const active = get().activeContext;
      if (!active) {
        set({ info: null });
        return null;
      }
      try {
        const base = active.url.replace(/\/api\/v1\/?$/, "").replace(/\/+$/, "");
        const res = await fetch(`${base}/info`, { signal: AbortSignal.timeout(8000) });
        if (!res.ok) {
          set({ info: null });
          return null;
        }
        const data = await res.json();
        const info: OrchestratorInfo = {
          context: data.context,
          totalRunners: data.totalRunners,
          activeRunners: data.activeRunners,
        };
        set({ info });
        return info;
      } catch {
        set({ info: null });
        return null;
      }
    },

    // Legacy compat
    setUrl: (url) => {
      if (!url) {
        // Clear active
        setActiveContextId(null);
        set({ activeContextId: null, activeContext: null, url: null, info: null });
        return;
      }
      // Find or create context with this URL
      const existing = get().contexts.find((c) => c.url === url.replace(/\/+$/, ""));
      if (existing) {
        get().switchContext(existing.id);
      } else {
        const ctx = get().addContext(url, url);
        get().switchContext(ctx.id);
      }
    },
  };
});

/** Returns the base API URL (with /api/v1) or null if no backend is configured. Safe to call outside React. */
export function getApiUrl(): string | null {
  const active = useOrchestratorStore.getState().activeContext;
  if (!active) return null;
  const url = active.url;
  return url.endsWith("/api/v1") ? url : `${url}/api/v1`;
}
