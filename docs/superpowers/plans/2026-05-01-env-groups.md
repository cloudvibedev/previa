# Env Groups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add project-level environment groups that resolve with `{{envs.<group>.<env>}}`, can be selected before E2E/load execution, and coexist with existing OpenAPI specs.

**Architecture:** Keep runtime configuration separate from OpenAPI specs. Store env groups under the project, expose CRUD routes under `/api/v1/projects/{projectId}/env-groups`, pass env groups to E2E/load execution requests, and extend the engine template context with an `envs` namespace. Preserve `{{specs.<slug>.url.<name>}}` and legacy `{{url.<slug>.<name>}}` behavior.

**Tech Stack:** Rust/Axum/SQLx for the orchestrator and runner, `previa-engine` for template resolution, React/TypeScript/Zustand for the IDE, Vitest and Rust unit/integration tests.

---

## File Structure

- Create `main/src/server/db/env_groups.rs`: SQLx CRUD for project env groups and entries.
- Create `main/src/server/handlers/env_groups.rs`: HTTP handlers for listing, creating, updating, and deleting env groups.
- Create `main/src/server/validation/env_groups.rs`: slug/name/url validation shared by handlers and DB-facing payload normalization.
- Create `main/migrations/sqlite/202605010001_add_env_groups.sql`: SQLite schema.
- Create `main/migrations/postgres/202605010001_add_env_groups.sql`: Postgres schema.
- Modify `main/src/server/models.rs`: request/response models and execution request payloads.
- Modify `main/src/server/db/mod.rs`, `main/src/server/handlers/mod.rs`, `main/src/server/validation/mod.rs`, `main/src/server/mod.rs`, `main/src/server/docs.rs`: module exports, routes, and OpenAPI docs.
- Modify `main/src/server/execution/runtime_specs.rs`: load runtime env groups for project executions.
- Modify `main/src/server/execution/e2e.rs`, `main/src/server/execution/e2e_queue.rs`, `main/src/server/execution/load.rs`: include env groups in runtime requests.
- Modify `main/src/server/validation/pipelines.rs`: validate `{{envs.<group>.<env>}}` references.
- Modify `engine/src/core/types.rs`, `engine/src/template/resolve.rs`, `engine/src/execution/engine.rs`, `engine/src/lib.rs`: add `RuntimeEnvGroup` and render `envs`.
- Modify `runner/src/server/handlers/e2e.rs` and load-test runner request handling if present: pass env groups to engine.
- Modify `app/src/types/project.ts`: add `ProjectEnvGroup`.
- Modify `app/src/lib/api-client.ts`: env group API methods and execution payload fields.
- Modify `app/src/stores/useProjectStore.ts`: load and mutate env groups alongside specs/pipelines.
- Modify `app/src/stores/useExecutionHistoryStore.ts`, `app/src/stores/useLoadTestHistoryStore.ts`, `app/src/lib/remote-executor.ts`: pass selected env group context.
- Modify `app/src/pages/TestExecutionPage.tsx` and `app/src/components/LoadTestConfigPanel.tsx`: add env selection controls.
- Modify `app/src/components/StepCreatorPanel.tsx`, `app/src/lib/template-validator.ts`, `app/src/lib/monaco-template-setup.ts`, `app/src/components/PipelineDocsPanel.tsx`, `app/src/components/AIPipelineChat.tsx`: teach authoring tools about `envs`.

## Data Model

Use project-level groups keyed by slug:

```json
{
  "id": "uuid-v7",
  "projectId": "project-id",
  "slug": "payments",
  "name": "Payments",
  "entries": [
    { "name": "local", "url": "http://localhost:3000", "description": "Local API" },
    { "name": "hml", "url": "https://payments-hml.example.com", "description": null }
  ],
  "createdAt": "2026-05-01T12:00:00Z",
  "updatedAt": "2026-05-01T12:00:00Z"
}
```

Runtime representation:

```rust
pub struct RuntimeEnvGroup {
    pub slug: String,
    pub urls: HashMap<String, String>,
}
```

Template syntax:

```text
{{envs.payments.local}}/charges
{{envs.payments.hml}}/charges
```

## Task 1: Engine Runtime Support

