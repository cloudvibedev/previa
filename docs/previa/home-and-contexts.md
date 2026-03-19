# Home and Contexts

This guide explains where `previa` stores state and how isolated local runtimes
are organized.

## `PREVIA_HOME`

`previa` stores runtime state, config, logs, and generated compose files under
`PREVIA_HOME`.

Resolution order:

1. `--home <path>`
2. `PREVIA_HOME`
3. `$HOME/.previa`

Examples:

```bash
previa --home ./.previa up --detach
PREVIA_HOME=.previa previa up --detach
```

For a single execution, these are effectively equivalent unless `--home` is
also provided.

## Layout

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

`previa-main` and `previa-runner` appear under `PREVIA_HOME/bin` when `previa up --bin`
downloads or refreshes local runtime binaries for the current CLI version.

## Contexts

A `context` is an isolated local runtime.

Each context has:

- a name such as `default`, `other`, or `staging-local`
- one `previa-main`
- zero or more local runners
- zero or more attached runners
- its own config files, logs, runtime state, and database

If `--context` is omitted, the default is `default`.

Examples:

```bash
previa up --context default --detach
previa up --context other --detach -p 6688 -P 56880:56889
```

## Context-Scoped Environment Files

`previa` guarantees these files exist after a real `up`:

`main.env`

```dotenv
ADDRESS=0.0.0.0
PORT=5588
ORCHESTRATOR_DATABASE_URL=sqlite:///.../orchestrator.db
RUNNER_ENDPOINTS=http://127.0.0.1:55880
RUST_LOG=info
```

`runner.env`

```dotenv
ADDRESS=127.0.0.1
PORT=55880
RUST_LOG=info
```

These live under:

```text
$PREVIA_HOME/stacks/<context>/config/
```

## See Also

- [Getting started](./getting-started.md)
- [Compose source](./compose.md)
- [Troubleshooting](./troubleshooting.md)
