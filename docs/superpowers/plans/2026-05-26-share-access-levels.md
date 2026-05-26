# Share Access Levels Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add viewer, runner, editor, and manager permission levels for stack and pipeline shares.

**Architecture:** Extend existing share enums and reuse the current `access_level` database column. Centralize permission comparison in project and pipeline ACL services, then wire handlers and dialogs to the richer levels.

**Tech Stack:** Rust, Axum, SQLx, SQLite/Postgres-compatible dynamic SQL, React, TypeScript, Vite.

---

### Task 1: Backend Share Level Types

**Files:**
- Modify: `main/src/server/models.rs`
- Modify: `main/src/server/db/project_shares.rs`
- Modify: `main/src/server/db/pipeline_shares.rs`

- [ ] Extend `ProjectShareAccessLevel` and `PipelineShareAccessLevel` with `Viewer`, `Runner`, `Editor`, and `Manager`.
- [ ] Update `Display` and `FromStr` to serialize lowercase strings.
- [ ] Keep default access level as `Editor` for backward compatibility.
- [ ] Keep unknown database values falling back to `Editor`, matching current behavior.

### Task 2: Backend ACL Enforcement

**Files:**
- Modify: `main/src/server/services/project_access.rs`
- Modify: `main/src/server/services/pipeline_access.rs`

- [ ] Replace boolean share checks with share-level lookups.
- [ ] Add helpers that map `Read -> viewer`, `Run -> runner`, `Write -> editor`, `Manage/Delete -> manager`.
- [ ] Add `Run` access variants to project and pipeline access enums.
- [ ] Ensure pipeline access inherits stack share levels.

### Task 3: Handler Permission Mapping

**Files:**
- Modify: `main/src/server/handlers/tests_e2e.rs`
- Modify: `main/src/server/handlers/tests_load.rs`
- Modify: `main/src/server/handlers/runner_reservations.rs`
- Modify: `main/src/server/handlers/pipelines.rs`
- Modify: `main/src/server/handlers/projects.rs`

- [ ] Change execution endpoints from write checks to run checks.
- [ ] Keep edit endpoints using write checks.
- [ ] Keep share, visibility, and delete endpoints using manage/delete checks.

### Task 4: Backend Tests

**Files:**
- Modify: `main/src/server/services/project_access.rs`
- Modify: `main/src/server/services/pipeline_access.rs`

- [ ] Add tests proving each access level grants only the expected operations.
- [ ] Add inherited stack-to-pipeline access assertions.
- [ ] Run targeted tests before implementation and confirm failures, then pass after implementation.

### Task 5: Frontend Share Dialogs

**Files:**
- Modify: `app/src/lib/api-client.ts`
- Modify: `app/src/components/ProjectSharingDialog.tsx`
- Modify: `app/src/components/PipelineSharingDialog.tsx`

- [ ] Update TypeScript union types to `viewer | runner | editor | manager`.
- [ ] Add a level selector when sharing a stack or pipeline.
- [ ] Display each existing share level in the list.
- [ ] Send the selected `accessLevel` in share requests.

### Task 6: Verification and Release

**Commands:**
- `cargo fmt --check`
- `cargo test -p previa-main`
- `npm run build` from `app/`
- `cargo build --release`
- Restart local backend on `127.0.0.1:55988`.
- Smoke test via API that viewer cannot run, runner can run, editor can edit, manager can delete/share.
