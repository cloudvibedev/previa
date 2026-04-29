import yaml from "js-yaml";
import { OpenAPIV3 } from "openapi-types";
import type { OpenAPIV3_1 } from "openapi-types";
import type { OpenAPIRoute, OpenAPISpec, OpenAPIParameter, OpenAPIRequestBody, OpenAPIResponse } from "@/types/pipeline";

type V3Document = OpenAPIV3.Document | OpenAPIV3_1.Document;
type V3Operation = OpenAPIV3.OperationObject | OpenAPIV3_1.OperationObject;
type V3Parameter = OpenAPIV3.ParameterObject | OpenAPIV3_1.ParameterObject;
type V3ReferenceObject = OpenAPIV3.ReferenceObject;

function isRef(obj: unknown): obj is V3ReferenceObject {
  return typeof obj === "object" && obj !== null && "$ref" in obj;
}

function resolveRef(doc: V3Document, ref: string): unknown {
  const path = ref.replace(/^#\//, "").split("/");
  let current: unknown = doc;
  for (const segment of path) {
    if (current == null || typeof current !== "object") return undefined;
    current = (current as Record<string, unknown>)[segment];
  }
  return current;
}

function resolveSchema(doc: V3Document, schema: unknown): Record<string, unknown> | undefined {
  if (!schema || typeof schema !== "object") return undefined;
  const s = schema as Record<string, unknown>;
  if ("$ref" in s && typeof s.$ref === "string") {
    const resolved = resolveRef(doc, s.$ref);
    if (resolved && typeof resolved === "object") return resolved as Record<string, unknown>;
    return undefined;
  }
  return s;
}

function parseParameters(params?: (V3ReferenceObject | V3Parameter)[]): OpenAPIParameter[] {
  if (!params) return [];
  return params
    .filter((p): p is V3Parameter => !isRef(p))
    .map((param) => ({
      name: param.name,
      in: param.in as OpenAPIParameter["in"],
      required: param.required,
      description: param.description,
      schema: param.schema && !isRef(param.schema)
        ? (param.schema as Record<string, unknown>)
        : undefined,
    }));
}

function parseRequestBody(doc: V3Document, body?: V3ReferenceObject | OpenAPIV3.RequestBodyObject): OpenAPIRequestBody | undefined {
  if (!body) return undefined;
  if (isRef(body)) {
    const resolved = resolveRef(doc, body.$ref);
    if (!resolved || typeof resolved !== "object") return undefined;
    body = resolved as OpenAPIV3.RequestBodyObject;
  }
  return {
    description: body.description,
    required: body.required,
    content: body.content
      ? Object.fromEntries(
          Object.entries(body.content).map(([mediaType, mediaObj]) => [
            mediaType,
            { schema: resolveSchema(doc, mediaObj.schema) },
          ])
        )
      : undefined,
  };
}

function parseResponses(responses?: OpenAPIV3.ResponsesObject): OpenAPIResponse[] {
  if (!responses) return [];
  return Object.entries(responses)
    .filter(([, res]) => !isRef(res))
    .map(([statusCode, res]) => ({
      statusCode,
      description: (res as OpenAPIV3.ResponseObject).description,
    }));
}

function extractResponseFields(
  doc: V3Document,
  responses?: OpenAPIV3.ResponsesObject
): Array<{ name: string; type?: string; description?: string }> | undefined {
  if (!responses) return undefined;

  // Resolve response entry (may itself be a $ref)
  const pick = responses["200"] ?? responses["201"]
    ?? Object.entries(responses).find(([k]) => k.startsWith("2"))?.[1];
  if (!pick) return undefined;

  let responseObj = pick;
  if (isRef(responseObj)) {
    const resolved = resolveRef(doc, (responseObj as V3ReferenceObject).$ref);
    if (!resolved || typeof resolved !== "object") return undefined;
    responseObj = resolved as OpenAPIV3.ResponseObject;
  }

  const content = (responseObj as OpenAPIV3.ResponseObject).content;
  if (!content) return undefined;

  const mediaObj = content["application/json"] ?? Object.values(content)[0];
  if (!mediaObj?.schema) return undefined;

  let schema = resolveSchema(doc, mediaObj.schema);
  if (!schema) return undefined;

  // Handle allOf: merge properties
  if (Array.isArray(schema.allOf)) {
    const merged: Record<string, unknown> = {};
    for (const sub of schema.allOf) {
      const resolved = resolveSchema(doc, sub);
      if (resolved?.properties) Object.assign(merged, resolved.properties);
    }
    if (Object.keys(merged).length > 0) {
      return Object.entries(merged).map(([name, propSchema]) => {
        const ps = resolveSchema(doc, propSchema) ?? {};
        return { name, type: ps.type as string | undefined, description: ps.description as string | undefined };
      });
    }
  }

  // Handle array with items
  if (schema.type === "array" && schema.items) {
    schema = resolveSchema(doc, schema.items) ?? schema;
  }

  if (!schema.properties) return undefined;
  const props = schema.properties as Record<string, unknown>;
  return Object.entries(props).map(([name, propSchema]) => {
    const ps = resolveSchema(doc, propSchema) ?? {};
    return { name, type: ps.type as string | undefined, description: ps.description as string | undefined };
  });
}

export function parseOpenAPISpec(content: string): OpenAPISpec {
  let parsed: Record<string, unknown>;

  try {
    parsed = JSON.parse(content);
  } catch {
    try {
      parsed = yaml.load(content) as Record<string, unknown>;
    } catch {
      throw new Error("Invalid format. Please provide a valid JSON or YAML OpenAPI spec.");
    }
  }

  const doc = parsed as unknown as V3Document;

  if (!doc.info?.title) {
    throw new Error("Invalid OpenAPI/Swagger spec: missing info.title");
  }

  const paths = doc.paths;
  if (!paths) {
    throw new Error("Invalid OpenAPI/Swagger spec: missing paths");
  }

  const routes: OpenAPIRoute[] = [];
  const methods: OpenAPIV3.HttpMethods[] = [
    OpenAPIV3.HttpMethods.GET,
    OpenAPIV3.HttpMethods.POST,
    OpenAPIV3.HttpMethods.PUT,
    OpenAPIV3.HttpMethods.PATCH,
    OpenAPIV3.HttpMethods.DELETE,
    OpenAPIV3.HttpMethods.OPTIONS,
    OpenAPIV3.HttpMethods.HEAD,
  ];

  for (const [path, pathItem] of Object.entries(paths)) {
    if (!pathItem || isRef(pathItem)) continue;

    for (const method of methods) {
      const operation = (pathItem as Record<string, unknown>)[method] as V3Operation | undefined;
      if (!operation) continue;

      const parameters = parseParameters(operation.parameters as (V3ReferenceObject | V3Parameter)[] | undefined);
      const requestBody = parseRequestBody(doc, operation.requestBody as V3ReferenceObject | OpenAPIV3.RequestBodyObject | undefined);
      const responses = parseResponses(operation.responses as OpenAPIV3.ResponsesObject | undefined);

      const responseFields = extractResponseFields(doc, operation.responses as OpenAPIV3.ResponsesObject | undefined);

      routes.push({
        method: method.toUpperCase(),
        path,
        operationId: operation.operationId,
        summary: operation.summary,
        description: operation.description,
        tags: operation.tags,
        parameters: parameters.length > 0 ? parameters : undefined,
        requestBody,
        responses: responses.length > 0 ? responses : undefined,
        responseFields,
      });
    }
  }

  if (routes.length === 0) {
    throw new Error("No routes found in the OpenAPI spec.");
  }

  return {
    raw: parsed,
    title: doc.info.title,
    version: doc.info.version || "unknown",
    routes,
  };
}
