<p align="center">
  <img src="assets/logo.png" alt="Previa logo" width="220">
</p>

# Previa

Previa is a local-first CLI for running and operating a Previa stack on your machine.
With `previa` you can start a local `previa-main`, manage local and attached runners,
inspect runtime health, open the UI, and import pipeline files for local testing.

## What You Can Do With `previa`

- Start a local stack with `previa-main` and local runners
- Isolate environments with `context` names and `--home`
- Run in detached mode and inspect state later
- Use a `previa-compose.yaml|yml|json` file as runtime input
- Import local pipeline files with `--import`
- Check status, processes, logs, and open the hosted UI with your local context

## Install

### Linux

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

Direct script URL:

```text
https://downloads.previa.dev/install.sh
```

The installer writes `previa` to `~/.previa/bin`, sets `PREVIA_HOME="$HOME/.previa"`,
and updates `~/.zshrc` and `~/.bashrc` when they exist.

## Quick Start

Start the local stack:

```bash
previa up --detach
```

Check status and open the UI:

```bash
previa status
previa open
```

Use a local runtime home inside your repo:

```bash
previa --home ./.previa up --detach
previa --home ./.previa status
```

Import local pipeline files into a new project:

```bash
previa up --detach --import ./api-smoke.previa.yaml --stack smoke_tests
```

Import a directory recursively:

```bash
previa up --detach -i ./tests/e2e -r -s app_e2e
```

Optionally pull a specific image tag first:

```bash
previa pull all --version 0.0.7
previa up --detach --version 0.0.7
```

## Documentation

Start here for the full CLI docs:

- [Previa CLI docs index](docs/previa/README.md)

Feature guides:

- [Getting started](docs/previa/getting-started.md)
- [Home and contexts](docs/previa/home-and-contexts.md)
- [Compose source](docs/previa/compose.md)
- [Up and runtime](docs/previa/up-and-runtime.md)
- [Pipeline import](docs/previa/pipeline-import.md)
- [Operations](docs/previa/operations.md)
- [Troubleshooting](docs/previa/troubleshooting.md)

Technical reference:

- [CLI specification](docs/specs/previa-v1.md)

## Workspace

Previa is built from four Rust crates:

- `previa` - local operations CLI
- `previa-main` - orchestrator API
- `previa-runner` - execution API
- `previa-engine` - pipeline execution core

Crate READMEs:

- [engine/README.md](engine/README.md)
- [runner/README.md](runner/README.md)
- [main/README.md](main/README.md)

## Local Verification

```bash
cargo check --workspace
cargo test --workspace
```
