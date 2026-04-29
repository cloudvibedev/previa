import type { Pipeline } from "@/types/pipeline";

const PREFIX = "api-pipeline-studio:pipeline-order:";

export function getPipelineOrder(contextId: string): string[] {
  try {
    const raw = localStorage.getItem(`${PREFIX}${contextId}`);
    if (raw) return JSON.parse(raw) as string[];
  } catch { /* ignore */ }
  return [];
}

export function savePipelineOrder(contextId: string, order: string[]): void {
  localStorage.setItem(`${PREFIX}${contextId}`, JSON.stringify(order));
}

export function applyOrder(pipelines: Pipeline[], order: string[]): Pipeline[] {
  if (order.length === 0) return pipelines;
  const map = new Map(pipelines.map((p) => [p.id, p]));
  const ordered: Pipeline[] = [];
  for (const id of order) {
    const p = map.get(id);
    if (p) {
      ordered.push(p);
      map.delete(id);
    }
  }
  // Append any pipelines not in the saved order
  for (const p of map.values()) {
    ordered.push(p);
  }
  return ordered;
}
