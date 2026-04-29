import type { OpenAPIRoute } from "@/types/pipeline";
import type { KeyValue } from "./types";

export function buildUrl(
  baseUrl: string,
  path: string,
  pathParams: KeyValue[],
  queryParams: KeyValue[]
): string {
  let resolvedPath = path;
  for (const p of pathParams) {
    if (p.key && p.value) {
      resolvedPath = resolvedPath.replace(`{${p.key}}`, encodeURIComponent(p.value));
    }
  }
  const url = new URL(baseUrl);
  const basePath = url.pathname.replace(/\/+$/, "");
  const routePath = resolvedPath.replace(/^\/+/, "");

  // OpenAPI paths usually start with "/", but if the selected server already
  // contains a path prefix we must preserve it instead of replacing it.
  url.pathname = [basePath, routePath]
    .filter(Boolean)
    .join("/")
    .replace(/\/{2,}/g, "/") || "/";

  for (const q of queryParams) {
    if (q.key) url.searchParams.append(q.key, q.value);
  }
  return url.toString();
}

export function generateSampleBody(route: OpenAPIRoute): string {
  if (!route.requestBody?.content) return "{}";
  const jsonContent = route.requestBody.content["application/json"];
  if (!jsonContent?.schema) return "{}";
  const schema = jsonContent.schema;
  if (schema.type === "object" && schema.properties) {
    const sample: Record<string, unknown> = {};
    for (const [key, prop] of Object.entries(schema.properties as Record<string, any>)) {
      if (prop.type === "string") sample[key] = prop.example ?? "";
      else if (prop.type === "number" || prop.type === "integer") sample[key] = prop.example ?? 0;
      else if (prop.type === "boolean") sample[key] = prop.example ?? false;
      else if (prop.type === "array") sample[key] = [];
      else sample[key] = null;
    }
    return JSON.stringify(sample, null, 2);
  }
  return "{}";
}

export function statusColor(status: number): string {
  if (status >= 200 && status < 300) return "text-success";
  if (status >= 300 && status < 400) return "text-primary";
  if (status >= 400 && status < 500) return "text-warning";
  return "text-destructive";
}

export function formatBody(body: unknown): string {
  if (typeof body === "string") {
    try {
      return JSON.stringify(JSON.parse(body), null, 2);
    } catch {
      return body;
    }
  }
  return JSON.stringify(body, null, 2);
}
