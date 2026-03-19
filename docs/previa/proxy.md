# Proxy

Previa exposes `POST /proxy` on `previa-main` so you can probe live HTTP behavior from the same local stack used by the IDE, API, and MCP.

Base URL:

```text
http://127.0.0.1:5588/proxy
```

## What It Is For

Use `/proxy` when you want to:

- inspect a live endpoint before editing a pipeline
- test headers, auth, payloads, redirects, or query params
- inspect SSE behavior from an upstream service
- debug environment-specific API behavior from the local stack

## Basic Example

```bash
curl -sS http://127.0.0.1:5588/proxy \
  -H 'content-type: application/json' \
  -d '{
    "method": "GET",
    "url": "https://httpbin.org/status/200",
    "headers": {}
  }'
```

## JSON Request Example

```bash
curl -sS http://127.0.0.1:5588/proxy \
  -H 'content-type: application/json' \
  -d '{
    "method": "POST",
    "url": "https://httpbin.org/anything",
    "headers": {
      "content-type": "application/json"
    },
    "body": {
      "name": "Alice"
    }
  }'
```

## SSE Example

If the upstream responds with `text/event-stream`, Previa keeps the response as SSE:

```bash
curl -N http://127.0.0.1:5588/proxy \
  -H 'content-type: application/json' \
  -d '{
    "method": "GET",
    "url": "https://example.com/sse",
    "headers": {}
  }'
```

## Important Notes

- the `url` must be valid and absolute
- `method` must be a valid HTTP method
- headers must be valid HTTP header names and values
- for non-SSE upstream responses, Previa forwards the upstream status and body
- for SSE upstream responses, Previa streams SSE events back to the caller

Previa also filters some hop-by-hop and infrastructure headers from proxied responses to keep the local response compatible with the local HTTP writer.

## When to Prefer `/proxy`

Prefer `/proxy` when:

- you need evidence from the live API before changing a pipeline
- you want to check whether a failure is in the target API or in your pipeline logic
- your MCP assistant is doing live HTTP inspection

## See Also

- [API workflows](./api-workflows.md)
- [MCP integration](./mcp.md)
- [Troubleshooting](./troubleshooting.md)
