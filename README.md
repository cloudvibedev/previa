<p align="center">
  <img src="assets/logo.png" alt="Previa logo" width="220">
</p>

# Previa

[![Release](https://img.shields.io/github/v/release/cloudvibedev/previa?display_name=tag)](https://github.com/cloudvibedev/previa/releases)
[![Build](https://img.shields.io/github/actions/workflow/status/cloudvibedev/previa/release.yaml?branch=main&label=build)](https://github.com/cloudvibedev/previa/actions/workflows/release.yaml)
[![License](https://img.shields.io/github/license/cloudvibedev/previa)](https://github.com/cloudvibedev/previa/blob/main/LICENSE)
[![Stars](https://img.shields.io/github/stars/cloudvibedev/previa?style=social)](https://github.com/cloudvibedev/previa/stargazers)

**The first AI-First IDE for QA. Test, design, and validate APIs with AI assistance from your desktop, CI/CD, or your favorite AI assistant.**

Previa is a platform for simulating, executing, and tracing real end-to-end API operations so you can see what happened, where a failure occurred, and why.

The `previa` CLI is the local entry point for running a Previa stack on your machine. It starts `previa-main`, manages local and attached `previa-runner` instances, opens the IDE in the browser, and helps you bootstrap projects from pipeline files.

## What Is Previa?

Previa combines local runtime operations with project-scoped API testing workflows:

- `previa` runs and manages the local stack
- `previa-main` is the orchestrator API for projects, specs, pipelines, history, proxying, queues, and MCP
- `previa-runner` executes E2E and load tests
- the browser IDE at `https://ide.previa.dev` connects to your local `previa-main`

In practice, the flow looks like this:

```text
previa CLI -> previa-main -> previa-runner -> target API
```

## Why Previa Exists

I created Previa to make end-to-end and load testing simple enough for any AI to understand, generate, and execute through pipelines. The goal was to build a testing system that could become the preferred runtime for AI-first development workflows.

I have been building with an AI-first mindset since 2025, when I started using tools like Codex and Claude Code heavily in day-to-day development. Over time, I kept running into the same bottleneck: tests were often missing, brittle, misleading, or easy for AI assistants to fake with weak assertions that looked correct but did not really protect real user flows.

That led to a simple idea: end-to-end testing should live outside the application as an independent runtime that any team, developer, or AI assistant can use to verify whether a real workflow broke. And once that runtime already understands the system, it should also make load testing just as easy, whether through a few clicks in the IDE or a prompt sent from an AI assistant.

*Philippe Assis*

## AI-First Workflow

Previa is designed to work well in AI-first development loops:

- the CLI starts a real test runtime outside your application codebase
- the IDE gives you a visual place to inspect specs, pipelines, executions, and failures
- the HTTP API lets CI/CD and automation trigger the same workflows
- the MCP server lets assistants inspect, generate, validate, and troubleshoot using the same runtime

The main idea is simple: your assistant should not have to guess whether a workflow still works. It should be able to ask Previa to run it.

## Install

Install the CLI with:

```bash
curl -fsSL https://downloads.previa.dev/install.sh | sh
```

Today the installer targets Linux and writes `previa` under `~/.previa/bin`, while also setting `PREVIA_HOME="$HOME/.previa"`.

## Quick Start

`-d` is the short form of `--detach`.

The shortest happy path is:

```text
install -> up -> open -> create or import a pipeline -> run tests
```

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
- [Release and install](docs/previa/release-install.md)
- [MCP integration](docs/previa/mcp.md)
- [Operations cheatsheet](docs/previa/operations-cheatsheet.md)
- [Contributing](CONTRIBUTING.md)

## License

Previa is released under the MIT License.
