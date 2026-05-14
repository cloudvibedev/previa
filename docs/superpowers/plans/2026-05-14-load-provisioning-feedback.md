# Load Provisioning Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show clear load-test runner provisioning progress in the test screen while Kubernetes runners are being reserved and started.

**Architecture:** The main service already persists Kubernetes reservation state in `runner_reservations`; add a read-only project/pipeline API that exposes the latest non-secret reservation record. The React app starts polling that API immediately after the user starts a load test, even while the load-test `POST` is still waiting for runner readiness, and renders a provisioning panel with a progress bar until execution metrics begin or provisioning ends.

**Tech Stack:** Rust, Axum, SQLx, SQLite migrations already present, Utoipa OpenAPI, React, Zustand, Vite, Vitest, Radix Progress.

---

## File Structure

- Modify `main/src/server/db/runner_reservations.rs`: add a query for the latest reservation for a pipeline, plus tests.
- Modify `main/src/server/db/mod.rs`: export the latest-reservation query.
- Create `main/src/server/handlers/runner_reservations.rs`: add the HTTP handler for the latest reservation state.
- Modify `main/src/server/handlers/mod.rs`: register the new handler module.
- Modify `main/src/server/mod.rs`: wire the new route into Axum.
- Modify `main/src/server/docs.rs`: add the route and schema to OpenAPI.
- Modify `app/src/types/load-test.ts`: add `LoadProvisioningStatus` and extend `LoadTestState` with `provisioning`.
- Modify `app/src/lib/api-client.ts`: add the API type and fetch helper for latest pipeline reservation state.
- Modify `app/src/lib/remote-executor.ts`: start/stop provisioning polling around `runRemoteLoadTest`.
- Modify `app/src/stores/useLoadTestHistoryStore.ts`: store provisioning state and clear it on terminal states.
- Create `app/src/components/LoadProvisioningStatusPanel.tsx`: render progress, counts, elapsed wait, status, and message.
- Modify `app/src/components/LoadTestTab.tsx`: render the provisioning panel above results while provisioning is active.
- Modify `app/src/i18n/locales/en.json` and `app/src/i18n/locales/pt-BR.json`: add UI strings.
- Modify `app/src/lib/remote-executor.test.ts`: verify polling begins before the load-test POST resolves and stops at stream init/error.
- Modify `app/src/components/LoadTestTab.test.tsx`: verify the provisioning panel renders in the test screen.

## Behavior Contract

- The client does not receive the reservation token.
- The API returns `404` when the pipeline has no reservation record.
- The API returns `200` with the latest reservation for the project/pipeline when a record exists.
- Progress is calculated as `readyRunnerCount / requestedRunnerCount`.
- The UI enters `provisioning` as soon as Start is clicked for a remote load test.
- While the load-test `POST` is waiting for the Kubernetes plugin, polling keeps the UI alive.
- When the SSE stream emits `execution:init`, `execution:snapshot`, `metrics`, `complete`, or `error`, polling stops.
- When the user cancels, polling stops and the panel leaves the active state.
- If the main process restarts and loses the in-memory/emptyDir database, the UI surfaces that provisioning state is no longer available instead of looking frozen.

## Task 1: Database Query For Latest Pipeline Reservation

**Files:**
- Modify: `main/src/server/db/runner_reservations.rs`
- Modify: `main/src/server/db/mod.rs`

- [ ] **Step 1: Write the failing database test**

Add the import and test below to `main/src/server/db/runner_reservations.rs`.

```rust
use super::{
    load_latest_runner_reservation_for_pipeline, load_runner_reservation,
    upsert_runner_reservation,
};
```

