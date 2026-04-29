/**
 * OpenAPI 3.x / Swagger 2.x JSON Schema for Monaco Editor validation
 * Simplified schema covering the most common structures
 */
export const OPENAPI_3_SCHEMA = {
  $schema: "http://json-schema.org/draft-07/schema#",
  title: "OpenAPI / Swagger Specification",
  type: "object",
  required: ["info"],
  properties: {
    openapi: {
      type: "string",
      pattern: "^3\\.(0|1)\\.\\d+$",
      description: "OpenAPI version (3.0.x or 3.1.x)",
    },
    swagger: {
      type: "string",
      pattern: "^2\\.\\d+$",
      description: "Swagger version (2.x)",
    },
    info: {
      $ref: "#/definitions/Info",
    },
    servers: {
      type: "array",
      items: { $ref: "#/definitions/Server" },
    },
    paths: {
      $ref: "#/definitions/Paths",
    },
    components: {
      $ref: "#/definitions/Components",
    },
    security: {
      type: "array",
      items: { $ref: "#/definitions/SecurityRequirement" },
    },
    tags: {
      type: "array",
      items: { $ref: "#/definitions/Tag" },
    },
    externalDocs: {
      $ref: "#/definitions/ExternalDocumentation",
    },
  },
  definitions: {
    Info: {
      type: "object",
      required: ["title", "version"],
      properties: {
        title: { type: "string", description: "The title of the API" },
        description: { type: "string", description: "A short description of the API" },
        termsOfService: { type: "string", format: "uri" },
        contact: { $ref: "#/definitions/Contact" },
        license: { $ref: "#/definitions/License" },
        version: { type: "string", description: "The version of the OpenAPI document" },
      },
    },
    Contact: {
      type: "object",
      properties: {
        name: { type: "string" },
        url: { type: "string", format: "uri" },
        email: { type: "string", format: "email" },
      },
    },
    License: {
      type: "object",
      required: ["name"],
      properties: {
        name: { type: "string" },
        url: { type: "string", format: "uri" },
      },
    },
    Server: {
      type: "object",
      required: ["url"],
      properties: {
        url: { type: "string", description: "A URL to the target host" },
        description: { type: "string" },
        variables: {
          type: "object",
          additionalProperties: { $ref: "#/definitions/ServerVariable" },
        },
      },
    },
    ServerVariable: {
      type: "object",
      required: ["default"],
      properties: {
        enum: { type: "array", items: { type: "string" } },
        default: { type: "string" },
        description: { type: "string" },
      },
    },
    Paths: {
      type: "object",
      patternProperties: {
        "^/": { $ref: "#/definitions/PathItem" },
      },
      additionalProperties: false,
    },
    PathItem: {
      type: "object",
      properties: {
        $ref: { type: "string" },
        summary: { type: "string" },
        description: { type: "string" },
        get: { $ref: "#/definitions/Operation" },
        put: { $ref: "#/definitions/Operation" },
        post: { $ref: "#/definitions/Operation" },
        delete: { $ref: "#/definitions/Operation" },
        options: { $ref: "#/definitions/Operation" },
        head: { $ref: "#/definitions/Operation" },
        patch: { $ref: "#/definitions/Operation" },
        trace: { $ref: "#/definitions/Operation" },
        servers: { type: "array", items: { $ref: "#/definitions/Server" } },
        parameters: { type: "array", items: { $ref: "#/definitions/Parameter" } },
      },
    },
    Operation: {
      type: "object",
      properties: {
        tags: { type: "array", items: { type: "string" } },
        summary: { type: "string", description: "A short summary of what the operation does" },
        description: { type: "string", description: "A verbose explanation of the operation" },
        externalDocs: { $ref: "#/definitions/ExternalDocumentation" },
        operationId: { type: "string", description: "Unique identifier for the operation" },
        parameters: { type: "array", items: { $ref: "#/definitions/Parameter" } },
        requestBody: { $ref: "#/definitions/RequestBody" },
        responses: { $ref: "#/definitions/Responses" },
        callbacks: { type: "object", additionalProperties: { $ref: "#/definitions/Callback" } },
        deprecated: { type: "boolean", default: false },
        security: { type: "array", items: { $ref: "#/definitions/SecurityRequirement" } },
        servers: { type: "array", items: { $ref: "#/definitions/Server" } },
      },
      required: ["responses"],
    },
    Parameter: {
      type: "object",
      required: ["name", "in"],
      properties: {
        name: { type: "string" },
        in: { type: "string", enum: ["query", "header", "path", "cookie"] },
        description: { type: "string" },
        required: { type: "boolean" },
        deprecated: { type: "boolean" },
        allowEmptyValue: { type: "boolean" },
        style: { type: "string" },
        explode: { type: "boolean" },
        allowReserved: { type: "boolean" },
        schema: { $ref: "#/definitions/Schema" },
        example: {},
        examples: { type: "object", additionalProperties: { $ref: "#/definitions/Example" } },
        content: { type: "object", additionalProperties: { $ref: "#/definitions/MediaType" } },
      },
    },
    RequestBody: {
      type: "object",
      required: ["content"],
      properties: {
        description: { type: "string" },
        content: { type: "object", additionalProperties: { $ref: "#/definitions/MediaType" } },
        required: { type: "boolean", default: false },
      },
    },
    MediaType: {
      type: "object",
      properties: {
        schema: { $ref: "#/definitions/Schema" },
        example: {},
        examples: { type: "object", additionalProperties: { $ref: "#/definitions/Example" } },
        encoding: { type: "object", additionalProperties: { $ref: "#/definitions/Encoding" } },
      },
    },
    Encoding: {
      type: "object",
      properties: {
        contentType: { type: "string" },
        headers: { type: "object", additionalProperties: { $ref: "#/definitions/Header" } },
        style: { type: "string" },
        explode: { type: "boolean" },
        allowReserved: { type: "boolean" },
      },
    },
    Responses: {
      type: "object",
      properties: {
        default: { $ref: "#/definitions/Response" },
      },
      patternProperties: {
        "^[1-5]\\d{2}$": { $ref: "#/definitions/Response" },
      },
      additionalProperties: false,
    },
    Response: {
      type: "object",
      required: ["description"],
      properties: {
        description: { type: "string" },
        headers: { type: "object", additionalProperties: { $ref: "#/definitions/Header" } },
        content: { type: "object", additionalProperties: { $ref: "#/definitions/MediaType" } },
        links: { type: "object", additionalProperties: { $ref: "#/definitions/Link" } },
      },
    },
    Callback: {
      type: "object",
      additionalProperties: { $ref: "#/definitions/PathItem" },
    },
    Example: {
      type: "object",
      properties: {
        summary: { type: "string" },
        description: { type: "string" },
        value: {},
        externalValue: { type: "string", format: "uri" },
      },
    },
    Link: {
      type: "object",
      properties: {
        operationRef: { type: "string" },
        operationId: { type: "string" },
        parameters: { type: "object" },
        requestBody: {},
        description: { type: "string" },
        server: { $ref: "#/definitions/Server" },
      },
    },
    Header: {
      type: "object",
      properties: {
        description: { type: "string" },
        required: { type: "boolean" },
        deprecated: { type: "boolean" },
        allowEmptyValue: { type: "boolean" },
        style: { type: "string" },
        explode: { type: "boolean" },
        allowReserved: { type: "boolean" },
        schema: { $ref: "#/definitions/Schema" },
        example: {},
        examples: { type: "object", additionalProperties: { $ref: "#/definitions/Example" } },
        content: { type: "object", additionalProperties: { $ref: "#/definitions/MediaType" } },
      },
    },
    Tag: {
      type: "object",
      required: ["name"],
      properties: {
        name: { type: "string" },
        description: { type: "string" },
        externalDocs: { $ref: "#/definitions/ExternalDocumentation" },
      },
    },
    ExternalDocumentation: {
      type: "object",
      required: ["url"],
      properties: {
        description: { type: "string" },
        url: { type: "string", format: "uri" },
      },
    },
    Schema: {
      type: "object",
      properties: {
        $ref: { type: "string" },
        type: {
          oneOf: [
            { type: "string", enum: ["array", "boolean", "integer", "number", "object", "string", "null"] },
            { type: "array", items: { type: "string", enum: ["array", "boolean", "integer", "number", "object", "string", "null"] } },
          ],
        },
        format: { type: "string" },
        title: { type: "string" },
        description: { type: "string" },
        default: {},
        enum: { type: "array" },
        const: {},
        multipleOf: { type: "number", exclusiveMinimum: 0 },
        maximum: { type: "number" },
        exclusiveMaximum: { type: "number" },
        minimum: { type: "number" },
        exclusiveMinimum: { type: "number" },
        maxLength: { type: "integer", minimum: 0 },
        minLength: { type: "integer", minimum: 0, default: 0 },
        pattern: { type: "string", format: "regex" },
        maxItems: { type: "integer", minimum: 0 },
        minItems: { type: "integer", minimum: 0, default: 0 },
        uniqueItems: { type: "boolean", default: false },
        maxProperties: { type: "integer", minimum: 0 },
        minProperties: { type: "integer", minimum: 0, default: 0 },
        required: { type: "array", items: { type: "string" } },
        items: { $ref: "#/definitions/Schema" },
        properties: { type: "object", additionalProperties: { $ref: "#/definitions/Schema" } },
        additionalProperties: {
          oneOf: [{ type: "boolean" }, { $ref: "#/definitions/Schema" }],
        },
        allOf: { type: "array", items: { $ref: "#/definitions/Schema" } },
        oneOf: { type: "array", items: { $ref: "#/definitions/Schema" } },
        anyOf: { type: "array", items: { $ref: "#/definitions/Schema" } },
        not: { $ref: "#/definitions/Schema" },
        nullable: { type: "boolean", default: false },
        discriminator: { $ref: "#/definitions/Discriminator" },
        readOnly: { type: "boolean", default: false },
        writeOnly: { type: "boolean", default: false },
        xml: { $ref: "#/definitions/XML" },
        externalDocs: { $ref: "#/definitions/ExternalDocumentation" },
        example: {},
        deprecated: { type: "boolean", default: false },
      },
    },
    Discriminator: {
      type: "object",
      required: ["propertyName"],
      properties: {
        propertyName: { type: "string" },
        mapping: { type: "object", additionalProperties: { type: "string" } },
      },
    },
    XML: {
      type: "object",
      properties: {
        name: { type: "string" },
        namespace: { type: "string", format: "uri" },
        prefix: { type: "string" },
        attribute: { type: "boolean", default: false },
        wrapped: { type: "boolean", default: false },
      },
    },
    Components: {
      type: "object",
      properties: {
        schemas: { type: "object", additionalProperties: { $ref: "#/definitions/Schema" } },
        responses: { type: "object", additionalProperties: { $ref: "#/definitions/Response" } },
        parameters: { type: "object", additionalProperties: { $ref: "#/definitions/Parameter" } },
        examples: { type: "object", additionalProperties: { $ref: "#/definitions/Example" } },
        requestBodies: { type: "object", additionalProperties: { $ref: "#/definitions/RequestBody" } },
        headers: { type: "object", additionalProperties: { $ref: "#/definitions/Header" } },
        securitySchemes: { type: "object", additionalProperties: { $ref: "#/definitions/SecurityScheme" } },
        links: { type: "object", additionalProperties: { $ref: "#/definitions/Link" } },
        callbacks: { type: "object", additionalProperties: { $ref: "#/definitions/Callback" } },
      },
    },
    SecurityScheme: {
      type: "object",
      required: ["type"],
      properties: {
        type: { type: "string", enum: ["apiKey", "http", "oauth2", "openIdConnect"] },
        description: { type: "string" },
        name: { type: "string" },
        in: { type: "string", enum: ["query", "header", "cookie"] },
        scheme: { type: "string" },
        bearerFormat: { type: "string" },
        flows: { $ref: "#/definitions/OAuthFlows" },
        openIdConnectUrl: { type: "string", format: "uri" },
      },
    },
    OAuthFlows: {
      type: "object",
      properties: {
        implicit: { $ref: "#/definitions/OAuthFlow" },
        password: { $ref: "#/definitions/OAuthFlow" },
        clientCredentials: { $ref: "#/definitions/OAuthFlow" },
        authorizationCode: { $ref: "#/definitions/OAuthFlow" },
      },
    },
    OAuthFlow: {
      type: "object",
      required: ["scopes"],
      properties: {
        authorizationUrl: { type: "string", format: "uri" },
        tokenUrl: { type: "string", format: "uri" },
        refreshUrl: { type: "string", format: "uri" },
        scopes: { type: "object", additionalProperties: { type: "string" } },
      },
    },
    SecurityRequirement: {
      type: "object",
      additionalProperties: { type: "array", items: { type: "string" } },
    },
  },
};
