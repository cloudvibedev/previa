import { generateUUID } from "./uuid";

export interface OrchestratorContext {
  id: string;
  name: string;
  url: string;
}

const CONTEXTS_KEY = "api-pipeline-studio:contexts";
const ACTIVE_KEY = "api-pipeline-studio:active-context-id";
const LEGACY_KEY = "api-pipeline-studio:orchestrator-url";

export function getContexts(): OrchestratorContext[] {
  try {
    const raw = localStorage.getItem(CONTEXTS_KEY);
    if (raw) return JSON.parse(raw) as OrchestratorContext[];
  } catch { /* ignore */ }

  // Migrate legacy single URL
  const legacyUrl = localStorage.getItem(LEGACY_KEY) || (window as any).API_URL;
  if (legacyUrl) {
    const ctx: OrchestratorContext = { id: generateUUID(), name: "Default", url: legacyUrl.replace(/\/+$/, "") };
    saveContexts([ctx]);
    setActiveContextId(ctx.id);
    localStorage.removeItem(LEGACY_KEY);
    return [ctx];
  }

  return [];
}

export function saveContexts(list: OrchestratorContext[]): void {
  localStorage.setItem(CONTEXTS_KEY, JSON.stringify(list));
}

export function getActiveContextId(): string | null {
  return localStorage.getItem(ACTIVE_KEY);
}

export function setActiveContextId(id: string | null): void {
  if (id) localStorage.setItem(ACTIVE_KEY, id);
  else localStorage.removeItem(ACTIVE_KEY);
}

export function getActiveContext(): OrchestratorContext | null {
  const contexts = getContexts();
  const activeId = getActiveContextId();
  if (!activeId) return contexts[0] || null;
  return contexts.find((c) => c.id === activeId) || contexts[0] || null;
}