```rust
#[tokio::test]
async fn latest_runner_reservation_for_pipeline_returns_newest_record() {
    let db = db().await;

    upsert_runner_reservation(
        &db,
        RunnerReservationUpsert {
            execution_id: "exec-old".to_owned(),
            pipeline_id: Some("pipe-1".to_owned()),
            capacity_mode: "kubernetes".to_owned(),
            requested_runner_count: 2,
            ready_runner_count: 1,
            target_rps: 1_000,
            node_profile: Some("4gn.nano".to_owned()),
            reservation_id: Some("rr-old".to_owned()),
            reservation_token: Some("secret-old".to_owned()),
            reservation_expires_at: Some("2026-05-14T10:00:00Z".to_owned()),
            reservation_status: "provisioning".to_owned(),
            runner_endpoints: vec!["http://10.0.0.1:55880".to_owned()],
        },
    )
    .await
    .expect("insert old reservation");

    tokio::time::sleep(std::time::Duration::from_millis(2)).await;

    upsert_runner_reservation(
        &db,
        RunnerReservationUpsert {
            execution_id: "exec-new".to_owned(),
            pipeline_id: Some("pipe-1".to_owned()),
            capacity_mode: "kubernetes".to_owned(),
            requested_runner_count: 3,
            ready_runner_count: 2,
            target_rps: 2_500,
            node_profile: Some("4gn.nano".to_owned()),
            reservation_id: Some("rr-new".to_owned()),
            reservation_token: Some("secret-new".to_owned()),
            reservation_expires_at: Some("2026-05-14T10:05:00Z".to_owned()),
            reservation_status: "provisioning".to_owned(),
            runner_endpoints: vec![
                "http://10.0.0.2:55880".to_owned(),
                "http://10.0.0.3:55880".to_owned(),
            ],
        },
    )
    .await
    .expect("insert new reservation");

    let loaded = load_latest_runner_reservation_for_pipeline(&db, "pipe-1")
        .await
        .expect("load latest reservation")
        .expect("reservation exists");

    assert_eq!(loaded.execution_id, "exec-new");
    assert_eq!(loaded.reservation_id.as_deref(), Some("rr-new"));
    assert_eq!(loaded.requested_runner_count, 3);
    assert_eq!(loaded.ready_runner_count, 2);
    assert_eq!(loaded.reservation_token.as_deref(), Some("secret-new"));
}

#[tokio::test]
async fn latest_runner_reservation_for_pipeline_returns_none_without_record() {
    let db = db().await;

    let loaded = load_latest_runner_reservation_for_pipeline(&db, "pipe-missing")
        .await
        .expect("load missing reservation");

    assert!(loaded.is_none());
}
```

- [ ] **Step 2: Run the database tests and verify the new function is missing**

Run:

```bash
cargo test -p previa-main runner_reservation
```

Expected: the build fails with an unresolved import or missing function named `load_latest_runner_reservation_for_pipeline`.

- [ ] **Step 3: Add the query implementation**

Add this function after `load_runner_reservation`.

```rust
pub async fn load_latest_runner_reservation_for_pipeline(
    db: &DbPool,
    pipeline_id: &str,
) -> Result<Option<RunnerReservationRecord>, sqlx::Error> {
    let row = db
        .query(
            "SELECT execution_id, pipeline_id, capacity_mode, requested_runner_count,
                ready_runner_count, target_rps, node_profile, reservation_id, reservation_token,
                reservation_expires_at, reservation_status, runner_endpoints_json, created_at,
                updated_at
            FROM runner_reservations
            WHERE pipeline_id = ?
            ORDER BY updated_at DESC, created_at DESC
            LIMIT 1",
        )
        .bind(pipeline_id)
        .fetch_optional(db)
        .await?;

    Ok(row.as_ref().map(record_from_row))
}
```

Update the test module import so it includes the new function.

```rust
use super::{
    load_latest_runner_reservation_for_pipeline, load_runner_reservation,
    upsert_runner_reservation,
};
```

- [ ] **Step 4: Run the database tests and verify they pass**

Export both reservation helpers from `main/src/server/db/mod.rs`.

```rust
pub use runner_reservations::{
    load_latest_runner_reservation_for_pipeline, load_runner_reservation,
    upsert_runner_reservation,
};
```

This replaces the existing single-line `pub use runner_reservations::upsert_runner_reservation;`.

- [ ] **Step 5: Run the database tests and verify they pass**

Run:

```bash
cargo test -p previa-main runner_reservation
```

Expected: all `runner_reservation` tests pass.

- [ ] **Step 6: Commit the database query**

Run:

```bash
git add main/src/server/db/runner_reservations.rs main/src/server/db/mod.rs
git commit -m "feat: query latest runner reservation by pipeline"
```

## Task 2: Main API For Pipeline Provisioning State

**Files:**
- Create: `main/src/server/handlers/runner_reservations.rs`
- Modify: `main/src/server/handlers/mod.rs`
- Modify: `main/src/server/mod.rs`
- Modify: `main/src/server/docs.rs`

- [ ] **Step 1: Write the handler test**

Add a test module to the new handler file.

