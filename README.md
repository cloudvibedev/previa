<p align="center">
  <img src="assets/logo.png" alt="Previa logo" width="220">
</p>

# Previa

## Overview

Previa is a high-speed distributed testing platform built for teams that need confidence before shipping.
It exists to remove friction from API test execution: instead of ad-hoc scripts, fragmented tooling, and slow feedback loops, you get one execution model that scales from local checks to distributed load runs.

In short: Previa helps you move faster with safer releases.

## Start From The UI First

Go to **https://previa.dev** and use the full power of Previa from the UI by running your own runners directly behind it.

1. Launch your runners and orchestrator.
2. Open Previa UI.
3. Point the UI to your server URL.
4. Create projects, pipelines, and execute tests end-to-end.

Previa architecture is composed of three Rust crates:

- `previa-main` (orchestrator API)
- `previa-runner` (remote execution API)
- `previa-engine` (pipeline execution core)
- `previactl` (CLI para instalar e operar os binários do Previa)

Data flow:

```text
main -> runner -> engine
```

## Install

### Linux and macOS

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

Direct script URL:

```text
https://downloads.previa.dev/install.sh
```

The installer writes binaries to `~/.previa/bin`, sets `PREVIA_HOME="$HOME/.previa"`, and updates `~/.zshrc` and `~/.bashrc` when they exist.

## Quick Start

### 1. Start one or more runners

```bash
ADDRESS=0.0.0.0 PORT=55880 cargo run -p previa-runner
```

### 2. Start the orchestrator

```bash
ORCHESTRATOR_DATABASE_URL="sqlite://orchestrator.db" \
RUNNER_ENDPOINTS="http://127.0.0.1:55880" \
ADDRESS=0.0.0.0 PORT=5588 \
cargo run -p previa-main
```

### 3. Connect from Previa UI

Open **https://previa.dev** (fully free UI), add your server URL, and start running tests.

Example server URL:

```text
http://127.0.0.1:5588
```

## Workspace Crates

- [`engine/README.md`](engine/README.md)
- [`runner/README.md`](runner/README.md)
- [`main/README.md`](main/README.md)
- [`docs/previactl-usage.md`](docs/previactl-usage.md)
- [`docs/specs/previactl-v1.md`](docs/specs/previactl-v1.md)

## Local Verification

```bash
cargo check --workspace
cargo test --workspace
```
