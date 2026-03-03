# `previa-main`

> Rustdoc-style crate documentation.

## Crate Purpose

`previa-main` is the orchestrator API. It routes execution to runners, aggregates SSE streams, and stores E2E/load history in SQLite.

## Quick Start

### Start runner(s)

```bash
ADDRESS=0.0.0.0 PORT=55880 cargo run -p previa-runner
```

You can also download prebuilt binaries at: **https://previa.dev/downloads**

### Start orchestrator

```bash
ORCHESTRATOR_DATABASE_URL="sqlite://orchestrator.db" \
RUNNER_ENDPOINTS="http://127.0.0.1:55880" \
ADDRESS=0.0.0.0 PORT=5588 \
cargo run -p previa-main
```

You can also download prebuilt binaries at: **https://previa.dev/downloads**

### Connect from UI

Use **https://previa.dev** and add:

```text
http://127.0.0.1:5588
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `ORCHESTRATOR_DATABASE_URL` | `sqlite://orchestrator.db` | SQLite database URL |
| `RUNNER_RPS_PER_NODE` | `1000` | Per-node capacity hint for load planning |
| `RUNNER_ENDPOINTS` | empty | Runner endpoints CSV |
| `ADDRESS` | `0.0.0.0` | Bind address |
| `PORT` | `8383` | Bind port |
| `RUST_LOG` | unset | Tracing filter |

## HTTP API Surface

Base URL: `http://localhost:5588`

### System

- `GET /health`
- `GET /info`
- `GET /openapi.json`

### Proxy

- `POST /proxy`

### Projects

- `GET /api/v1/projects`
- `POST /api/v1/projects`
- `GET /api/v1/projects/{projectId}`
- `PUT /api/v1/projects/{projectId}`
- `DELETE /api/v1/projects/{projectId}`

### Specs

- `POST /api/v1/specs/validate`
- `GET /api/v1/projects/{projectId}/specs`
- `POST /api/v1/projects/{projectId}/specs`
- `GET /api/v1/projects/{projectId}/specs/{specId}`
- `PUT /api/v1/projects/{projectId}/specs/{specId}`
- `DELETE /api/v1/projects/{projectId}/specs/{specId}`

### Pipelines

- `GET /api/v1/projects/{projectId}/pipelines`
- `POST /api/v1/projects/{projectId}/pipelines`
- `GET /api/v1/projects/{projectId}/pipelines/{pipelineId}`
- `PUT /api/v1/projects/{projectId}/pipelines/{pipelineId}`
- `DELETE /api/v1/projects/{projectId}/pipelines/{pipelineId}`

### E2E / Load Execution

- `POST /api/v1/projects/{projectId}/tests/e2e`
- `POST /api/v1/projects/{projectId}/tests/load`

### Execution Stream / Cancel

- `GET /api/v1/projects/{projectId}/executions/{executionId}`
- `POST /api/v1/executions/{executionId}/cancel`

### History

- `GET|DELETE /api/v1/projects/{projectId}/tests/e2e`
- `GET|DELETE /api/v1/projects/{projectId}/tests/e2e/{test_id}`
- `GET|DELETE /api/v1/projects/{projectId}/tests/load`
- `GET|DELETE /api/v1/projects/{projectId}/tests/load/{test_id}`

## SSE Events

Primary events emitted by orchestration flows:

- `execution:init`
- `step:start`
- `step:result`
- `pipeline:complete`
- `metrics`
- `complete`
- `error`

Common context fields include node planning and runner metadata (`nodesFound`, `nodesUsed`, `runners`, `warning`, etc.).

## Error Contract

```json
{
  "error": "bad_request|not_found|service_unavailable|internal_server_error",
  "message": "description"
}
```

## Module Relationship

```text
main -> runner -> engine
```

## Common Pitfalls

- Missing `RUNNER_ENDPOINTS`.
- No active runners on `/health`.
- Empty pipeline steps in execution payloads.