```rust
#[cfg(test)]
mod tests {
    use super::sanitize_runner_reservation;
    use crate::server::models::RunnerReservationRecord;

    #[test]
    fn sanitize_runner_reservation_removes_secret_token() {
        let record = RunnerReservationRecord {
            execution_id: "exec-1".to_owned(),
            pipeline_id: Some("pipe-1".to_owned()),
            capacity_mode: "kubernetes".to_owned(),
            requested_runner_count: 2,
            ready_runner_count: 1,
            target_rps: 1_000,
            node_profile: Some("4gn.nano".to_owned()),
            reservation_id: Some("rr-1".to_owned()),
            reservation_token: Some("secret".to_owned()),
            reservation_expires_at: Some("2026-05-14T10:00:00Z".to_owned()),
            reservation_status: "provisioning".to_owned(),
            runner_endpoints: vec!["http://10.0.0.1:55880".to_owned()],
            created_at: "2026-05-14T09:55:00Z".to_owned(),
            updated_at: "2026-05-14T09:56:00Z".to_owned(),
        };

        let sanitized = sanitize_runner_reservation(record);

        assert!(sanitized.reservation_token.is_none());
        assert_eq!(sanitized.reservation_id.as_deref(), Some("rr-1"));
        assert_eq!(sanitized.ready_runner_count, 1);
    }
}
```

- [ ] **Step 2: Run the handler test and verify it fails**

Run:

```bash
cargo test -p previa-main sanitize_runner_reservation
```

Expected: the test fails because `runner_reservations.rs` and `sanitize_runner_reservation` do not exist yet.

- [ ] **Step 3: Create the handler module**

Create `main/src/server/handlers/runner_reservations.rs`.

```rust
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::server::db::{
    load_latest_runner_reservation_for_pipeline, load_project_pipeline_record,
};
use crate::server::models::{ErrorResponse, RunnerReservationRecord};
use crate::server::state::AppState;

#[utoipa::path(
    get,
    path = "/api/v1/projects/{projectId}/pipelines/{pipelineId}/runner-reservation/latest",
    params(
        ("projectId" = String, Path, description = "Project id"),
        ("pipelineId" = String, Path, description = "Pipeline id")
    ),
    responses(
        (status = 200, description = "Latest runner reservation for the pipeline", body = RunnerReservationRecord),
        (status = 404, description = "Pipeline or runner reservation not found", body = ErrorResponse),
        (status = 500, description = "Failed to load runner reservation", body = ErrorResponse)
    )
)]
pub async fn get_latest_runner_reservation_for_pipeline(
    State(state): State<AppState>,
    Path((project_id, pipeline_id)): Path<(String, String)>,
) -> Result<Json<RunnerReservationRecord>, (StatusCode, Json<ErrorResponse>)> {
    let pipeline = load_project_pipeline_record(&state.db, &project_id, &pipeline_id)
        .await
        .map_err(|err| internal_error(format!("failed to verify pipeline: {err}")))?;

    if pipeline.is_none() {
        return Err(not_found("pipeline not found"));
    }

    let record = load_latest_runner_reservation_for_pipeline(&state.db, &pipeline_id)
        .await
        .map_err(|err| internal_error(format!("failed to load runner reservation: {err}")))?;

    match record {
        Some(record) => Ok(Json(sanitize_runner_reservation(record))),
        None => Err(not_found("runner reservation not found")),
    }
}

pub(crate) fn sanitize_runner_reservation(
    mut record: RunnerReservationRecord,
) -> RunnerReservationRecord {
    record.reservation_token = None;
    record
}

fn not_found(message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn internal_error(message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}
```

- [ ] **Step 4: Register the handler module and route**

Add this line to `main/src/server/handlers/mod.rs`.

```rust
pub mod runner_reservations;
```

Add this import to `main/src/server/mod.rs`.

```rust
use crate::server::handlers::runner_reservations::get_latest_runner_reservation_for_pipeline;
```

Add this route near the existing pipeline routes in `build_app_with_config`.

```rust
.route(
    "/api/v1/projects/{projectId}/pipelines/{pipelineId}/runner-reservation/latest",
    get(get_latest_runner_reservation_for_pipeline),
)
```

- [ ] **Step 5: Add OpenAPI registration**

In `main/src/server/docs.rs`, add `RunnerReservationRecord` to the models import list.

```rust
RunnerRecord, RunnerReservationRecord, RunnerRuntimeInfo, RunnerUpdateRequest,
```

Add the path in the `paths(...)` list.

```rust
crate::server::handlers::runner_reservations::get_latest_runner_reservation_for_pipeline,
```

Add the schema in `components(schemas(...))`.

```rust
RunnerReservationRecord,
```

- [ ] **Step 6: Run API tests and OpenAPI build**

Run:

```bash
cargo test -p previa-main sanitize_runner_reservation
cargo test -p previa-main openapi_info_version_matches_cargo_package_version
```

Expected: both tests pass.

- [ ] **Step 7: Commit the API route**

Run:

```bash
git add main/src/server/handlers/runner_reservations.rs main/src/server/handlers/mod.rs main/src/server/mod.rs main/src/server/docs.rs
git commit -m "feat: expose runner reservation provisioning status"
```

