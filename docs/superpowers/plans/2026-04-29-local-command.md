# Local Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `previa local ...` as repository-local sugar for commands that use `--home ./.previa`.

**Architecture:** Extend the `previa` crate CLI parser with a `Local` command that wraps selected existing command argument structs. Normalize `Cli.home` to `./.previa` during dispatch only when no explicit global home was provided.

**Tech Stack:** Rust 2024, clap derive, anyhow, existing `previa` CLI command handlers.

---

### Task 1: CLI Parser And Dispatch

**Files:**
- Modify: `previa/src/cli.rs`
- Modify: `previa/src/lib.rs`
- Test: `previa/src/cli.rs`

- [ ] **Step 1: Write parser tests**

Add tests that parse `previa local up -d`, `previa local status`, and `previa --home ./custom local status`. Assert the parsed command variant and home value.

- [ ] **Step 2: Run parser tests to verify failure**

Run: `cargo test -p previa cli::tests::parses_local_up -- cli::tests::parses_local_status --exact`

Expected: tests fail to compile or fail because `local` does not exist.

- [ ] **Step 3: Implement `LocalCommands`**

Add a `Local(LocalArgs)` top-level variant, define `LocalArgs`, and reuse existing args structs for `up`, `down`, `status`, `logs`, and `open`.

- [ ] **Step 4: Implement local home resolution**

In `previa/src/lib.rs`, before dispatching a local subcommand, set the discovered paths to use `./.previa` only when `cli.home` is `None`.

- [ ] **Step 5: Run parser tests**

Run: `cargo test -p previa cli::tests::parses_local_up cli::tests::parses_local_status`

Expected: parser tests pass.

### Task 2: Documentation

**Files:**
- Modify: `docs/previa/getting-started.md`
- Modify: `docs/previa/cli-commands.md`
- Modify: `README.md`

- [ ] **Step 1: Document the local workflow**

Add `previa local up -d`, `previa local status`, `previa local open`, and `previa local down` examples where project-local `--home ./.previa` is currently documented.

- [ ] **Step 2: Run docs-adjacent verification**

Run: `cargo test -p previa`

Expected: CLI tests pass after docs edits.

### Task 3: Release Verification

**Files:**
- No source edits expected.

- [ ] **Step 1: Run release build**

Run: `cargo build --release`

Expected: release build succeeds.

- [ ] **Step 2: Commit and push**

Run: `git status --short`, stage only related files, commit with `feat: add local cli workflow`, and push the current branch.
