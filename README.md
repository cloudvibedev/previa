<p align="center">
  <img src="assets/logo.png" alt="Previa logo" width="220">
</p>

# Previa

**The first AI-First IDE for QA. Test, design, and validate APIs with AI assistance from your desktop, CI/CD, or your favorite AI assistant.**

Previa is a platform for simulating, executing, and tracing real end-to-end API operations so you can understand exactly what happened, where a failure occurred, and why.

The `previa` CLI is the local entry point for running a Previa stack on your machine. It starts `previa-main`, manages local and attached `previa-runner` instances, opens the IDE in the browser, and helps you bootstrap projects from pipeline files.

## What Is Previa?

Previa combines local runtime operations with project-scoped API testing workflows:

- `previa` runs and manages the local stack
- `previa-main` is the orchestrator API for projects, specs, pipelines, history, proxying, and test execution
- `previa-runner` executes E2E and load tests
- `previa-engine` resolves templates, performs HTTP steps, and evaluates assertions
- the browser IDE at `https://ide.previa.dev` connects to your local `previa-main`
- MCP integrations can drive project, spec, pipeline, and execution workflows through the same platform

## Summary

- [Install](#install)
- [Quick Start](#quick-start)
- [Documentation](#documentation)
- [MCP Integration](#mcp-integration)
- [Use Cases](#use-cases)
- [Previa Compose](#previa-compose)
- [PREVIA_HOME](#previa_home)
- [Import Pipeline Files From a Repository](#import-pipeline-files-from-a-repository)
- [Import and Export Projects](#import-and-export-projects)
- [Final Notes](#final-notes)
- [License](#license)

## Install

Install the CLI with:

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

Today the installer targets Linux and writes `previa` under `~/.previa/bin`, while also setting `PREVIA_HOME="$HOME/.previa"`.

## Quick Start

`-d` is the short form of `--detach`.

Start a Docker-backed stack:

```bash
previa up -d
```

This is the general runtime path when Docker is available.

Start a binary-backed stack without Docker:

```bash
previa up -d --bin
```

This mode uses local `previa-main` and `previa-runner` binaries. Published runtime binaries are currently Linux-only.

Inspect the runtime and open the IDE:

```bash
previa status
previa open
```

`previa open` launches your default browser with a URL in this shape:

```text
https://ide.previa.dev?add_context=http%3A%2F%2F127.0.0.1%3A5588
```

That URL attaches your local `previa-main` context to the hosted IDE at `https://ide.previa.dev`.

## Documentation

Core guides:

- [Architecture at a glance](docs/previa/architecture.md)
- [Minimal happy path](docs/previa/minimal-happy-path.md)
- [Runtime modes](docs/previa/runtime-modes.md)
- [Main and runner authentication](docs/previa/main-runner-auth.md)
- [Remote runners](docs/previa/remote-runners.md)
- [MCP integration](docs/previa/mcp.md)
- [E2E queues](docs/previa/e2e-queues.md)

Existing operator guides:

- [Previa CLI docs index](docs/previa/README.md)
- [Getting started](docs/previa/getting-started.md)
- [Home and contexts](docs/previa/home-and-contexts.md)
- [Compose source](docs/previa/compose.md)
- [Pipeline import](docs/previa/pipeline-import.md)
- [Operations](docs/previa/operations.md)
- [Troubleshooting](docs/previa/troubleshooting.md)

## MCP Integration

Previa can expose an MCP server from `previa-main`, so you can connect the local stack to your favorite AI assistant.

### 1. Enable MCP on `previa-main`

When starting `previa-main` directly, enable it with:

```bash
MCP_ENABLED=true cargo run -p previa-main
```

If you are using `previa up`, the cleanest option is to enable it through the main environment in `previa-compose.yaml`:

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

Then start the stack:

```bash
previa up -d .
```

By default, the MCP endpoint is exposed at:

```text
http://localhost:5588/mcp
```

If you changed the main port or `MCP_PATH`, adjust the URL accordingly.

### 2. Connect your assistant

Any assistant or MCP client that supports remote HTTP MCP can point to the Previa endpoint.

For Codex, the configuration looks like this:

```toml
[mcp_servers.previa]
enabled = true
url = "http://localhost:5588/mcp"
```

On the same machine, `localhost` is usually the right host even if `previa-main` is bound to `0.0.0.0`.

### 3. What the assistant can do

Once connected, an MCP-enabled assistant can work with the same Previa platform capabilities exposed by `previa-main`, including:

- project and pipeline inspection
- OpenAPI spec workflows
- E2E and load execution flows
- E2E queue operations
- project import and export
- live API probing through the proxy

That means you can use your assistant to inspect a project, propose or create pipelines, analyze failures, and operate test workflows against the same local stack you opened with `previa open`.

## Use Cases

The examples below assume you already started a local stack with `previa up -d` or `previa up -d --bin`.

Important boundary:

- use the CLI to start and operate the local stack
- use the IDE, API, or MCP to create projects, specs, pipelines, and executions

### 1. Create a CRUD users API spec

Create a project:

```bash
curl -sS http://127.0.0.1:5588/api/v1/projects \
  -H 'content-type: application/json' \
  -d '{
    "name": "Users API",
    "description": "CRUD validation for the users service",
    "pipelines": []
  }'
```

Copy the returned `id` and reuse it below:

```bash
PROJECT_ID="<project-id>"
```

Create a project spec with multiple base URLs and an OpenAPI document:

```bash
curl -sS http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/specs \
  -H 'content-type: application/json' \
  -d @- <<'JSON'
{
  "slug": "users",
  "urls": [
    {
      "name": "hml",
      "url": "https://hml.cloudvibe.dev",
      "description": "Homologation environment"
    },
    {
      "name": "prd",
      "url": "https://api.cloudvibe.dev",
      "description": "Production environment"
    }
  ],
  "sync": false,
  "live": false,
  "spec": {
    "openapi": "3.0.3",
    "info": {
      "title": "Users API",
      "version": "1.0.0"
    },
    "paths": {
      "/users": {
        "get": {
          "responses": {
            "200": {
              "description": "List users"
            }
          }
        },
        "post": {
          "requestBody": {
            "required": true,
            "content": {
              "application/json": {
                "schema": {
                  "type": "object",
                  "required": ["name", "email"],
                  "properties": {
                    "name": { "type": "string" },
                    "email": { "type": "string", "format": "email" }
                  }
                }
              }
            }
          },
          "responses": {
            "201": {
              "description": "User created"
            }
          }
        }
      },
      "/users/{id}": {
        "parameters": [
          {
            "name": "id",
            "in": "path",
            "required": true,
            "schema": { "type": "string" }
          }
        ],
        "get": {
          "responses": {
            "200": { "description": "User found" },
            "404": { "description": "User not found" }
          }
        },
        "delete": {
          "responses": {
            "204": { "description": "User deleted" }
          }
        }
      }
    }
  }
}
JSON
```

At this point the project has a runtime spec slug called `users`, which can be referenced from pipelines as `{{specs.users.url.hml}}` or `{{specs.users.url.prd}}`.

### 2. Create a pipeline for that API

Create a project pipeline through the API:

```bash
curl -sS http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/pipelines \
  -H 'content-type: application/json' \
  -d @- <<'JSON'
{
  "name": "Users CRUD flow",
  "description": "Creates, fetches, and deletes a user through the users API.",
  "steps": [
    {
      "id": "create_user",
      "name": "Create user",
      "method": "POST",
      "url": "{{specs.users.url.hml}}/users",
      "headers": {
        "content-type": "application/json",
        "x-request-id": "{{helpers.uuid}}"
      },
      "body": {
        "name": "{{helpers.name}}",
        "email": "{{helpers.email}}"
      },
      "asserts": [
        {
          "field": "status",
          "operator": "equals",
          "expected": "201"
        }
      ]
    },
    {
      "id": "get_user",
      "name": "Get user",
      "method": "GET",
      "url": "{{specs.users.url.hml}}/users/{{steps.create_user.id}}",
      "headers": {},
      "asserts": [
        {
          "field": "status",
          "operator": "equals",
          "expected": "200"
        },
        {
          "field": "body.email",
          "operator": "equals",
          "expected": "{{steps.create_user.email}}"
        }
      ]
    },
    {
      "id": "delete_user",
      "name": "Delete user",
      "method": "DELETE",
      "url": "{{specs.users.url.hml}}/users/{{steps.create_user.id}}",
      "headers": {},
      "asserts": [
        {
          "field": "status",
          "operator": "equals",
          "expected": "204"
        }
      ]
    }
  ]
}
JSON
```

Copy the returned pipeline `id` for execution:

```bash
PIPELINE_ID="<pipeline-id>"
```

### 3. Run an E2E test against the users API

Run the stored project pipeline as an E2E execution:

```bash
curl -N http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e \
  -H 'content-type: application/json' \
  -d @- <<JSON
{
  "pipelineId": "$PIPELINE_ID",
  "selectedBaseUrlKey": "hml",
  "specs": []
}
JSON
```

The response is an SSE stream with events such as `execution:init`, `step:start`, `step:result`, and `pipeline:complete`.

### 4. Run a load test against the users API

Run a load test for the same stored pipeline:

```bash
curl -N http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/load \
  -H 'content-type: application/json' \
  -d @- <<JSON
{
  "pipelineId": "$PIPELINE_ID",
  "selectedBaseUrlKey": "hml",
  "config": {
    "totalRequests": 1000,
    "concurrency": 20,
    "rampUpSeconds": 10
  },
  "specs": []
}
JSON
```

This response is also an SSE stream, with repeated `metrics` events followed by `complete`.

### 5. Run an E2E queue

When you want to execute multiple stored pipelines in order for the same project, create an E2E queue:

```bash
curl -sS -D queue.headers \
  http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue \
  -H 'content-type: application/json' \
  -d @- <<JSON
{
  "pipelineIds": [
    "$PIPELINE_ID",
    "another-pipeline-id"
  ],
  "selectedBaseUrlKey": "hml",
  "specs": []
}
JSON
```

The API responds with `202 Accepted`, returns a JSON snapshot of the queue, and also includes:

- `x-queue-id`
- `Location: /api/v1/projects/<projectId>/tests/e2e/queue/<queueId>`

You can inspect the current active queue snapshot for the project:

```bash
curl -sS http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue
```

And you can follow a specific queue by ID:

```bash
QUEUE_ID="<queue-id>"

curl -N http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue/$QUEUE_ID
```

While the queue is active, that endpoint returns SSE updates such as `queue:update`. Once the queue finishes, the same endpoint returns the final JSON snapshot instead.

If needed, cancel the queue:

```bash
curl -X DELETE \
  http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/e2e/queue/$QUEUE_ID
```

## Previa Compose

`previa up` can read a compose-like runtime description from:

- `previa-compose.yaml`
- `previa-compose.yml`
- `previa-compose.json`

Example `previa-compose.yaml`:

```yaml
version: 1
main:
  address: 0.0.0.0
  port: 5588
  env:
    RUST_LOG: info
runners:
  local:
    address: 127.0.0.1
    count: 2
    port_range:
      start: 55880
      end: 55889
    env:
      RUST_LOG: info
  attach:
    - 10.0.0.12:55880
```

Run it from the current directory:

```bash
previa up .
```

Run it from an explicit file path:

```bash
previa up ./previa-compose.yaml
```

CLI flags still override values from the compose source.

## PREVIA_HOME

`PREVIA_HOME` is the root directory for local Previa state.

Resolution order:

1. `--home <path>`
2. `PREVIA_HOME`
3. `$HOME/.previa`

Typical layout:

```text
$PREVIA_HOME/
  bin/
    previa
    previa-main
    previa-runner
  stacks/
    <context>/
      config/
        main.env
        runner.env
      data/
        main/
          orchestrator.db
      logs/
        main.log
        runners/
          <port>.log
      run/
        docker-compose.generated.yaml
        lock
        state.json
```

Use a project-local home when you want a self-contained environment inside a repository:

```bash
previa --home ./.previa up -d
previa --home ./.previa status
previa --home ./.previa open
```

That is the current way to create a local Previa environment scoped to the repo. A future `--local` convenience flag could wrap the same idea, but `--local` is not a CLI flag today.

## Import Pipeline Files From a Repository

Previa can bootstrap a new local project from pipeline files when the runtime starts.

Supported pipeline suffixes are:

- `.previa`
- `.previa.json`
- `.previa.yaml`
- `.previa.yml`

Example pipeline file:

```yaml
id: users-crud
name: Users CRUD flow
description: CRUD regression coverage for the users API.
steps:
  - id: create_user
    name: Create user
    method: POST
    url: https://hml.cloudvibe.dev/users
    headers:
      content-type: application/json
      x-request-id: "{{helpers.uuid}}"
    body:
      name: "{{helpers.name}}"
      email: "{{helpers.email}}"
    asserts:
      - field: status
        operator: equals
        expected: "201"
  - id: get_user
    name: Get user
    method: GET
    url: https://hml.cloudvibe.dev/users/{{steps.create_user.id}}
    headers: {}
    asserts:
      - field: status
        operator: equals
        expected: "200"
  - id: delete_user
    name: Delete user
    method: DELETE
    url: https://hml.cloudvibe.dev/users/{{steps.create_user.id}}
    headers: {}
    asserts:
      - field: status
        operator: equals
        expected: "204"
```

Import a single file into a freshly started detached stack:

```bash
previa up -d --import ./tests/e2e/users-crud.previa.yaml --stack users_api
```

Import an entire directory recursively:

```bash
previa up -d -i ./tests/e2e -r -s users_api
```

This creates a new local project named by `--stack` and stores the imported pipelines under that project.

## Import and Export Projects

Project bundle import and export exist today through the platform API, not as dedicated `previa import` or `previa export` CLI commands.

Export a project bundle:

```bash
curl -sS "http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/export?includeHistory=true" \
  -o users-api.project.json
```

Import the same bundle into another Previa environment:

```bash
curl -sS http://127.0.0.1:5588/api/v1/projects/import?includeHistory=true \
  -H 'content-type: application/json' \
  -d @users-api.project.json
```

This flow is useful for moving project definitions, specs, pipelines, and optional execution history between environments. If you are using MCP, the same capability is also exposed there through project migration tools.

## Final Notes

Previa can be used in several layers at once:

- locally through the `previa` CLI
- visually through the IDE at `https://ide.previa.dev`
- programmatically through the HTTP API exposed by `previa-main`
- remotely through MCP-enabled assistants

For more detail, start with:

- [Previa CLI docs index](docs/previa/README.md)
- [Getting started](docs/previa/getting-started.md)
- [Compose source](docs/previa/compose.md)
- [Home and contexts](docs/previa/home-and-contexts.md)
- [Pipeline import](docs/previa/pipeline-import.md)
- [Operations](docs/previa/operations.md)
- [Troubleshooting](docs/previa/troubleshooting.md)
- [CLI specification](docs/specs/previa-v1.md)

Workspace components:

- `previa` - local operations CLI
- `previa-main` - orchestrator API
- `previa-runner` - execution API
- `previa-engine` - pipeline execution core

## License

Previa is released under the MIT License.
