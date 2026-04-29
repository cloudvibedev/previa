# Operations Cheatsheet

This is a short command reference for day-to-day Previa usage.

## Start

Docker-backed:

```bash
previa up -d
```

Binary-backed:

```bash
previa up -d --bin
```

With a compose source:

```bash
previa up -d .
previa up -d ./previa-compose.yaml
```

## Inspect

```bash
previa status
previa status --json
previa ps
previa logs
previa logs --follow
previa open
```

## Stop and Restart

```bash
previa down
previa restart
previa down --all-contexts
```

## Work With Contexts

```bash
previa list
previa status --context other
previa logs --context other
previa down --context other
```

## Use a Local Home

```bash
previa local up -d
previa local status
previa local down
```

## Import Pipelines

Single file:

```bash
previa up -d --import ./api-smoke.previa.yaml --stack smoke_tests
```

Recursive directory import:

```bash
previa up -d -i ./tests/e2e -r -s app_e2e
```

## Attach a Remote Runner

```bash
RUNNER_AUTH_KEY=shared-secret previa up -d --attach-runner 10.0.0.12:55880
```

## MCP

Enable MCP on `previa-main` and connect your assistant to:

```text
http://localhost:5588/mcp
```

## Common Paths

```text
$PREVIA_HOME/stacks/<context>/config/main.env
$PREVIA_HOME/stacks/<context>/config/runner.env
$PREVIA_HOME/stacks/<context>/logs/main.log
$PREVIA_HOME/stacks/<context>/run/state.json
```

## See Also

- [CLI commands](./cli-commands.md)
- [Operations](./operations.md)
- [Troubleshooting](./troubleshooting.md)
- [Main and runner authentication](./main-runner-auth.md)
