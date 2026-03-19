# Getting Started

This guide covers the shortest path to a working local Previa stack.

## Fast Path

If you want the shortest operator flow, it is:

1. install `previa`
2. start a local stack with `previa up -d`
3. open the IDE with `previa open`
4. create or import a pipeline
5. run E2E or load tests from the IDE, API, or MCP-connected assistant

## Install

On Linux:

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

The installer places `previa` under `~/.previa/bin` and configures
`PREVIA_HOME="$HOME/.previa"`.

## First Local Stack

Start the default context in detached mode:

```bash
previa up --detach
```

Check status:

```bash
previa status
```

Open the UI with your local context:

```bash
previa open
```

This opens:

```text
https://ide.previa.dev?add_context=<your-local-main-url>
```

From there, you can:

- create a project and add specs
- create or import pipelines
- run E2E and load tests
- inspect failures and history

Stop the stack:

```bash
previa down
```

## Work Inside a Repo

Use a project-local runtime home:

```bash
previa --home ./.previa up --detach
previa --home ./.previa status
previa --home ./.previa down
```

This keeps runtime state, logs, and database files inside the repository.

## Optional: Pull a Specific Image Tag

```bash
previa pull all --version 0.0.7
previa up --detach --version 0.0.7
```

By default, `previa up` and `previa pull` use the same version tag as the running `previa` CLI.

## What Gets Created

When you start a detached stack, `previa` writes files under:

```text
$PREVIA_HOME/stacks/<context>/
```

Notably:

- `config/main.env`
- `config/runner.env`
- `data/main/orchestrator.db`
- `run/docker-compose.generated.yaml`
- `run/state.json`

## See Also

- [Home and contexts](./home-and-contexts.md)
- [Up and runtime](./up-and-runtime.md)
- [Release and install](./release-install.md)
- [Operations](./operations.md)