## Task 3: Frontend Types And API Client

**Files:**
- Modify: `app/src/types/load-test.ts`
- Modify: `app/src/lib/api-client.ts`

- [ ] **Step 1: Write the API client test**

Create a new describe block in `app/src/lib/remote-executor.test.ts` or move it later to a dedicated `app/src/lib/api-client.test.ts` if the project already adds one during execution.

```ts
describe("runner reservation status API", () => {
  it("fetches the latest runner reservation for a pipeline without exposing a token", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue({
      ok: true,
      json: async () => ({
        executionId: "exec-1",
        pipelineId: "pipe-1",
        capacityMode: "kubernetes",
        requestedRunnerCount: 3,
        readyRunnerCount: 2,
        targetRps: 2500,
        nodeProfile: "4gn.nano",
        reservationId: "rr-1",
        reservationExpiresAt: "2026-05-14T10:00:00Z",
        reservationStatus: "provisioning",
        runnerEndpoints: ["http://10.0.0.1:55880"],
        createdAt: "2026-05-14T09:55:00Z",
        updatedAt: "2026-05-14T09:56:00Z",
      }),
    } as Response);

    const status = await fetchLatestRunnerReservation(
      "http://localhost:5589",
      "project-1",
      "pipe-1",
    );

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:5589/api/v1/projects/project-1/pipelines/pipe-1/runner-reservation/latest",
      expect.objectContaining({ method: "GET" }),
    );
    expect(status?.reservationId).toBe("rr-1");
    expect("reservationToken" in status!).toBe(false);
  });
});
```

- [ ] **Step 2: Run the frontend test and verify the helper is missing**

Run:

```bash
cd app && npm test -- remote-executor.test.ts
```

Expected: the test fails because `fetchLatestRunnerReservation` is not exported.

- [ ] **Step 3: Add the frontend type**

In `app/src/types/load-test.ts`, change the state union and add the provisioning type.

```ts
export type LoadTestState = "idle" | "provisioning" | "running" | "completed" | "cancelled";

export interface LoadProvisioningStatus {
  executionId: string;
  pipelineId?: string | null;
  capacityMode: string;
  requestedRunnerCount: number;
  readyRunnerCount: number;
  targetRps: number;
  nodeProfile?: string | null;
  reservationId?: string | null;
  reservationExpiresAt?: string | null;
  reservationStatus: string;
  runnerEndpoints: string[];
  createdAt: string;
  updatedAt: string;
  unavailable?: boolean;
  message?: string;
}
```

- [ ] **Step 4: Add the API client helper**

In `app/src/lib/api-client.ts`, update the import.

```ts
import type {
  LoadProvisioningStatus,
  LoadTestMetrics,
  LoadTestState,
  RunnerResourcePoint,
} from "@/types/load-test";
```

Add the helper near the load history helpers.

```ts
export async function fetchLatestRunnerReservation(
  baseUrl: string,
  projectId: string,
  pipelineId: string,
): Promise<LoadProvisioningStatus | null> {
  const base = ensureApiPrefix(baseUrl);
  const url = `${base}/projects/${encodeURIComponent(projectId)}/pipelines/${encodeURIComponent(pipelineId)}/runner-reservation/latest`;
  const init: RequestInit = { method: "GET" };
  const token = getAuthToken();
  const response = await fetch(url, token ? withBearer(init, token) : init);

  if (response.status === 404) return null;
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`HTTP ${response.status}: ${body}`);
  }

  return await response.json() as LoadProvisioningStatus;
}
```

- [ ] **Step 5: Run the frontend test and verify it passes**

Run:

```bash
cd app && npm test -- remote-executor.test.ts
```

Expected: the runner reservation status API test passes.

- [ ] **Step 6: Commit frontend API primitives**

Run:

```bash
git add app/src/types/load-test.ts app/src/lib/api-client.ts app/src/lib/remote-executor.test.ts
git commit -m "feat: add load provisioning status client"
```

## Task 4: Remote Executor Provisioning Polling

**Files:**
- Modify: `app/src/lib/remote-executor.ts`
- Modify: `app/src/lib/remote-executor.test.ts`

- [ ] **Step 1: Write the polling test**

Add this test to `describe("remote load execution requests", ...)` in `app/src/lib/remote-executor.test.ts`.

