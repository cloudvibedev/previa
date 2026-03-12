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
- `previactl` (CLI para instalar e operar o stack local do Previa via Docker Compose)

Data flow:

```text
main -> runner -> engine
```

## Install

### Linux

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

Direct script URL:

```text
https://downloads.previa.dev/install.sh
```

The installer writes `previactl` to `~/.previa/bin`, sets `PREVIA_HOME="$HOME/.previa"`, and updates `~/.zshrc` and `~/.bashrc` when they exist.

`previactl` release binaries are also published for macOS and Windows. O stack local gerenciado pelo CLI usa as imagens publicadas do `previa-main` e `previa-runner`.

You can also pull published container images with `previactl pull`, for example `previactl pull all` or `previactl pull runner --version 0.0.7`.

## Quick Start

### 1. Start the local stack

```bash
previactl up --detach
```

### 2. Check status and open the UI

```bash
previactl status
previactl open
```

### 3. Optional: pull a specific image tag first

```bash
previactl pull all --version 0.0.7
previactl up --detach --version 0.0.7
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
