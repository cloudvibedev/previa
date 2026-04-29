import type { StepExecutionResult } from "@/types/pipeline";
import { generateUUID } from "@/lib/uuid";

export interface ExecutionRun {
  id: string;
  projectId: string;
  pipelineIndex: number;
  pipelineName: string;
  status: "success" | "error" | "running";
  timestamp: string;
  duration: number;
  results: Record<string, StepExecutionResult>;
  executionId?: string;
}

const DB_NAME = "previa-executions-v1";
const DB_VERSION = 1;
const STORE_NAME = "runs";

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        const store = db.createObjectStore(STORE_NAME, { keyPath: "id" });
        store.createIndex("project_pipeline", ["projectId", "pipelineIndex"], { unique: false });
        store.createIndex("projectId", "projectId", { unique: false });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function getRuns(projectId: string, pipelineIndex: number): Promise<ExecutionRun[]> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readonly");
    const store = tx.objectStore(STORE_NAME);
    const index = store.index("project_pipeline");
    const req = index.getAll(IDBKeyRange.only([projectId, pipelineIndex]));
    req.onsuccess = () => {
      const runs = (req.result as ExecutionRun[]).sort(
        (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
      );
      resolve(runs);
    };
    req.onerror = () => reject(req.error);
  });
}

export async function getAllRunsForProject(projectId: string): Promise<ExecutionRun[]> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readonly");
    const store = tx.objectStore(STORE_NAME);
    const index = store.index("projectId");
    const req = index.getAll(IDBKeyRange.only(projectId));
    req.onsuccess = () => resolve(req.result as ExecutionRun[]);
    req.onerror = () => reject(req.error);
  });
}

export async function importRuns(runs: Omit<ExecutionRun, "id">[], newProjectId: string): Promise<void> {
  const db = await openDB();
  const tx = db.transaction(STORE_NAME, "readwrite");
  const store = tx.objectStore(STORE_NAME);
  for (const run of runs) {
    store.add({ ...run, id: generateUUID(), projectId: newProjectId });
  }
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

export async function deleteRunsForPipeline(projectId: string, pipelineIndex: number): Promise<void> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, "readwrite");
    const store = tx.objectStore(STORE_NAME);
    const index = store.index("project_pipeline");
    const req = index.openCursor(IDBKeyRange.only([projectId, pipelineIndex]));
    req.onsuccess = () => {
      const cursor = req.result;
      if (cursor) {
        cursor.delete();
        cursor.continue();
      } else {
        resolve();
      }
    };
    req.onerror = () => reject(req.error);
  });
}
