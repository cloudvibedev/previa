# CLI Commands

This guide explains every command currently exposed by the `previa` CLI.

For fast day-to-day usage, see [Operations cheatsheet](./operations-cheatsheet.md). For deeper runtime behavior, see [Up and runtime](./up-and-runtime.md) and [Operations](./operations.md).

## Command Overview

Top-level help:

```bash
previa --help
previa help
previa help up
```

Current commands:

- `up`
- `pull`
- `down`
- `restart`
- `status`
- `list`
- `ps`
- `logs`
- `open`
- `version`
- `help`

## Global Option

`previa` supports one global option:

```text
--home <PATH>
```

Resolution order for the runtime home is:

1. `--home <PATH>`
2. `PREVIA_HOME`
3. `$HOME/.previa`

Example:

```bash
previa --home ./.previa up -d
previa --home ./.previa status
```

## `previa up`

Starts a Previa stack for one context.

```text
previa up [OPTIONS] [SOURCE]
```

Main uses:

- start a Docker-backed stack
- start a binary-backed stack with `--bin`
- apply a `previa-compose` source
- attach remote runners
- import pipelines after startup

Important options:

- `--context <CONTEXT>`: selects the context name, default `default`
- `[SOURCE]`: compose source directory or file
- `--main-address <ADDR>` and `--main-port <PORT>`: override main bind target
- `--runner-address <ADDR>` and `--runner-port-range <START:END>`: override local runner binds
- `--runners <N>`: number of local runners to start
- `--attach-runner <RUNNER>`: attach an existing runner endpoint; may be repeated
- `--import <PATH>`: import one file or a directory of pipeline files after startup
- `--recursive`: required when recursively importing a directory
- `--stack <STACK>`: required when using `--import`
- `--dry-run`: prints the planned runtime without starting it
- `-d, --detach`: starts the stack in detached mode
- `--bin`: uses local binaries instead of container images
- `--version <TAG>`: image tag for compose-backed runtimes, default is the current CLI version

Examples:

```bash
previa up
previa up -d
previa up -d --bin
previa up ./
previa up ./previa-compose.yaml
previa up --context other -p 6688 -P 56880:56889 --runners 2
previa up -d --attach-runner 10.0.0.12:55880
previa up -d --import ./tests/e2e -r --stack app_e2e
previa up --dry-run
```

Notes:

- detached mode writes runtime state and unlocks `status`, `logs`, `ps`, `restart`, and `down`
- `--dry-run` cannot be combined with `--detach`
- `--version` is not used with `--bin`
- when `--bin` cannot find local runtime binaries, `previa` can bootstrap them into `PREVIA_HOME/bin`
- when only local runners are used and `RUNNER_AUTH_KEY` is missing, `previa up` generates one automatically
- when `--attach-runner` is used, `RUNNER_AUTH_KEY` is required

See also:

- [Up and runtime](./up-and-runtime.md)
- [Compose source](./compose.md)
- [Pipeline import](./pipeline-import.md)
- [Main and runner authentication](./main-runner-auth.md)

## `previa pull`

Pulls published runtime images.

```text
previa pull [OPTIONS] [TARGET]
```

Targets:

- `main`
- `runner`
- `all` (default)

Important options:

- `--version <TAG>`: image tag to pull, default is the current CLI version

Examples:

```bash
previa pull
previa pull main
previa pull runner --version 0.0.7
previa pull all
```

This command is mainly useful for compose-backed runtimes.

## `previa down`

Stops a detached context, or selected local runners inside it.

```text
previa down [OPTIONS]
```

Important options:

- `--context <CONTEXT>`: context to stop, default `default`
- `--all-contexts`: stops every detached context under `PREVIA_HOME/stacks`
- `--runner <RUNNER>`: stops only the selected local runner; may be repeated

Examples:

```bash
previa down
previa down --context other
previa down --runner 55880
previa down --all-contexts
```

Notes:

- `--all-contexts` and `--runner` are mutually exclusive
- attached runners are never stopped by `previa`
- `--runner` only affects locally recorded runners
- removing the last local runner fails if no attached runner remains

## `previa restart`

Restarts a detached context using its saved runtime configuration.

```text
previa restart [OPTIONS]
```

Important options:

- `--context <CONTEXT>`: context to restart, default `default`
- `--version <TAG>`: only supported for compose-backed runtimes

Examples:

```bash
previa restart
previa restart --context other
previa restart --version 0.0.7
```

Notes:

- restart requires an existing detached context
- for `--bin`, restart ignores image tags and reuses the saved local runtime shape

## `previa status`

Shows the current health and state for one context.

```text
previa status [OPTIONS]
```

Important options:

- `--context <CONTEXT>`: context to inspect, default `default`
- `--main`: show only the main process
- `--runner <RUNNER>`: show only the selected runner
- `--json`: render machine-readable output

Examples:

```bash
previa status
previa status --main
previa status --runner 55880
previa status --json
```

Notes:

- `--main` and `--runner` are mutually exclusive
- state is derived from runtime metadata plus health probing when possible

## `previa list`

Lists every known context under `PREVIA_HOME/stacks`.

```text
previa list [OPTIONS]
```

Important options:

- `--json`: render machine-readable output

Examples:

```bash
previa list
previa list --json
```

Typical output shows the context name, current state, and backing runtime file.

## `previa ps`

Shows recorded local process metadata for one context.

```text
previa ps [OPTIONS]
```

Important options:

- `--context <CONTEXT>`: context to inspect, default `default`
- `--json`: render machine-readable output

Examples:

```bash
previa ps
previa ps --context other
previa ps --json
```

Typical fields include role, pid, state, address, port, health URL, and log path.

## `previa logs`

Reads logs from a detached runtime.

```text
previa logs [OPTIONS]
```

Important options:

- `--context <CONTEXT>`: context to inspect, default `default`
- `--main`: show only main logs
- `--runner <RUNNER>`: show only one runner log
- `--follow`: stream logs
- `-t, --tail [<N>]`: tail mode; when used without a value it defaults to `10`

Examples:

```bash
previa logs
previa logs --main
previa logs --runner 55880
previa logs --follow
previa logs -t
previa logs --tail 50
```

Notes:

- `--main` and `--runner` are mutually exclusive
- `-t 0` is invalid
- without filters, `previa` shows main plus all local runners
- for compose-backed runtimes, logs come from Docker Compose
- for binary-backed runtimes, logs come from files under the context log directory

## `previa open`

Opens the hosted Previa IDE in the browser with the current context attached.

```text
previa open [OPTIONS]
```

Important options:

- `--context <CONTEXT>`: context to open, default `default`

Examples:

```bash
previa open
previa open --context other
```

Runtime behavior:

- builds a URL like `https://ide.previa.dev?add_context=<main-url>`
- opens the default browser
- prints the final URL to stdout

If the main runtime is bound to `0.0.0.0` or `::`, `previa` normalizes it to loopback for the browser URL.

## `previa version`

Prints the compiled CLI version.

```text
previa version
previa --version
```

Examples:

```bash
previa version
previa --version
```

## `previa help`

Shows built-in command help from the CLI parser.

```text
previa help
previa help up
previa help logs
```

This is the fastest way to confirm the exact flags supported by the binary you are running.

## See Also

- [Getting started](./getting-started.md)
- [Operations cheatsheet](./operations-cheatsheet.md)
- [Operations](./operations.md)
- [Up and runtime](./up-and-runtime.md)
