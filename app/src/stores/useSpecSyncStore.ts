import { create } from "zustand";
import { checkForUpdate } from "@/lib/spec-sync";
import { upsertSpec, type ProjectSpecUpsertRequest } from "@/lib/api-client";
import { getApiUrl } from "@/stores/useOrchestratorStore";
import { useProjectStore } from "@/stores/useProjectStore";
import type { ProjectSpec } from "@/types/project";

export interface SyncState {
  url: string;
  hash: string; // specMd5 from backend
  enabled: boolean;
  newContent?: string;
  newHash?: string;
}

/** Build a PUT payload from the current project spec, overriding sync fields. */
function buildPayload(
  spec: ProjectSpec,
  overrides: { sync: boolean; url?: string }
): ProjectSpecUpsertRequest {
  return {
    spec: (spec.spec as any)?.raw ?? spec.spec,
    sync: overrides.sync,
    url: overrides.url ?? spec.url ?? null,
    slug: spec.slug ?? null,
    servers: spec.servers ?? null,
  };
}

function findSpec(projectId: string, specId: string): ProjectSpec | undefined {
  const project = useProjectStore.getState().currentProject;
  if (!project || project.id !== projectId) return undefined;
  return project.specs.find((s) => s.id === specId);
}

interface SpecSyncStore {
  syncs: Record<string, SyncState>;
  enableSync: (projectId: string, specId: string, url: string, specMd5: string) => void;
  disableSync: (projectId: string, specId: string) => void;
  checkAll: () => Promise<void>;
  acceptUpdate: (projectId: string, specId: string) => { newContent: string; newHash: string } | null;
  dismissUpdate: (projectId: string, specId: string) => void;
  /** Hydrate from project specs (replaces localStorage hydration). */
  hydrate: (specs: ProjectSpec[]) => void;
}

export const useSpecSyncStore = create<SpecSyncStore>((set, get) => ({
  syncs: {},

  enableSync: (projectId, specId, url, specMd5) => {
    const entry: SyncState = { url, hash: specMd5, enabled: true };
    set((state) => ({
      syncs: { ...state.syncs, [specId]: { ...entry } },
    }));

    // Persist to backend (fire-and-forget)
    const baseUrl = getApiUrl();
    const spec = findSpec(projectId, specId);
    if (baseUrl && spec) {
      upsertSpec(baseUrl, projectId, specId, buildPayload(spec, { sync: true, url })).catch(
        (err) => console.warn("[SpecSync] Failed to enable sync on backend:", err)
      );
    }
  },

  disableSync: (projectId, specId) => {
    set((state) => {
      const { [specId]: _, ...rest } = state.syncs;
      return { syncs: rest };
    });

    const baseUrl = getApiUrl();
    const spec = findSpec(projectId, specId);
    if (baseUrl && spec) {
      upsertSpec(baseUrl, projectId, specId, buildPayload(spec, { sync: false })).catch(
        (err) => console.warn("[SpecSync] Failed to disable sync on backend:", err)
      );
    }
  },

  checkAll: async () => {
    const { syncs } = get();
    const updates: Record<string, SyncState> = {};
    let hasChange = false;

    await Promise.all(
      Object.entries(syncs).map(async ([specId, sync]) => {
        if (!sync.enabled) return;
        const result = await checkForUpdate(sync.url, sync.hash);
        if (result.changed && result.newContent && result.newHash) {
          updates[specId] = {
            ...sync,
            newContent: result.newContent,
            newHash: result.newHash,
          };
          hasChange = true;
        }
      })
    );

    if (hasChange) {
      set((state) => ({
        syncs: { ...state.syncs, ...updates },
      }));
    }
  },

  acceptUpdate: (projectId, specId) => {
    const sync = get().syncs[specId];
    if (!sync?.newContent || !sync?.newHash) return null;

    const { newContent, newHash } = sync;
    const updatedEntry: SyncState = { url: sync.url, hash: newHash, enabled: true };
    set((state) => ({
      syncs: {
        ...state.syncs,
        [specId]: { ...updatedEntry, newContent: undefined, newHash: undefined },
      },
    }));

    // Persist updated hash to backend
    const baseUrl = getApiUrl();
    const spec = findSpec(projectId, specId);
    if (baseUrl && spec) {
      upsertSpec(baseUrl, projectId, specId, buildPayload(spec, { sync: true })).catch(
        (err) => console.warn("[SpecSync] Failed to persist accepted update:", err)
      );
    }

    return { newContent, newHash };
  },

  dismissUpdate: (projectId, specId) => {
    set((state) => {
      const { [specId]: _, ...rest } = state.syncs;
      return { syncs: rest };
    });

    const baseUrl = getApiUrl();
    const spec = findSpec(projectId, specId);
    if (baseUrl && spec) {
      upsertSpec(baseUrl, projectId, specId, buildPayload(spec, { sync: false })).catch(
        (err) => console.warn("[SpecSync] Failed to dismiss sync on backend:", err)
      );
    }
  },

  hydrate: (specs: ProjectSpec[]) => {
    const syncs: Record<string, SyncState> = {};
    for (const spec of specs) {
      if (spec.sync && spec.url) {
        syncs[spec.id] = {
          url: spec.url,
          hash: spec.specMd5 ?? "",
          enabled: true,
        };
      }
    }
    set({ syncs });
  },
}));