```ts
it("polls provisioning status while the load stream request is pending", async () => {
  vi.useFakeTimers();
  const callbacks = {
    onError: vi.fn(),
    onProvisioningUpdate: vi.fn(),
  };
  let resolveLoadRequest: ((value: Response) => void) | null = null;
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation((input) => {
    const url = String(input);
    if (url.includes("/runner-reservation/latest")) {
      return Promise.resolve({
        ok: true,
        json: async () => ({
          executionId: "exec-1",
          pipelineId: "pipeline-1",
          capacityMode: "kubernetes",
          requestedRunnerCount: 3,
          readyRunnerCount: 1,
          targetRps: 2500,
          nodeProfile: "4gn.nano",
          reservationId: "rr-1",
          reservationExpiresAt: "2026-05-14T10:00:00Z",
          reservationStatus: "provisioning",
          runnerEndpoints: ["http://10.0.0.1:55880"],
          createdAt: "2026-05-14T09:55:00Z",
          updatedAt: "2026-05-14T09:56:00Z",
        }),
      } as Response);
    }

    return new Promise<Response>((resolve) => {
      resolveLoadRequest = resolve;
    });
  });

  const controller = runRemoteLoadTest(
    "http://localhost:5589",
    pipeline,
    { points: [{ atMs: 0, intensity: 10 }], interpolation: "smooth" },
    callbacks,
    "project-1",
    undefined,
    0,
    [],
    [],
    null,
    2500,
  );

  await vi.advanceTimersByTimeAsync(1_100);

  expect(callbacks.onProvisioningUpdate).toHaveBeenCalledWith(
    expect.objectContaining({
      reservationId: "rr-1",
      readyRunnerCount: 1,
      requestedRunnerCount: 3,
    }),
  );

  controller.disconnect();
  resolveLoadRequest?.({
    ok: false,
    text: async () => "cancelled",
  } as Response);
  vi.useRealTimers();
  fetchMock.mockRestore();
});
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```bash
cd app && npm test -- remote-executor.test.ts
```

Expected: the test fails because `onProvisioningUpdate` is not part of the callback contract and polling is not implemented.

- [ ] **Step 3: Extend callback types and imports**

In `app/src/lib/remote-executor.ts`, import the new type and API helper.

```ts
import type {
  LoadProvisioningStatus,
  LoadRunConfig,
  LoadTestMetrics,
  LoadTestState,
  RemoteMetricsEvent,
  ConsolidatedLoadMetrics,
  LoadLifecycleBucket,
  RpsPoint,
  RunnerResourcePoint,
  RunnerRuntimeInfo,
} from "@/types/load-test";
import { cancelExecution, ensureApiPrefix, fetchLatestRunnerReservation } from "./api-client";
```

Add the callback member to the remote load callback interface.

```ts
onProvisioningUpdate?: (status: LoadProvisioningStatus) => void;
```

- [ ] **Step 4: Add the polling helper inside `runRemoteLoadTest`**

Place this helper before the `run` function in `runRemoteLoadTest`.

```ts
let provisioningPollStopped = false;

const stopProvisioningPolling = () => {
  provisioningPollStopped = true;
};