**Files:**
- Modify `engine/src/core/types.rs`
- Modify `engine/src/template/resolve.rs`
- Modify `engine/src/execution/engine.rs`
- Modify `engine/src/lib.rs`
- Test in `engine/src/template/resolve.rs`
- Test in `engine/src/execution/engine.rs`

- [ ] **Step 1: Add the runtime type**

Add next to `RuntimeSpec` in `engine/src/core/types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RuntimeEnvGroup {
    pub slug: String,
    #[serde(default)]
    pub urls: HashMap<String, String>,
}
```

- [ ] **Step 2: Export the type**

Update `engine/src/lib.rs` exports from:

```rust
pub use core::types::{
    AssertionResult, Pipeline, PipelineStep, RuntimeSpec, StepAssertion, StepExecutionResult,
    StepRequest, StepResponse,
};
```

to:

```rust
pub use core::types::{
    AssertionResult, Pipeline, PipelineStep, RuntimeEnvGroup, RuntimeSpec, StepAssertion,
    StepExecutionResult, StepRequest, StepResponse,
};
```

- [ ] **Step 3: Write failing template tests**

Add tests in `engine/src/template/resolve.rs`:

```rust
#[test]
fn resolves_env_url_variable() {
    let envs = [RuntimeEnvGroup {
        slug: "payments".to_owned(),
        urls: HashMap::from([("hml".to_owned(), "https://hml.payments.example".to_owned())]),
    }];
    let context = build_template_context(&HashMap::new(), None, Some(&envs));
    let rendered = resolve_template_variables_with_context(
        &Value::String("{{envs.payments.hml}}/charges".to_owned()),
        &context,
    );
    assert_eq!(
        rendered,
        Value::String("https://hml.payments.example/charges".to_owned())
    );
}
```

Expected first run: compile failure because `RuntimeEnvGroup` and the new `build_template_context` signature do not exist.

- [ ] **Step 4: Extend template context**

Change `build_template_context` to accept env groups:

```rust
pub(crate) fn build_template_context(
    steps: &HashMap<String, StepExecutionResult>,
    specs: Option<&[RuntimeSpec]>,
    env_groups: Option<&[RuntimeEnvGroup]>,
) -> Value {
    let mut root = Map::new();
    // keep existing steps and specs blocks unchanged

    let mut envs_map = Map::new();
    if let Some(env_groups) = env_groups {
        for group in env_groups {
            let slug = group.slug.trim();
            if slug.is_empty() {
                continue;
            }
            let mut urls_map = Map::new();
            for (name, url) in &group.urls {
                let name = name.trim();
                let url = url.trim();
                if !name.is_empty() && !url.is_empty() {
                    urls_map.insert(name.to_owned(), Value::String(url.to_owned()));
                }
            }
            envs_map.insert(slug.to_owned(), Value::Object(urls_map));
        }
    }
    root.insert("envs".to_owned(), Value::Object(envs_map));

    Value::Object(root)
}
```

Update all callers. Existing public helpers should pass `None` for env groups to avoid breaking simple rendering APIs.

- [ ] **Step 5: Thread env groups through execution**

Add env group parameters to the specs execution path in `engine/src/execution/engine.rs`:

```rust
pub async fn execute_pipeline_with_runtime_hooks<FStart, FResult, FCancel>(
    pipeline: &Pipeline,
    selected_base_url_key: Option<&str>,
    specs: Option<&[RuntimeSpec]>,
    env_groups: Option<&[RuntimeEnvGroup]>,
    on_step_start: FStart,
    on_step_result: FResult,
    should_cancel: FCancel,
) -> Vec<StepExecutionResult>
```

Keep `execute_pipeline_with_specs_hooks` as a compatibility wrapper that calls the new function with `env_groups: None`.

- [ ] **Step 6: Verify engine tests**

Run:

```bash
cargo test -p previa-engine env
cargo test -p previa-engine resolves_spec_url_variable
```

Expected: all selected tests pass.

## Task 2: Orchestrator Models, Validation, and Persistence

**Files:**
- Create `main/src/server/validation/env_groups.rs`
- Create `main/src/server/db/env_groups.rs`
- Modify `main/src/server/models.rs`
- Modify `main/src/server/db/mod.rs`
- Modify `main/src/server/validation/mod.rs`
- Create migrations in `main/migrations/sqlite/` and `main/migrations/postgres/`

- [ ] **Step 1: Add migrations**

Create SQLite migration:

