import type { Project } from "@/types/project";
import { getApiUrl } from "@/stores/useOrchestratorStore";
import {
  exportProjectRemote,
  importProjectRemote,
  type ProjectExportEnvelope,
} from "@/lib/api-client";

function downloadJson(payload: unknown, fileName: string) {
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = fileName;
  a.click();
  URL.revokeObjectURL(url);
}

function requireApi(): string {
  const apiUrl = getApiUrl();
  if (!apiUrl) throw new Error("Backend não conectado. Configure a URL do orquestrador para exportar/importar projetos.");
  return apiUrl;
}

export async function exportProject(project: Project, includeHistory: boolean): Promise<void> {
  const apiUrl = requireApi();
  const fileName = `${project.name.toLowerCase().replace(/\s+/g, "-")}.previa.json`;
  const envelope = await exportProjectRemote(apiUrl, project.id, includeHistory);
  downloadJson(envelope, fileName);
}

export async function importProject(fileContent: string): Promise<Project> {
  const apiUrl = requireApi();

  let parsed: any;
  try {
    parsed = JSON.parse(fileContent);
  } catch {
    throw new Error("Arquivo JSON inválido.");
  }

  const hasHistory = !!(parsed.history && parsed.history.length > 0)
    || !!(parsed.loadTestHistory && parsed.loadTestHistory.length > 0);

  const result = await importProjectRemote(
    apiUrl,
    parsed as ProjectExportEnvelope,
    hasHistory,
  );

  return {
    id: result.id,
    name: result.name,
    description: parsed.project?.description,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    specs: parsed.project?.specs || [],
    pipelines: parsed.project?.pipelines || [],
  };
}
