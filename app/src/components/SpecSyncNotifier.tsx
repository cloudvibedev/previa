import { useEffect, useRef } from "react";
import { useSpecSyncStore } from "@/stores/useSpecSyncStore";
import { useProjectStore } from "@/stores/useProjectStore";
import { useEventStore } from "@/stores/useEventStore";
import i18n from "@/i18n";

const POLL_INTERVAL = 30_000;

export function SpecSyncNotifier() {
  const syncs = useSpecSyncStore((s) => s.syncs);
  const checkAll = useSpecSyncStore((s) => s.checkAll);
  const hydrate = useSpecSyncStore((s) => s.hydrate);
  const { currentProject } = useProjectStore();

  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const notifiedRef = useRef<Set<string>>(new Set());

  // Hydrate from project specs whenever currentProject changes
  useEffect(() => {
    if (currentProject?.specs) {
      hydrate(currentProject.specs);
    }
  }, [currentProject?.specs, hydrate]);

  // Polling
  useEffect(() => {
    const hasSyncs = Object.keys(syncs).length > 0;
    if (!hasSyncs) {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      return;
    }

    checkAll();

    intervalRef.current = setInterval(() => {
      checkAll();
    }, POLL_INTERVAL);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [Object.keys(syncs).join(","), checkAll]);

  // Show toasts/events for specs with pending updates
  useEffect(() => {
    console.log("[SpecSyncNotifier] effect triggered", {
      currentProject: currentProject?.id ?? null,
      syncsKeys: Object.keys(syncs),
      alreadyNotified: [...notifiedRef.current],
    });

    if (!currentProject) {
      console.log("[SpecSyncNotifier] currentProject not loaded yet — skipping");
      return;
    }

    for (const [specId, sync] of Object.entries(syncs)) {
      console.log(`[SpecSyncNotifier] checking specId=${specId}`, {
        hasNewContent: !!sync.newContent,
        alreadyNotified: notifiedRef.current.has(specId),
        newHash: sync.newHash,
      });

      if (sync.newContent && !notifiedRef.current.has(specId)) {
        notifiedRef.current.add(specId);
        const spec = currentProject.specs.find((s) => s.id === specId);
        const name = spec?.name || specId;
        const projectId = currentProject.id;
        const diffUrl = `/projects/${projectId}/specs/${specId}/diff`;

        console.log("[SpecSyncNotifier] addEvent →", { uid: sync.newHash, actionUrl: diffUrl });

        useEventStore.getState().addEvent({
          uid: `spec-sync-${specId}`,
          type: "warning",
          title: i18n.t("specSync.outdated"),
          message: i18n.t("specSync.hasChanges", { name }),
          details: { url: sync.url },
          actionUrl: diffUrl,
          actionLabel: i18n.t("specSync.viewChanges"),
        });

        console.log("[SpecSyncNotifier] store events after addEvent:", useEventStore.getState().events.map(e => ({ id: e.id, uid: e.uid, actionUrl: e.actionUrl })));
      }
    }

    // Clean up notified refs for specs that no longer have pending updates
    for (const id of notifiedRef.current) {
      if (!syncs[id]?.newContent) {
        notifiedRef.current.delete(id);
      }
    }
  }, [syncs, currentProject]);

  return null;
}
