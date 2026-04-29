export type ProductModule = "qa" | "atlas" | "o11y";

interface ProductModuleConfig {
  id: ProductModule;
  label: string;
  routePrefix: `/${string}`;
  apiPrefix: `/${string}/api/v1`;
}

export const DEFAULT_PRODUCT_MODULE: ProductModule = "qa";

export const PRODUCT_MODULES: Record<ProductModule, ProductModuleConfig> = {
  qa: {
    id: "qa",
    label: "Previa QA",
    routePrefix: "/qa",
    apiPrefix: "/qa/api/v1",
  },
  atlas: {
    id: "atlas",
    label: "Previa Atlas",
    routePrefix: "/atlas",
    apiPrefix: "/atlas/api/v1",
  },
  o11y: {
    id: "o11y",
    label: "Previa Watch",
    routePrefix: "/o11y",
    apiPrefix: "/o11y/api/v1",
  },
};

const KNOWN_API_SUFFIX_RE = /\/(?:(qa|atlas|o11y)\/)?api\/v1\/?$/;

function normalizeModuleSubpath(path = ""): string {
  if (!path) return "";
  const normalized = path.startsWith("/") ? path : `/${path}`;
  return normalized.replace(/\/+$/, "");
}

function matchesModulePrefix(pathname: string, routePrefix: string): boolean {
  return pathname === routePrefix || pathname.startsWith(`${routePrefix}/`);
}

export function getProductModuleConfig(module: ProductModule): ProductModuleConfig {
  return PRODUCT_MODULES[module];
}

export function getModuleRootPath(module: ProductModule): string {
  return getProductModuleConfig(module).routePrefix;
}

export function buildModulePath(module: ProductModule, subpath = ""): string {
  const suffix = normalizeModuleSubpath(subpath);
  return suffix ? `${getModuleRootPath(module)}${suffix}` : getModuleRootPath(module);
}

export function getProductModuleFromPath(pathname: string): ProductModule {
  if (matchesModulePrefix(pathname, PRODUCT_MODULES.atlas.routePrefix)) return "atlas";
  if (matchesModulePrefix(pathname, PRODUCT_MODULES.o11y.routePrefix)) return "o11y";
  return "qa";
}

export function stripProductApiSuffix(url: string): string {
  const clean = url.replace(/\/+$/, "");
  return clean.replace(KNOWN_API_SUFFIX_RE, "");
}

export function resolveModuleApiBaseUrl(
  url: string,
  module: ProductModule = DEFAULT_PRODUCT_MODULE,
  options?: { preferLegacyQa?: boolean },
): string {
  const clean = url.replace(/\/+$/, "");
  if (KNOWN_API_SUFFIX_RE.test(clean)) return clean;

  const base = stripProductApiSuffix(clean);
  if (module === "qa" && options?.preferLegacyQa !== false) {
    return `${base}/api/v1`;
  }

  return `${base}${getProductModuleConfig(module).apiPrefix}`;
}

export const qaPaths = {
  home: () => buildModulePath("qa"),
  project: (projectId: string) => buildModulePath("qa", `/projects/${projectId}`),
  projectDashboard: (projectId: string) => buildModulePath("qa", `/projects/${projectId}/dashboard`),
  pipeline: (projectId: string, pipelineId: string) => buildModulePath("qa", `/projects/${projectId}/pipeline/${pipelineId}`),
  newPipelineEditor: (projectId: string) => buildModulePath("qa", `/projects/${projectId}/pipeline/new/editor`),
  pipelineEditor: (projectId: string, pipelineId: string) => buildModulePath("qa", `/projects/${projectId}/pipeline/${pipelineId}/editor`),
  pipelineIntegrationTest: (projectId: string, pipelineId: string) => buildModulePath("qa", `/projects/${projectId}/pipeline/${pipelineId}/integration-test`),
  pipelineLoadTest: (projectId: string, pipelineId: string) => buildModulePath("qa", `/projects/${projectId}/pipeline/${pipelineId}/load-test`),
  pipelineDashboard: (projectId: string, pipelineId: string) => buildModulePath("qa", `/projects/${projectId}/pipeline/${pipelineId}/dashboard`),
  newSpecEditor: (projectId: string) => buildModulePath("qa", `/projects/${projectId}/specs/new/editor`),
  newSpecTryIt: (projectId: string) => buildModulePath("qa", `/projects/${projectId}/specs/new/try-it`),
  specEditor: (projectId: string, specId: string) => buildModulePath("qa", `/projects/${projectId}/specs/${specId}/editor`),
  specTryIt: (projectId: string, specId: string) => buildModulePath("qa", `/projects/${projectId}/specs/${specId}/try-it`),
  specDiff: (projectId: string, specId: string) => buildModulePath("qa", `/projects/${projectId}/specs/${specId}/diff`),
};

export function rewriteLegacyQaPath(pathname: string): string | null {
  const clean = pathname.replace(/\/+$/, "") || "/";
  if (clean === "/" || clean === "/projects") return qaPaths.home();

  if (clean.startsWith("/qa")) return clean;

  if (clean.startsWith("/projects/")) {
    return buildModulePath("qa", clean);
  }

  const singularProjectMatch = clean.match(/^\/project\/([^/]+)(\/.*)?$/);
  if (singularProjectMatch) {
    const [, projectId, rest = ""] = singularProjectMatch;
    return buildModulePath("qa", `/projects/${projectId}${rest}`);
  }

  return null;
}
