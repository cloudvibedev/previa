# Up and Runtime

This guide covers how `previa up` starts a stack and how runtime behavior is resolved.

## Command Shape

```text
previa up [--context <context>] [SOURCE] [--main-address <addr>] [-p, --main-port <port>] [--runner-address <addr>] [-P, --runner-port-range <start:end>] [--runners <N>] [-a, --attach-runner <selector> ...] [-i, --import <path>] [-r, --recursive] [-s, --stack <name>] [--dry-run] [-d, --detach] [--version <tag>] [--bin]
```

## What `up` Does

- generates a `docker-compose.generated.yaml` per context
- starts exactly one `previa-main`
- can start local runners
- can attach existing remote/local runners by URL
- can run in foreground or detached mode

## Common Examples

Default stack:

```bash
previa up
```

Detached:

```bash
previa up --detach
```

Custom ports and local runners:

```bash
previa up --context other -p 6688 -P 56880:56889 --runners 2
```

Attached runners:

```bash
previa up -a 55880 -a 10.0.0.12:55880
```

Dry run:

```bash
previa up --dry-run
```

## Foreground vs Detached

Foreground mode keeps the command attached to the runtime process lifecycle.

Detached mode:

- runs `docker compose up -d` for compose-backed stacks
- writes `run/state.json`
- allows later use of `status`, `list`, `ps`, `logs`, `restart`, and `down`

Typical detached output:

```text
context 'default' started in detached mode (main: 0.0.0.0:5588)
```

## Local and Attached Runners

You need at least one runner source:

- `--runners > 0`
- at least one `--attach-runner`
- or both

`--attach-runner` accepts:

- `55880` -> `http://127.0.0.1:55880`
- `10.0.0.12:55880` -> `http://10.0.0.12:55880`
- `10.0.0.12` -> `http://10.0.0.12:55880`

## `--bin`

`--bin` starts the locally resolved `previa-main` and `previa-runner` binaries
instead of the published container images.

This is useful for local development, but remember that `previa` resolves
binaries from `PREVIA_HOME/bin` before falling back to workspace targets.
When installed runtime binaries do not match the current CLI version, `previa`
replaces them with matching binaries automatically.

## `RUNNER_AUTH_KEY`

`previa-main` can authenticate requests to runners using the `Authorization`
header with the raw value from `RUNNER_AUTH_KEY`.

Current behavior:

- if `RUNNER_AUTH_KEY` is unset on the runner, requests work as they do today
- if `RUNNER_AUTH_KEY` is set on the runner, it becomes required on:
  - `/health`
  - `/info`
  - `/api/v1/tests/e2e`
  - `/api/v1/tests/load`

For local `previa up`, precedence is:

1. process env `RUNNER_AUTH_KEY`
2. compose env maps
3. existing `main.env` and `runner.env`

Example:

```bash
RUNNER_AUTH_KEY=local-dev-secret previa up --detach
```

The same shared key is used for all runners in one local context.

When no `RUNNER_AUTH_KEY` is configured and the stack uses only local runners,
`previa up` generates a UUID v4 automatically and persists it to the
context-scoped `main.env` and `runner.env` files.

When `--attach-runner` is used, `RUNNER_AUTH_KEY` becomes required. `previa`
must know the shared key up front so `previa-main` can authenticate against the
attached runner endpoints.

## `--version`

For compose-backed runtimes, `--version` selects the container image tag.
If you do not pass `--version`, `previa up` uses the same version as the running CLI:

```bash
previa pull all --version 0.0.7
previa up --detach --version 0.0.7
```

With `--bin`, version overrides are not used.

## Port Conflict Prompts

If a planned local bind is already occupied, `up` prompts for a `+100` shift:

- main port: suggests `-p <port+100>`
- runner range: suggests `-P <start+100:end+100>`

Pressing Enter is treated as `yes`.

## Important Validation Rules

- `--dry-run` cannot be combined with `--detach`
- `main.port` must be between `1` and `65535`
- the runner port range must fit the requested `--runners` count
- `up` fails early if the selected context is already running
- `up` fails early if required local bind targets are already in use

## See Also

- [Compose source](./compose.md)
- [Pipeline import](./pipeline-import.md)
- [Troubleshooting](./troubleshooting.md)
