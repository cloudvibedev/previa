# MCP Integration

Previa can expose an MCP server from `previa-main`, allowing AI assistants to operate against your local stack.

## Enable MCP

Direct startup:

```bash
MCP_ENABLED=true cargo run -p previa-main
```

With compose input:

```yaml
version: 1
main:
  env:
    MCP_ENABLED: "true"
    MCP_PATH: /mcp
runners:
  local:
    count: 1
```

Then:

```bash
previa up -d .
```

## Endpoint

Default endpoint:

```text
http://localhost:5588/mcp
```

If you override the main port or `MCP_PATH`, update the URL accordingly.

## Codex Example

```toml
[mcp_servers.previa]
enabled = true
url = "http://localhost:5588/mcp"
```

## What Assistants Can Do

Through MCP, assistants can work with capabilities exposed by `previa-main`, including:

- project inspection
- pipeline creation and repair
- OpenAPI spec workflows
- E2E and load execution
- E2E queue operations
- project import and export
- HTTP probing through the proxy

## Suggested First Prompts

After connecting your assistant, try:

- inspect this project and summarize its specs and pipelines
- create a CRUD pipeline from my users spec
- run an E2E queue for these pipeline IDs
- analyze why this pipeline step failed

## See Also

- [Architecture at a glance](./architecture.md)
- [E2E queues](./e2e-queues.md)
