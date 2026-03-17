# Troubleshooting

This guide covers common local issues when working with `previa`.

## Context Already Running

Error:

```text
context '<name>' is already running
```

What it means:

- the selected context already has active local processes

What to do:

```bash
previa status --context <name>
previa down --context <name>
```

## No Detached Runtime

Error:

```text
no detached runtime exists for context '<name>'
```

What it means:

- the context was never started with `--detach`
- or it has already been stopped

## Runner Selector Did Not Match

Error:

```text
runner selector '<value>' did not match any local runner
```

What it means:

- the selector used in `status`, `logs`, or `down --runner` did not match any
  local runner in the recorded runtime

## Requested Runner Count Exceeds Port Range

Error:

```text
requested local runner count exceeds the configured port range
```

What it means:

- the configured `-P/--runner-port-range` cannot fit the requested
  `--runners` count

## Old Binaries in `PREVIA_HOME/bin`

This is a common source of confusion when using `--bin` inside the workspace.

`previa` resolves local binaries from:

1. `PREVIA_HOME/bin`
2. workspace `target/debug`
3. workspace `target/release`

So an older installed `previa-main` or `previa-runner` under `PREVIA_HOME/bin`
can shadow a newer workspace build.

Typical workaround:

```bash
cargo build -p previa-main -p previa-runner
previa --home ./.previa-dev up --detach --bin
```

## Pipeline Import Failures

Check these first:

- `--stack` is present
- `--detach` is present
- the file suffix is one of `.previa`, `.previa.json`, `.previa.yaml`, `.previa.yml`
- recursive mode points to a directory
- non-recursive mode points to a file
- the file content is a direct `Pipeline` object

If import fails after startup, the runtime remains running. You can inspect it
with:

```bash
previa status
previa logs
```

## Path and Home Confusion

To isolate everything inside a repo, prefer:

```bash
previa --home ./.previa up --detach
```

This avoids mixing project-local experimentation with the default
`$HOME/.previa`.

## See Also

- [Home and contexts](./home-and-contexts.md)
- [Up and runtime](./up-and-runtime.md)
- [Pipeline import](./pipeline-import.md)
