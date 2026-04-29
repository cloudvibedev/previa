import type { Pipeline, PipelineStep, OpenAPISpec, OpenAPIRoute } from "@/types/pipeline";
import type { ProjectSpec } from "@/types/project";

export interface ContractWarning {
  stepId: string;
  stepIndex: number;
  type: "unknown_path" | "invalid_method" | "missing_path_param" | "unexpected_body" | "missing_required_header" | "unknown_spec";
  message: string;
}

/**
 * Extract the spec slug from a step URL.
 * E.g. `{{specs.auth-api.url.hml}}/users` → `"auth-api"`
 * Also supports legacy `{{url.auth-api.hml}}` format.
 */
export function extractSpecSlug(url: string): string | null {
  const match = url.match(/^\{\{specs\.([^.}]+)\.url\.[^}]+\}\}/) ?? url.match(/^\{\{url\.([^.}]+)\.[^}]+\}\}/);
  return match ? match[1] : null;
}

/**
 * Extract the relative path from a step URL by removing the {{specs.slug.url.env}} prefix.
 */
function extractRelativePath(url: string): string {
  return url
    .replace(/^\{\{specs\.[^}]+\}\}/, "")
    .replace(/^\{\{url\.[^}]+\}\}/, "");
}

/**
 * Normalize a step path segment by replacing {{...}} interpolations with a wildcard placeholder.
 */
function normalizeStepPath(relativePath: string): string {
  return relativePath.replace(/\{\{[^}]+\}\}/g, "{*}");
}

/**
 * Normalize an OpenAPI spec path by replacing named path params with the same wildcard.
 */
function normalizeSpecPath(specPath: string): string {
  return specPath.replace(/\{[^}]+\}/g, "{*}");
}

/**
 * Find matching routes from spec for a given step path.
 */
function findMatchingRoutes(normalizedStepPath: string, routes: OpenAPIRoute[]): OpenAPIRoute[] {
  return routes.filter((route) => normalizeSpecPath(route.path) === normalizedStepPath);
}

/**
 * Check if a step URL contains values for all required path parameters from the spec route.
 */
function getMissingPathParams(step: PipelineStep, route: OpenAPIRoute): string[] {
  const pathParams = (route.parameters || []).filter((p) => p.in === "path" && p.required !== false);
  if (pathParams.length === 0) return [];

  const relativePath = extractRelativePath(step.url);
  const segments = relativePath.split("/").filter(Boolean);
  const specSegments = route.path.split("/").filter(Boolean);

  const missing: string[] = [];
  for (let i = 0; i < specSegments.length; i++) {
    const specSeg = specSegments[i];
    const paramMatch = specSeg.match(/^\{(.+)\}$/);
    if (!paramMatch) continue;

    const paramName = paramMatch[1];
    const isRequired = pathParams.some((p) => p.name === paramName);
    if (!isRequired) continue;

    const stepSeg = segments[i];
    if (!stepSeg || stepSeg.trim() === "") {
      missing.push(paramName);
    }
  }

  return missing;
}

/**
 * Get required headers from the spec route that are missing in the step.
 */
function getMissingRequiredHeaders(step: PipelineStep, route: OpenAPIRoute): string[] {
  const requiredHeaders = (route.parameters || []).filter(
    (p) => p.in === "header" && p.required === true
  );
  if (requiredHeaders.length === 0) return [];

  const stepHeaderKeys = Object.keys(step.headers || {}).map((k) => k.toLowerCase());
  return requiredHeaders
    .filter((h) => !stepHeaderKeys.includes(h.name.toLowerCase()))
    .map((h) => h.name);
}

/**
 * Validate a pipeline's steps against OpenAPI specs, returning non-blocking warnings.
 * Uses the {{specs.<slug>.url.<env>}} to identify which spec to validate against.
 */
export function validatePipelineContract(
  pipeline: Pipeline,
  spec: OpenAPISpec | ProjectSpec[],
): ContractWarning[] {
  const isMultiSpec = Array.isArray(spec);

  // Build a map of slug → routes for multi-spec
  const specMap = new Map<string, OpenAPIRoute[]>();
  let allRoutes: OpenAPIRoute[] = [];

  if (isMultiSpec) {
    for (const s of spec) {
      specMap.set(s.slug, s.spec.routes);
      allRoutes.push(...s.spec.routes);
    }
  } else {
    allRoutes = spec.routes;
  }

  if (allRoutes.length === 0) return [];

  const warnings: ContractWarning[] = [];

  pipeline.steps.forEach((step, index) => {
    const slug = extractSpecSlug(step.url);
    const usesUrlVar = slug !== null;

    // Skip steps that don't reference a spec URL (external URLs)
    if (!usesUrlVar) {
      return;
    }

    // Determine which routes to validate against
    let routesToCheck: OpenAPIRoute[];
    if (usesUrlVar && isMultiSpec) {
      const specRoutes = specMap.get(slug);
      if (!specRoutes) {
        warnings.push({
          stepId: step.id,
          stepIndex: index,
          type: "unknown_spec",
          message: `Spec "${slug}" não encontrada no projeto`,
        });
        return;
      }
      routesToCheck = specRoutes;
    } else {
      routesToCheck = allRoutes;
    }

    const relativePath = extractRelativePath(step.url);
    const normalizedStep = normalizeStepPath(relativePath);
    const matchingRoutes = findMatchingRoutes(normalizedStep, routesToCheck);

    if (matchingRoutes.length === 0) {
      warnings.push({
        stepId: step.id,
        stepIndex: index,
        type: "unknown_path",
        message: `Rota "${relativePath}" não encontrada no spec${slug ? ` "${slug}"` : ""}`,
      });
      return;
    }

    // Check method
    const methodMatch = matchingRoutes.find(
      (r) => r.method.toUpperCase() === step.method.toUpperCase()
    );

    if (!methodMatch) {
      const allowed = matchingRoutes.map((r) => r.method.toUpperCase()).join(", ");
      warnings.push({
        stepId: step.id,
        stepIndex: index,
        type: "invalid_method",
        message: `Método ${step.method} não permitido para "${relativePath}". Permitidos: ${allowed}`,
      });
      return;
    }

    // Check path params
    const missingParams = getMissingPathParams(step, methodMatch);
    for (const param of missingParams) {
      warnings.push({
        stepId: step.id,
        stepIndex: index,
        type: "missing_path_param",
        message: `Parâmetro de path obrigatório "{${param}}" ausente na URL`,
      });
    }

    // Check unexpected body
    const noBodyMethods = ["GET", "HEAD", "DELETE"];
    if (
      noBodyMethods.includes(step.method.toUpperCase()) &&
      !methodMatch.requestBody &&
      step.body &&
      Object.keys(step.body).length > 0
    ) {
      warnings.push({
        stepId: step.id,
        stepIndex: index,
        type: "unexpected_body",
        message: `Rota ${step.method} "${relativePath}" não aceita request body`,
      });
    }

    // Check required headers
    const missingHeaders = getMissingRequiredHeaders(step, methodMatch);
    for (const header of missingHeaders) {
      warnings.push({
        stepId: step.id,
        stepIndex: index,
        type: "missing_required_header",
        message: `Header obrigatório "${header}" ausente`,
      });
    }
  });

  return warnings;
}
