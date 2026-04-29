# Local Push Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `previa local push` to copy a local project snapshot to a remote Previa main.

**Architecture:** Add a CLI subcommand under `local`, keep HTTP push orchestration in a focused `local_push` module, and reuse existing project export/import/delete APIs. The CLI resolves the local context and prints a concise summary.

**Tech Stack:** Rust 2024, clap derive, reqwest, axum-based unit test mocks.

---

### Task 1: CLI Shape

**Files:**
- Modify: `previa/src/cli.rs`

- [x] Add `LocalCommands::Push(LocalPushArgs)`.
- [x] Add args for `--context`, `--project`, `--to`, `--remote-project-id`, `--overwrite`, and `--include-history`.
- [x] Add parser test for `previa local push --project my_app --to https://remote --overwrite --include-history`.

### Task 2: Push Client

**Files:**
- Create: `previa/src/local_push.rs`
- Modify: `previa/src/lib.rs`

- [x] Resolve local project by ID or exact name.
- [x] Export local project with `includeHistory` based on `--include-history`.
- [x] Resolve remote project by explicit remote ID, local project ID, or exact name.
- [x] Fail if remote exists without `--overwrite`.
- [x] With `--overwrite`, delete the matched remote project and import the local snapshot.
- [x] Print create/replace summary.

### Task 3: Tests And Docs

**Files:**
- Modify: `README.md`
- Modify: `docs/previa/cli-commands.md`
- Modify: `docs/previa/project-repository-workflow.md`

- [x] Add HTTP mock tests for create, conflict without overwrite, and overwrite replacement.
- [x] Document `previa local push` examples and overwrite behavior.
- [x] Run `cargo test -p previa`.
- [x] Run `cargo build --release`.