```sql
CREATE TABLE IF NOT EXISTS project_env_groups (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    entries_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL DEFAULT 0,
    updated_at_ms INTEGER NOT NULL DEFAULT 0,
    UNIQUE(project_id, slug),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
```

Create Postgres migration:

```sql
CREATE TABLE IF NOT EXISTS project_env_groups (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    entries_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_at_ms BIGINT NOT NULL DEFAULT 0,
    updated_at_ms BIGINT NOT NULL DEFAULT 0,
    UNIQUE(project_id, slug),
    FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
```

- [ ] **Step 2: Add models**

Add to `main/src/server/models.rs`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvGroupEntry {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectEnvGroupUpsertRequest {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub entries: Vec<EnvGroupEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEnvGroupRecord {
    pub id: String,
    pub project_id: String,
    pub slug: String,
    pub name: String,
    pub entries: Vec<EnvGroupEntry>,
    pub created_at: String,
    pub updated_at: String,
}
```

Also add `#[serde(default)] pub env_groups: Vec<RuntimeEnvGroup>` to `LoadTestRequest`, `E2eTestRequest`, `ProjectE2eTestRequest`, `ProjectE2eQueueRequest`, and `ProjectLoadTestRequest`.

- [ ] **Step 3: Add validation**

Create `main/src/server/validation/env_groups.rs`:

```rust
use std::collections::HashSet;

use crate::server::models::{EnvGroupEntry, ProjectEnvGroupUpsertRequest};

pub fn normalize_env_group_payload(
    mut payload: ProjectEnvGroupUpsertRequest,
) -> Result<ProjectEnvGroupUpsertRequest, &'static str> {
    payload.slug = normalize_slug(&payload.slug)?;
    payload.name = payload.name.trim().to_owned();
    if payload.name.is_empty() {
        return Err("env group name is required");
    }
    payload.entries = normalize_entries(payload.entries)?;
    Ok(payload)
}

pub fn normalize_slug(raw: &str) -> Result<String, &'static str> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("env group slug is required");
    }
    if value.starts_with('-') || value.ends_with('-') || value.contains("--") {
        return Err("env group slug cannot start/end with '-' or contain repeated separators");
    }
    if !value.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
        return Err("env group slug must use lowercase letters, numbers, or '-'");
    }
    Ok(value.to_owned())
}

pub fn normalize_entries(entries: Vec<EnvGroupEntry>) -> Result<Vec<EnvGroupEntry>, &'static str> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(entries.len());
    for entry in entries {
        let name = entry.name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("env entries[].name is required");
        }
        if !name.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_') {
            return Err("env entries[].name must use lowercase letters, numbers, '-' or '_'");
        }
        if !seen.insert(name.clone()) {
            return Err("env entries[].name must be unique");
        }
        let url = entry.url.trim().to_owned();
        if url.is_empty() {
            return Err("env entries[].url is required");
        }
        normalized.push(EnvGroupEntry {
            name,
            url,
            description: entry.description.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned),
        });
    }
    Ok(normalized)
}
```

- [ ] **Step 4: Add DB CRUD**

Create `main/src/server/db/env_groups.rs` using the same transaction and timestamp pattern as `main/src/server/db/specs.rs`. Required functions:

```rust
pub fn project_env_group_from_row(row: &sqlx::any::AnyRow) -> ProjectEnvGroupRecord
pub async fn list_project_env_group_records(db: &DbPool, project_id: &str) -> Result<Vec<ProjectEnvGroupRecord>, sqlx::Error>
pub async fn load_project_env_group_record_by_id(db: &DbPool, project_id: &str, group_id: &str) -> Result<Option<ProjectEnvGroupRecord>, sqlx::Error>
pub async fn insert_project_env_group_record(db: &DbPool, project_id: &str, payload: ProjectEnvGroupUpsertRequest) -> Result<ProjectEnvGroupRecord, sqlx::Error>
pub async fn update_project_env_group_record(db: &DbPool, project_id: &str, group_id: &str, payload: ProjectEnvGroupUpsertRequest) -> Result<Option<ProjectEnvGroupRecord>, sqlx::Error>
pub async fn delete_project_env_group_record(db: &DbPool, project_id: &str, group_id: &str) -> Result<bool, sqlx::Error>
```

- [ ] **Step 5: Verify DB tests**

Add Rust tests in `main/src/server/db/env_groups.rs` for insert/list/update/delete and unique slug rejection.

Run:

```bash
cargo test -p previa-main env_group
```

Expected: env group DB and validation tests pass.

## Task 3: Orchestrator API and Runtime Resolution

**Files:**
- Create `main/src/server/handlers/env_groups.rs`
- Modify `main/src/server/mod.rs`
- Modify `main/src/server/handlers/mod.rs`
- Modify `main/src/server/docs.rs`
- Modify `main/src/server/execution/runtime_specs.rs`
- Modify E2E/load execution modules.

- [ ] **Step 1: Add HTTP handlers**

Create handlers mirroring `main/src/server/handlers/specs.rs`:

```rust
pub async fn list_project_env_groups(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<ProjectEnvGroupRecord>>, StatusCode>

pub async fn create_project_env_group(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(payload): Json<ProjectEnvGroupUpsertRequest>,
) -> Result<Json<ProjectEnvGroupRecord>, (StatusCode, Json<ErrorResponse>)>

pub async fn get_project_env_group(...)
pub async fn upsert_project_env_group(...)
pub async fn delete_project_env_group(...)
```

Validation errors should return `400`, not `500`.

- [ ] **Step 2: Register routes**

Add to `main/src/server/mod.rs`:

```rust
.route(
    "/api/v1/projects/{projectId}/env-groups",
    get(list_project_env_groups).post(create_project_env_group),
)
.route(
    "/api/v1/projects/{projectId}/env-groups/{envGroupId}",
    get(get_project_env_group)
        .put(upsert_project_env_group)
        .delete(delete_project_env_group),
)
```

- [ ] **Step 3: Load runtime env groups for executions**

Extend `main/src/server/execution/runtime_specs.rs` or split it into a neutral runtime context module:

```rust
pub async fn load_runtime_env_groups_for_project(
    db: &DbPool,
    project_id: &str,
) -> Result<Vec<RuntimeEnvGroup>, sqlx::Error>

pub async fn resolve_runtime_env_groups_for_execution(
    db: &DbPool,
    project_id: Option<&str>,
    payload_env_groups: &[RuntimeEnvGroup],
) -> Result<Option<Vec<RuntimeEnvGroup>>, sqlx::Error>
```

Payload env groups win over stored project env groups, matching the current specs behavior.

- [ ] **Step 4: Pass env groups into execution**

For E2E, queue, and load handlers, resolve env groups and call the new engine/runtime API:

```rust
let runtime_env_groups = resolve_runtime_env_groups_for_execution(
    &state.db,
    Some(&project_id),
    &payload.env_groups,
).await?;
```

Then pass `runtime_env_groups.as_deref()` to runner/engine execution request structs.

- [ ] **Step 5: Add API tests**

Add tests beside existing handler tests to cover:

- `POST /api/v1/projects/{projectId}/env-groups` creates a group.
- `GET /api/v1/projects/{projectId}/env-groups` lists it.
- duplicate slug returns non-success.
- E2E execution accepts `envGroups` payload.

Run:

```bash
cargo test -p previa-main env_groups
cargo test -p previa-main e2e
```

## Task 4: Pipeline Template Validation

**Files:**
- Modify `main/src/server/validation/pipelines.rs`
- Modify `app/src/lib/template-validator.ts`
- Modify `app/src/lib/monaco-template-setup.ts`

- [ ] **Step 1: Add backend validation tests**

Add test:

```rust
#[test]
fn accepts_known_env_references() {
    let pipeline = Pipeline {
        id: None,
        name: "Env pipeline".to_owned(),
        description: None,
        steps: vec![sample_step("health", "{{envs.payments.hml}}/health")],
    };
    let env_groups = vec![RuntimeEnvGroup {
        slug: "payments".to_owned(),
        urls: HashMap::from([("hml".to_owned(), "https://hml.example.com".to_owned())]),
    }];
    let errors = validate_pipeline_templates(&pipeline, None, Some(&env_groups));
    assert!(errors.is_empty(), "{errors:?}");
}
```

- [ ] **Step 2: Implement backend validation**

Extend `validate_pipeline_templates` to accept `env_groups: Option<&[RuntimeEnvGroup]>`, build an env index, and validate:

```text
envs.<group>.<env>
```

Error messages:

```text
template variable '{{envs.payments}}' must use the format '{{envs.<group>.<env>}}'
template variable '{{envs.unknown.hml}}' references unknown env group 'unknown'
template variable '{{envs.payments.dev}}' references unknown env 'dev' for group 'payments'
```

- [ ] **Step 3: Update frontend validator**

In `app/src/lib/template-validator.ts`, add `"envs"` to valid namespaces and validate three segments exactly.

- [ ] **Step 4: Update Monaco completions**

Add completions:

```text
envs
envs.<group>
envs.<group>.<env>
```

Use `availableEnvGroups` from the template context, parallel to `availableSpecs`.

## Task 5: Frontend Data Loading and CRUD

**Files:**
- Modify `app/src/types/project.ts`
- Modify `app/src/lib/api-client.ts`
- Modify `app/src/stores/useProjectStore.ts`
- Add tests where existing project store/API client tests live.

- [ ] **Step 1: Add frontend types**

Add:

```ts
export interface ProjectEnvEntry {
  name: string;
  url: string;
  description?: string | null;
}

export interface ProjectEnvGroup {
  id: string;
  projectId: string;
  slug: string;
  name: string;
  entries: ProjectEnvEntry[];
  createdAt: string;
  updatedAt: string;
}
```

Add `envGroups: ProjectEnvGroup[]` to `Project`.

- [ ] **Step 2: Add API client methods**

Add:

```ts
export interface ProjectEnvGroupUpsertRequest {
  slug: string;
  name: string;
  entries: ProjectEnvEntry[];
}

export async function listProjectEnvGroups(baseUrl: string, projectId: string): Promise<ProjectEnvGroup[]> {
  return request<ProjectEnvGroup[]>(`${ensureApiPrefix(baseUrl)}/projects/${projectId}/env-groups`);
}

export async function createProjectEnvGroup(baseUrl: string, projectId: string, data: ProjectEnvGroupUpsertRequest): Promise<ProjectEnvGroup> {
  return request<ProjectEnvGroup>(`${ensureApiPrefix(baseUrl)}/projects/${projectId}/env-groups`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  });
}
```

Also add `getProjectEnvGroup`, `updateProjectEnvGroup`, and `deleteProjectEnvGroup`.

- [ ] **Step 3: Load env groups with project details**

In `getProject`, extend the parallel load:

```ts
const [record, pipelines, specs, envGroups] = await Promise.all([
  getProjectRecord(baseUrl, id),
  listProjectPipelines(baseUrl, id),
  listProjectSpecs(baseUrl, id),
  listProjectEnvGroups(baseUrl, id),
]);
```

- [ ] **Step 4: Add store actions**

Add actions to `useProjectStore`:

```ts
createEnvGroup(projectId: string, payload: ProjectEnvGroupUpsertRequest): Promise<ProjectEnvGroup>
updateEnvGroup(projectId: string, groupId: string, payload: ProjectEnvGroupUpsertRequest): Promise<ProjectEnvGroup>
deleteEnvGroup(projectId: string, groupId: string): Promise<void>
```

Update `currentProject.envGroups` and project list metadata after each mutation.

## Task 6: Execution Selection UX

**Files:**
- Modify `app/src/pages/TestExecutionPage.tsx`
- Modify `app/src/components/LoadTestConfigPanel.tsx`
- Modify `app/src/components/LoadTestTab.tsx`
- Modify `app/src/stores/useExecutionHistoryStore.ts`
- Modify `app/src/stores/useLoadTestHistoryStore.ts`
- Modify `app/src/lib/remote-executor.ts`

- [ ] **Step 1: Define selected env state**

In `TestExecutionPage`, add:

```ts
const [selectedEnvGroupSlug, setSelectedEnvGroupSlug] = useState<string | null>(null);
const [selectedEnvName, setSelectedEnvName] = useState<string | null>(null);
```

Default to the first group and first entry when available.

- [ ] **Step 2: Add selector**

Add compact controls near the Run button and batch controls:

```tsx
<Select value={selectedEnvGroupSlug ?? ""} onValueChange={setSelectedEnvGroupSlug}>
  {envGroups.map((group) => (
    <SelectItem key={group.slug} value={group.slug}>{group.name}</SelectItem>
  ))}
</Select>
<Select value={selectedEnvName ?? ""} onValueChange={setSelectedEnvName}>
  {selectedGroup?.entries.map((entry) => (
    <SelectItem key={entry.name} value={entry.name}>{entry.name}</SelectItem>
  ))}
</Select>
```

If no env groups exist, hide the selector and keep absolute URLs/spec templates working.

- [ ] **Step 3: Pass env groups to E2E execution**

Convert project env groups to runtime payload:

```ts
const runtimeEnvGroups = envGroups.map((group) => ({
  slug: group.slug,
  urls: Object.fromEntries(group.entries.map((entry) => [entry.name, entry.url])),
}));
```

Pass `envGroups: runtimeEnvGroups` in `runRemoteIntegrationTest` and `createE2eQueue`.

- [ ] **Step 4: Use selected env for placeholder templates**

The selection should not rewrite pipeline URLs at rest. It should provide default execution context and UI hints. For v1, `{{envs.<group>.<env>}}` resolves directly; if a pipeline hardcodes `{{envs.payments.hml}}`, selecting `prd` does not override it. A future phase can add aliases like `{{env.current.payments}}`.

- [ ] **Step 5: Pass env groups to load tests**

Add `envGroups` to `runRemoteLoadTest` request body and to the load store signature. Show the same selector inside `LoadTestConfigPanel` before starting.

## Task 7: Authoring Experience and Docs

**Files:**
- Modify `app/src/components/StepCreatorPanel.tsx`
- Modify `app/src/components/PipelineDocsPanel.tsx`
- Modify `app/src/components/AIPipelineChat.tsx`
- Modify `app/src/lib/sample-pipeline.ts`
- Modify `engine/README.md`
- Modify `README.md` if necessary.

- [ ] **Step 1: Prefer env templates for new steps**

Change URL generation in `StepCreatorPanel` from:

```ts
`{{specs.${specSlug}.url.${envKey}}}`
```

to:

```ts
`{{envs.${envGroupSlug}.${envKey}}}`
```

Only use `specs` when a route is tied to a spec and no matching env group exists.

- [ ] **Step 2: Update docs panel examples**

Add examples:

```json
{
  "url": "{{envs.users.hml}}/users"
}
```

Keep one compatibility note:

```text
OpenAPI specs still support {{specs.<slug>.url.<env>}} for existing pipelines.
```

- [ ] **Step 3: Update AI prompt**

In `AIPipelineChat`, replace the instruction:

```text
When building step URLs, ALWAYS use {{specs.<slug>.url.<env>}}/path.
```

with:

```text
When building step URLs, prefer {{envs.<group>.<env>}}/path when project env groups are available. Use {{specs.<slug>.url.<env>}} only for existing spec-backed pipelines or when no env group exists.
```

## Task 8: Verification, Migration Safety, and Release

**Files:**
- All touched files.

- [ ] **Step 1: Run focused Rust tests**

```bash
cargo test -p previa-engine env
cargo test -p previa-main env_group
cargo test -p previa-main e2e
cargo test -p previa-main load
```

- [ ] **Step 2: Run frontend tests**

```bash
cd app
npm test -- --run template-validator
npm test -- --run project
```

- [ ] **Step 3: Run release build**

Required by `AGENTS.md`:

```bash
cargo build --release
```

Expected: build succeeds.

- [ ] **Step 4: Manual smoke test**

1. Start the stack.
2. Create a project.
3. Create env group `payments` with entries `local` and `hml`.
4. Create pipeline with `{{envs.payments.local}}/health`.
5. Run single E2E.
6. Run batch E2E.
7. Run load test.
8. Confirm existing `{{specs.<slug>.url.<env>}}` pipeline still runs.

- [ ] **Step 5: Commit and push**

```bash
git add engine main runner app docs
git commit -m "feat: add project env groups"
git push -u origin codex/env-groups
```

## Self-Review

- Spec coverage: the plan covers persistence, API, runtime resolution, validation, frontend loading, execution selection, authoring, docs, and verification.
- Placeholder scan: no deferred implementation markers are present.
- Type consistency: backend uses `RuntimeEnvGroup { slug, urls }`; frontend converts `ProjectEnvGroup.entries[]` into that runtime shape.
- Scope note: this plan intentionally does not remove OpenAPI specs and does not implement `{{env.current.*}}` aliases. Both can be follow-up work after env groups are stable.