const startProvisioningPolling = () => {
  if (!callbacks.onProvisioningUpdate || !projectId || !pipeline.id) return;

  const poll = async () => {
    if (provisioningPollStopped || abortController.signal.aborted) return;
    try {
      const status = await fetchLatestRunnerReservation(backendUrl, projectId, pipeline.id);
      if (status) {
        callbacks.onProvisioningUpdate?.(status);
      } else {
        callbacks.onProvisioningUpdate?.({
          executionId: "",
          pipelineId: pipeline.id,
          capacityMode: "kubernetes",
          requestedRunnerCount: 0,
          readyRunnerCount: 0,
          targetRps: targetRps ?? 0,
          reservationStatus: "unavailable",
          runnerEndpoints: [],
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          unavailable: true,
          message: "Provisioning state is not available yet.",
        });
      }
    } catch (err) {
      callbacks.onProvisioningUpdate?.({
        executionId: "",
        pipelineId: pipeline.id,
        capacityMode: "kubernetes",
        requestedRunnerCount: 0,
        readyRunnerCount: 0,
        targetRps: targetRps ?? 0,
        reservationStatus: "unavailable",
        runnerEndpoints: [],
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        unavailable: true,
        message: err instanceof Error ? err.message : String(err),
      });
    }
  };

  void poll();
  const intervalId = window.setInterval(() => {
    if (provisioningPollStopped || abortController.signal.aborted) {
      window.clearInterval(intervalId);
      return;
    }
    void poll();
  }, 1_000);
};
```

Call it immediately before `const response = await fetch(basePath, ...)`.

```ts
startProvisioningPolling();
const response = await fetch(basePath, {
```

Call `stopProvisioningPolling()` after the load-test response is available and on all terminal paths.

```ts
const response = await fetch(basePath, {
  method: "POST",
  headers: {
    "Content-Type": "application/json",
    "Accept": "text/event-stream",
    "x-transaction-id": transactionId,
  },
  body: JSON.stringify(body),
  signal: abortController.signal,
});
stopProvisioningPolling();
```

Also call `stopProvisioningPolling()` at the start of `onError`, `complete`, caught non-abort error, `cancel`, and `disconnect` paths.

- [ ] **Step 5: Run the remote executor tests**

Run:

```bash
cd app && npm test -- remote-executor.test.ts
```

Expected: all remote executor tests pass.

- [ ] **Step 6: Commit remote polling**

Run:

```bash
git add app/src/lib/remote-executor.ts app/src/lib/remote-executor.test.ts
git commit -m "feat: poll load runner provisioning status"
```

## Task 5: Zustand Store Provisioning State

**Files:**
- Modify: `app/src/stores/useLoadTestHistoryStore.ts`

- [ ] **Step 1: Add the store shape**

Update imports.

```ts
import type {
  LoadProvisioningStatus,
  LoadRunConfig,
  LoadTestMetrics,
  LoadTestState,
} from "@/types/load-test";
```

Add fields to `LoadTestHistoryState`.

```ts
provisioningStatus: LoadProvisioningStatus | null;
provisioningStartedAt: number | null;
```

Initialize them.

```ts
provisioningStatus: null,
provisioningStartedAt: null,
```

- [ ] **Step 2: Set provisioning state on start**

In `runTest`, set both display and live state to `provisioning`.

```ts
set((s) => ({
  config: cfg,
  metrics: emptyMetrics,
  state: "provisioning",
  viewingHistoricRun: false,
  nodesInfo: null,
  provisioningStatus: null,
  provisioningStartedAt: Date.now(),
  liveMetrics: emptyMetrics,
  liveState: "provisioning",
  runs: [syntheticRun, ...s.runs.filter(r => r.state !== "running" && r.state !== "provisioning")],
  activeRunId: syntheticId,
}));
```

Set the synthetic run state to `provisioning`.

```ts
state: "provisioning",
```

- [ ] **Step 3: Wire provisioning updates from the remote executor**

Add this callback to the `runRemoteLoadTest` callback object.

```ts
onProvisioningUpdate: (status) => {
  const s = get();
  set({
    provisioningStatus: status,
    liveState: status.unavailable ? "provisioning" : "provisioning",
  });
  if (!s.viewingHistoricRun) {
    set({ state: "provisioning" });
  }
},
```

- [ ] **Step 4: Clear provisioning status on execution updates and terminal states**

In `onSnapshot`, `onMetricsUpdate`, `onComplete`, `onError`, `disconnectController`, `cancelTest`, `resetTest`, `selectHistoricRun`, `backToLive` when leaving provisioning, and `reconnectExecution`, set:

```ts
provisioningStatus: null,
provisioningStartedAt: null,
```

In `onSnapshot`, use:

```ts
provisioningStatus: null,
provisioningStartedAt: null,
liveState: snapshot.state,
```

In `onMetricsUpdate`, do not change `state` to `running` unless the current visible state is still `provisioning` or `running`.

```ts
if (!s.viewingHistoricRun) {
  set({ metrics: snapshot, state: "running", provisioningStatus: null, provisioningStartedAt: null });
}
set({ liveState: "running" });
```

- [ ] **Step 5: Run TypeScript tests**

Run:

```bash
cd app && npm test -- LoadTestTab.test.tsx remote-executor.test.ts
```

Expected: tests compile and pass after Task 6 adds the rendering path.

- [ ] **Step 6: Commit store changes**

Run:

```bash
git add app/src/stores/useLoadTestHistoryStore.ts
git commit -m "feat: track load provisioning state"
```

## Task 6: Provisioning Panel UI

**Files:**
- Create: `app/src/components/LoadProvisioningStatusPanel.tsx`
- Modify: `app/src/components/LoadTestTab.tsx`
- Modify: `app/src/i18n/locales/en.json`
- Modify: `app/src/i18n/locales/pt-BR.json`
- Modify: `app/src/components/LoadTestTab.test.tsx`

- [ ] **Step 1: Add i18n strings**

In `app/src/i18n/locales/en.json`, add keys under the existing `loadTest` object.

```json
"provisioning": {
  "title": "Provisioning Kubernetes runners",
  "subtitle": "{{ready}} of {{requested}} runners ready",
  "waiting": "Waiting for reservation state",
  "unavailable": "Provisioning state is unavailable",
  "reservation": "Reservation",
  "targetRps": "Target RPS",
  "nodeProfile": "Node profile",
  "elapsed": "Elapsed",
  "status": "Status"
}
```

In `app/src/i18n/locales/pt-BR.json`, add matching keys.

```json
"provisioning": {
  "title": "Provisionando runners Kubernetes",
  "subtitle": "{{ready}} de {{requested}} runners prontos",
  "waiting": "Aguardando estado da reserva",
  "unavailable": "Estado do provisionamento indisponível",
  "reservation": "Reserva",
  "targetRps": "RPS alvo",
  "nodeProfile": "Perfil do node",
  "elapsed": "Tempo",
  "status": "Status"
}
```

- [ ] **Step 2: Create the panel component**

Create `app/src/components/LoadProvisioningStatusPanel.tsx`.

```tsx
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { ServerCog } from "lucide-react";

import { Progress } from "@/components/ui/progress";
import type { LoadProvisioningStatus } from "@/types/load-test";

interface LoadProvisioningStatusPanelProps {
  status: LoadProvisioningStatus | null;
  startedAt: number | null;
}

function formatElapsed(startedAt: number | null) {
  if (!startedAt) return "0s";
  const seconds = Math.max(0, Math.floor((Date.now() - startedAt) / 1000));
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return minutes > 0 ? `${minutes}m ${rest}s` : `${rest}s`;
}

export function LoadProvisioningStatusPanel({
  status,
  startedAt,
}: LoadProvisioningStatusPanelProps) {
  const { t } = useTranslation();
  const requested = Math.max(0, status?.requestedRunnerCount ?? 0);
  const ready = Math.max(0, status?.readyRunnerCount ?? 0);
  const progress = requested > 0 ? Math.min(100, Math.round((ready / requested) * 100)) : 0;
  const elapsed = useMemo(() => formatElapsed(startedAt), [startedAt, status?.updatedAt]);

  return (
    <section
      data-testid="load-provisioning-status"
      className="rounded-lg border border-border bg-card p-4 shadow-sm"
    >
      <div className="flex items-start gap-3">
        <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
          <ServerCog className="h-5 w-5" />
        </div>
        <div className="min-w-0 flex-1 space-y-3">
          <div className="min-w-0">
            <h3 className="text-sm font-semibold text-foreground">
              {t("loadTest.provisioning.title")}
            </h3>
            <p className="text-xs text-muted-foreground">
              {status?.unavailable
                ? t("loadTest.provisioning.unavailable")
                : requested > 0
                  ? t("loadTest.provisioning.subtitle", { ready, requested })
                  : t("loadTest.provisioning.waiting")}
            </p>
          </div>

          <Progress value={progress} className="h-2.5" />

          <div className="grid gap-2 text-xs text-muted-foreground sm:grid-cols-2">
            <span>{t("loadTest.provisioning.status")}: {status?.reservationStatus ?? "pending"}</span>
            <span>{t("loadTest.provisioning.elapsed")}: {elapsed}</span>
            {status?.reservationId && (
              <span className="truncate">{t("loadTest.provisioning.reservation")}: {status.reservationId}</span>
            )}
            {status?.targetRps ? (
              <span>{t("loadTest.provisioning.targetRps")}: {status.targetRps}</span>
            ) : null}
            {status?.nodeProfile && (
              <span>{t("loadTest.provisioning.nodeProfile")}: {status.nodeProfile}</span>
            )}
          </div>

          {status?.message && (
            <p className="text-xs text-muted-foreground">{status.message}</p>
          )}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 3: Render the panel in the load test tab**

Update the destructuring in `app/src/components/LoadTestTab.tsx`.

```ts
const {
  state,
  metrics,
  config,
  nodesInfo,
  runs,
  activeRunId,
  viewingHistoricRun,
  liveState,
  provisioningStatus,
  provisioningStartedAt,
} = store;
```

Import the component.

```ts
import { LoadProvisioningStatusPanel } from "./LoadProvisioningStatusPanel";
```

Render it above results when the live or visible state is provisioning.

```tsx
{(state === "provisioning" || liveState === "provisioning") && (
  <LoadProvisioningStatusPanel
    status={provisioningStatus}
    startedAt={provisioningStartedAt}
  />
)}
```

Place that block immediately before `<LoadTestResultsPanel ... />` in `resultsContent`.

- [ ] **Step 4: Update the component test mock**

In `app/src/components/LoadTestTab.test.tsx`, change the store mock so it can be controlled.

```ts
let mockedStoreState = {
  state: "idle",
  liveState: "idle",
  provisioningStatus: null,
  provisioningStartedAt: null,
};
```

Spread those values into the mocked return.

```ts
...mockedStoreState,
```

Add this test.

```tsx
it("shows runner provisioning progress while a load test is provisioning", () => {
  mockedStoreState = {
    state: "provisioning",
    liveState: "provisioning",
    provisioningStartedAt: Date.now() - 2_000,
    provisioningStatus: {
      executionId: "exec-1",
      pipelineId: "pipeline-1",
      capacityMode: "kubernetes",
      requestedRunnerCount: 4,
      readyRunnerCount: 2,
      targetRps: 2500,
      nodeProfile: "4gn.nano",
      reservationId: "rr-1",
      reservationExpiresAt: "2026-05-14T10:00:00Z",
      reservationStatus: "provisioning",
      runnerEndpoints: ["http://10.0.0.1:55880", "http://10.0.0.2:55880"],
      createdAt: "2026-05-14T09:55:00Z",
      updatedAt: "2026-05-14T09:56:00Z",
    },
  };

  render(
    <LoadTestTab
      pipeline={pipeline}
      projectId="project-1"
      pipelineIndex={0}
    />,
  );

  expect(screen.getByTestId("load-provisioning-status")).toBeInTheDocument();
  expect(screen.getByText(/2.*4/)).toBeInTheDocument();
  expect(screen.getByText(/rr-1/)).toBeInTheDocument();
});
```

Reset `mockedStoreState` in `beforeEach`.

```ts
mockedStoreState = {
  state: "idle",
  liveState: "idle",
  provisioningStatus: null,
  provisioningStartedAt: null,
};
```

- [ ] **Step 5: Run the UI tests**

Run:

```bash
cd app && npm test -- LoadTestTab.test.tsx
```

Expected: all `LoadTestTab` tests pass.

- [ ] **Step 6: Commit the UI**

Run:

```bash
git add app/src/components/LoadProvisioningStatusPanel.tsx app/src/components/LoadTestTab.tsx app/src/i18n/locales/en.json app/src/i18n/locales/pt-BR.json app/src/components/LoadTestTab.test.tsx
git commit -m "feat: show load runner provisioning progress"
```

## Task 7: End-To-End Verification

**Files:**
- No source changes unless verification exposes a defect.

- [ ] **Step 1: Run focused backend tests**

Run:

```bash
cargo test -p previa-main runner_reservation
cargo test -p previa-main sanitize_runner_reservation
cargo test -p previa-main openapi_info_version_matches_cargo_package_version
```

Expected: all selected backend tests pass.

- [ ] **Step 2: Run focused frontend tests**

Run:

```bash
cd app && npm test -- remote-executor.test.ts LoadTestTab.test.tsx
```

Expected: all selected frontend tests pass.

- [ ] **Step 3: Run frontend build**

Run:

```bash
cd app && npm run build
```

Expected: Vite build completes without TypeScript or bundling errors.

- [ ] **Step 4: Run release build**

Run from repository root:

```bash
cargo build --release
```

Expected: release build completes.

- [ ] **Step 5: Manual local verification**

Start the app and main as currently used in sandbox testing.

```bash
cd app && npm run dev -- --host 127.0.0.1
```

```bash
kubectl -n previa port-forward svc/previa-main 5589:80
```

Open:

```text
http://127.0.0.1:5173/
```

Run a load test with Kubernetes capacity enabled. Expected UI behavior:

- Immediately after Start, the load test screen shows "Provisionando runners Kubernetes".
- The progress bar advances as `readyRunnerCount` approaches `requestedRunnerCount`.
- The reservation id is visible, but no reservation token is visible.
- When execution starts, the provisioning panel disappears and normal load metrics render.
- If `previa-main` restarts during provisioning and loses the reservation row, the panel reports unavailable provisioning state instead of leaving a blank/stuck screen.

- [ ] **Step 6: Final commit and push**

If all verification succeeds, run:

```bash
git status --short
git push
```

Expected: branch is pushed with the task commits.

## Self-Review

- Spec coverage: the plan covers visible provisioning feedback, progress by runner readiness, no token exposure to the client, polling while the main waits for Kubernetes readiness, cancellation cleanup, and the main-restart/lost-state symptom observed in the large test.
- Divergences: this plan does not make `previa-main` state durable after pod eviction; it only makes the provisioning phase observable. Durable main storage remains a separate reliability task.
- Compatibility: adding `provisioning` to `LoadTestState` changes frontend state handling and local run typing; backend API is additive.
- Risk: polling by latest pipeline reservation assumes only one active reservation per pipeline, which matches the current product rule that parallel executions of the same pipeline wait in queue.
