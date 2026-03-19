# Spec-Driven Testing

Previa lets you attach OpenAPI specs to a project and use those specs as the source of truth for environment-aware pipelines.

## Why Use Specs

Specs help you:

- organize target APIs per project
- define stable runtime base URLs such as `hml` and `prd`
- write reusable pipelines that do not hardcode environment URLs
- let IDE and MCP workflows reason about the same API model

## Core Concepts

- `slug`: the short stable identifier for the spec inside templates
- `urls`: named runtime base URLs such as `hml`, `prd`, or `local`
- `selectedBaseUrlKey`: the runtime choice passed during execution

## Create a Spec

Example:

```bash
curl -sS http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/specs \
  -H 'content-type: application/json' \
  -d @- <<'JSON'
{
  "slug": "users",
  "urls": [
    { "name": "hml", "url": "https://hml.example.com" },
    { "name": "prd", "url": "https://api.example.com" }
  ],
  "sync": false,
  "live": false,
  "spec": {
    "openapi": "3.0.3",
    "info": { "title": "Users API", "version": "1.0.0" },
    "paths": {}
  }
}
JSON
```

## Use Specs in Pipelines

Reference the base URL with:

```text
{{specs.<slug>.url.<name>}}
```

Example:

```yaml
url: "{{specs.users.url.hml}}/users"
```

## Choose the Runtime Base URL

At execution time, pass:

```json
{
  "selectedBaseUrlKey": "hml"
}
```

This makes it possible to reuse the same pipeline against different environments.

## When to Use Specs vs Absolute URLs

Use specs when:

- the same pipeline should run across `local`, `hml`, and `prd`
- you want assistants and the IDE to reason about the API structure
- the base URL should be centralized per project

Use absolute URLs when:

- you are building a one-off smoke test
- the target is not part of a modeled project API yet

## Recommended Conventions

- keep `slug` lowercase and stable
- use short URL names like `local`, `hml`, `prd`
- avoid changing slug names once pipelines depend on them
- keep one spec per API domain when practical

## Common Mistakes

- creating a pipeline with `{{specs.users.url.hml}}` before the `users` spec exists
- using a URL name that is not present in the project spec
- forgetting to pass `selectedBaseUrlKey` consistently during execution

## See Also

- [Pipeline authoring](./pipeline-authoring.md)
- [API workflows](./api-workflows.md)
