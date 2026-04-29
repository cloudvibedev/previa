/**
 * Browser-local IndexedDB persistence for AI chat conversations.
 *
 * This is intentionally local-only product state. Conversations are scoped to
 * the current browser/profile and are not synchronized with the orchestrator.
 */

import { generateUUID } from "@/lib/uuid";

export interface ChatConversation {
  id: string;
  projectId: string;
  title: string;
  messages: ChatMessage[];
  createdAt: string;
  updatedAt: string;
  /** If set, this conversation is a sub-conversation (e.g. Solve Now worker) linked to a parent */
  parentConversationId?: string;
}

export interface ChatMessage {
  role: "user" | "assistant" | "system";
  content: string;
  displayTitle?: string;
  attachments?: { name: string; content: string }[];
  // UI-only fields are not persisted (scenarios, specSuggestions, etc.)
}

const DB_NAME = "previa-chat-v1";
const DB_VERSION = 1;
const STORE = "conversations";

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE)) {
        const store = db.createObjectStore(STORE, { keyPath: "id" });
        store.createIndex("projectId", "projectId", { unique: false });
        store.createIndex("updatedAt", "updatedAt", { unique: false });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function listConversations(projectId: string): Promise<ChatConversation[]> {
  const db = await openDB();
  const tx = db.transaction(STORE, "readonly");
  const store = tx.objectStore(STORE);
  const index = store.index("projectId");
  
  return new Promise((resolve, reject) => {
    const req = index.getAll(projectId);
    req.onsuccess = () => {
      const results = (req.result as ChatConversation[])
        .sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime());
      resolve(results);
    };
    req.onerror = () => reject(req.error);
  });
}

export async function getConversation(id: string): Promise<ChatConversation | null> {
  const db = await openDB();
  const tx = db.transaction(STORE, "readonly");
  return new Promise((resolve, reject) => {
    const req = tx.objectStore(STORE).get(id);
    req.onsuccess = () => resolve((req.result as ChatConversation) || null);
    req.onerror = () => reject(req.error);
  });
}

export async function saveConversation(conv: ChatConversation): Promise<void> {
  const db = await openDB();
  const tx = db.transaction(STORE, "readwrite");
  tx.objectStore(STORE).put(conv);
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

export async function deleteConversation(id: string): Promise<void> {
  const db = await openDB();
  const tx = db.transaction(STORE, "readwrite");
  tx.objectStore(STORE).delete(id);
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

export function createNewConversation(projectId: string): ChatConversation {
  const now = new Date().toISOString();
  return {
    id: generateUUID(),
    projectId,
    title: "Nova conversa",
    messages: [],
    createdAt: now,
    updatedAt: now,
  };
}

/** Derive a short title from the first user message */
export function deriveTitle(messages: ChatMessage[]): string {
  const first = messages.find((m) => m.role === "user" && !m.displayTitle);
  if (!first) return "Nova conversa";
  const text = first.content.replace(/\n/g, " ").trim();
  return text.length > 50 ? text.slice(0, 50) + "…" : text;
}
