/**
 * Spec Sync — check for remote spec updates via MD5 comparison.
 * localStorage has been removed; persistence is handled by the backend API.
 */

import { validateSpec } from "@/lib/api-client";
import { getApiUrl } from "@/stores/useOrchestratorStore";

export async function checkForUpdate(
  url: string,
  currentHash: string
): Promise<{ changed: boolean; newContent?: string; newHash?: string }> {
  try {
    const baseUrl = getApiUrl();
    if (!baseUrl) return { changed: false };

    const res = await fetch(url);
    if (!res.ok) return { changed: false };
    const source = await res.text();

    const result = await validateSpec(baseUrl, source);
    if (result.sourceMd5 !== currentHash) {
      return { changed: true, newContent: source, newHash: result.sourceMd5 };
    }
    return { changed: false };
  } catch {
    return { changed: false };
  }
}
