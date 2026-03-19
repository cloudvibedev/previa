# Contributing to Previa

Thanks for contributing to Previa.

This guide is intentionally short. It focuses on the project structure,
day-to-day development flow, and the checks expected before changes are merged.

## What You Are Contributing To

Previa is an AI-first QA platform composed of:

- `previa/`: the CLI used to run and operate local stacks
- `main/`: the orchestrator API for projects, specs, pipelines, queues, MCP, and history
- `runner/`: the service that executes E2E and load tests
- `engine/`: the execution core used by the runtime
- `docs/previa/`: operator-facing documentation

## Local Setup

Recommended prerequisites:

- Rust toolchain
- Docker, if you want to test the compose-backed runtime
- a Linux environment if you want to exercise published `--bin` runtime flows

Useful commands:

```bash
cargo build
cargo test
cargo build --release
cargo run -p previa -- --help
cargo run -p previa up --bin
```

For a project-local runtime home:

```bash
previa --home ./.previa up --detach
```

## Architecture Rules

Keep these boundaries intact:

- `routes/` owns HTTP transport, request parsing, and response wiring
- `services/` owns reusable business logic and integrations
- `models/` owns API contracts and DB-facing structs

Do not mix HTTP handling with business logic or persistence structs in the same
module when it can be cleanly separated.

Keep data contracts aligned with the OpenAPI description and the existing model
structs.

## Database and Persistence

When touching persistence:

- prefer SQLx query macros and bound parameters
- reuse existing migrations instead of introducing schema drift in code
- keep database access focused in dedicated modules

## Documentation Expectations

Update documentation when behavior visible to operators or integrators changes.

Typical places to update:

- [README.md](./README.md) for top-level product or onboarding changes
- [docs/previa/README.md](./docs/previa/README.md) for documentation navigation
- the relevant guide under [docs/previa](./docs/previa/README.md) for runtime, MCP, auth, or workflow changes
- [AGENTS.md](./AGENTS.md) when provider or agent workflows change

## Testing and Validation

Before opening or finalizing a change:

- run focused tests for the code you touched
- add regression coverage when fixing a bug
- validate any user-facing documentation examples you changed when practical
- run:

```bash
cargo build --release
```

This repository expects a successful release build at the end of every change.

## Pull Requests

A good pull request should explain:

- what changed
- why it changed
- risks or behavior changes
- how it was validated

If a CLI, API, or runtime workflow changed, include the relevant command,
request, or expected output in the PR description.

For better automatic changelogs, prefer commit messages that roughly follow
conventional prefixes such as:

- `feat:`
- `fix:`
- `docs:`
- `refactor:`
- `perf:`
- `test:`
- `chore:`

## Commit and Push Workflow

Current repository workflow expects contributors to:

1. make the change
2. run `cargo build --release`
3. commit the result
4. push the branch

## Good First Contributions

Good contributions for new contributors include:

- documentation clarity improvements
- CLI UX polish
- regression tests for reported bugs
- consistency fixes across docs, help output, and runtime behavior
