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
binaries from `PREVIA_HOME/bin` before falling back to workspace targets. Old
installed binaries can therefore shadow newer workspace builds.

## `--version`

For compose-backed runtimes, `--version` selects the container image tag:

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
