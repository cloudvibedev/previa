# Previa CLI Documentation

This section documents the `previa` CLI as an operator-facing tool for local
stack management, runtime inspection, and pipeline import.

## Concepts

- `context`: an isolated local runtime managed by `previa`
- `PREVIA_HOME`: the root directory where `previa` stores runtime state
- `stack`: the running local Previa runtime for a given context
- `compose source`: a `previa-compose.yaml|yml|json` file used as input to `up`
- `pipeline import`: loading `.previa*` pipeline files into a local project

## Guides

- [Architecture at a glance](./architecture.md)
- [Getting started](./getting-started.md)
- [Minimal happy path](./minimal-happy-path.md)
- [Runtime modes](./runtime-modes.md)
- [Main and runner authentication](./main-runner-auth.md)
- [Remote runners](./remote-runners.md)
- [MCP integration](./mcp.md)
- [E2E queues](./e2e-queues.md)
- [Home and contexts](./home-and-contexts.md)
- [Compose source](./compose.md)
- [Up and runtime](./up-and-runtime.md)
- [Pipeline import](./pipeline-import.md)
- [Operations](./operations.md)
- [Troubleshooting](./troubleshooting.md)

## Command Summary

```text
previa --home <path> <COMMAND>
previa up [OPTIONS] [SOURCE]
previa pull [main|runner|all] [--version <version>]
previa down [OPTIONS]
previa restart [OPTIONS]
previa status [OPTIONS]
previa list [OPTIONS]
previa ps [OPTIONS]
previa logs [OPTIONS]
previa open [OPTIONS]
previa version
previa --version
```

## Technical Reference

- [CLI specification](../specs/previa-v1.md)
- [Repository README](../../README.md)

## See Also

- [Getting started](./getting-started.md)
- [Architecture at a glance](./architecture.md)
- [Operations](./operations.md)
