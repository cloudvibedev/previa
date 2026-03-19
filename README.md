<p align="center">
  <img src="assets/logo.png" alt="Previa logo" width="220">
</p>

# Previa

**The first AI-First IDE for QA. Test, design, and validate APIs with AI assistance from your desktop, CI/CD, or your favorite AI assistant.**

Previa is a platform for simulating, executing, and tracing real end-to-end API operations so you can see what happened, where a failure occurred, and why.

The `previa` CLI is the local entry point for running a Previa stack on your machine. It starts `previa-main`, manages local and attached `previa-runner` instances, opens the IDE in the browser, and helps you bootstrap projects from pipeline files.

## What Is Previa?

Previa combines local runtime operations with project-scoped API testing workflows:

- `previa` runs and manages the local stack
- `previa-main` is the orchestrator API for projects, specs, pipelines, history, proxying, queues, and MCP
- `previa-runner` executes E2E and load tests
- `previa-engine` resolves templates, performs HTTP steps, and evaluates assertions
- the browser IDE at `https://ide.previa.dev` connects to your local `previa-main`

## Install

Install the CLI with:

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

Today the installer targets Linux and writes `previa` under `~/.previa/bin`, while also setting `PREVIA_HOME="$HOME/.previa"`.

## Quick Start

`-d` is the short form of `--detach`.

Start a Docker-backed stack:

```bash
previa up -d
```

Start a binary-backed stack without Docker:

```bash
previa up -d --bin
```

Inspect the runtime and open the IDE:

```bash
previa status
previa open
```

`previa open` launches:

```text
https://ide.previa.dev?add_context=http%3A%2F%2F127.0.0.1%3A5588
```

## Documentation

Start here for the full documentation hub:

- [Previa docs index](docs/previa/README.md)

Recommended first reads:

- [Getting started](docs/previa/getting-started.md)
- [Minimal happy path](docs/previa/minimal-happy-path.md)
- [Architecture at a glance](docs/previa/architecture.md)
- [Runtime modes](docs/previa/runtime-modes.md)
- [MCP integration](docs/previa/mcp.md)
- [Operations cheatsheet](docs/previa/operations-cheatsheet.md)

## License

Previa is released under the MIT License.
