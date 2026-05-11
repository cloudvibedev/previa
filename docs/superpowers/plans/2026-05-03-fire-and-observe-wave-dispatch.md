# Fire And Observe Wave Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make wave load tests keep the configured HTTP start-rate curve even when responses are slow, by decoupling request emission from response/pipeline completion.

**Architecture:** The runner will stop using completed pipeline tasks as the effective stock for future HTTP starts. A runner-local wave emitter will own the dispatch clock, submit HTTP sends on schedule, and record start metrics at the send boundary; response collection will run in a separate observer lane. The existing sequential engine path remains available for normal execution, while load tests gain a fire-and-observe path that can preserve the wave and expose when the bottleneck moves to Tokio scheduling, HTTP client/socket, or target response backlog.

**Tech Stack:** Rust, Tokio, Reqwest, Axum/SSE, serde JSON, existing `previa-engine`, `previa-runner`, `previa-main`, React/TypeScript load-test dashboard.

---

## Current Problem

Today the wave opens start slots, but pipeline tasks consume those slots and then stay occupied until the HTTP response/body/assertions finish.

```text
wave slot
  -> pipeline task consumes slot
  -> httpStarted++
  -> reqwest.send().await
  -> response body/assertions
  -> task finishes
  -> stock can be replenished
```

When responses become slow, `outstandingRequests` grows, `readyRequests` drains, and later wave slots expire without a task ready to consume them. Raising `maxInFlight` only grows the backlog; it does not remove the coupling.

The new model must make the wave drive HTTP starts directly:

```text
wave slot
  -> submit HTTP send immediately
  -> httpStarted++ at send boundary
  -> response observer awaits headers/body independently
  -> completion metrics update later
```

## Boundaries And Guarantees

- The RPS chart continues to represent HTTP starts per second, not completed responses.
- `maxInFlight` must no longer be interpreted as "how many pipeline tasks can exist before the next start". It becomes a response-observation safety limit or is renamed internally to `maxObservedInFlight`.
- The emitter should not wait for response completion to open or consume future wave slots.
- The implementation must expose when exact curve adherence becomes limited by local runtime/socket capacity, not hide that as response latency.
- For response-dependent multi-step pipelines, only dispatchable requests can be started before their dependencies resolve. The runner must expose this as `dependencyLimitedStarts` instead of pretending the wave can start a request whose input does not exist yet.

## File Map

- Modify: `engine/src/execution/engine.rs`
  - Extract HTTP request preparation and send/result handling into reusable helpers.
- Create: `engine/src/execution/http_step.rs`
  - Own prepared HTTP step data and the function that sends one prepared step and builds `StepExecutionResult`.
- Modify: `engine/src/execution/mod.rs`
- Modify: `engine/src/lib.rs`
- Modify: `runner/src/lib.rs`
- Modify: `runner/src/server/handlers/load.rs`
  - Keep HTTP transport/wiring only; delegate wave execution to a service module.
- Create: `runner/src/server/wave_executor.rs`
  - Own fire-and-observe load execution lifecycle.
- Create: `runner/src/server/wave_emitter.rs`
  - Own dispatch clock consumption and HTTP send submission.
- Create: `runner/src/server/response_observer.rs`
  - Own response futures, completion collection, and safety caps.
- Modify: `runner/src/server/load_dispatch.rs`
  - Keep slot math, add lag/accounting tests where needed.
- Modify: `runner/src/server/metrics.rs`
  - Add lifecycle counters and lag metrics.
- Modify: `runner/src/server/models.rs`
  - Serialize new metrics fields.
- Modify: `main/src/server/utils.rs`
- Modify: `main/src/server/execution/load_batch.rs`
- Modify: `main/src/server/models.rs`
  - Aggregate and persist new metrics fields.
- Modify: `app/src/types/load-test.ts`
- Modify: `app/src/lib/remote-executor.ts`
- Modify: `app/src/components/LoadTestResultsPanel.tsx`
  - Show whether the run is response-limited, dependency-limited, or runtime/socket-limited.

---

## Stage 1: Prove The Boundary With Lifecycle Metrics

### Task 1: Add HTTP lifecycle counters

**Files:**
- Modify: `runner/src/server/metrics.rs`
- Modify: `runner/src/server/models.rs`
- Modify: `main/src/server/utils.rs`
- Modify: `main/src/server/execution/load_batch.rs`
- Modify: `app/src/types/load-test.ts`

- [ ] **Step 1: Add failing metrics tests**

Add tests in `runner/src/server/metrics.rs` asserting the snapshot can expose these counters:

```rust
#[test]
fn snapshot_includes_http_lifecycle_counters() {
    let mut metrics = MetricsAccumulator::new();

    metrics.record_dispatch_submitted_count(3);
    metrics.record_http_start();
    metrics.record_http_send_returned();
    metrics.record_http_completed_count(1);
    metrics.record_response_body_completed_count(1);

    let snapshot = metrics.snapshot(None, None);

    assert_eq!(snapshot.dispatch_submitted, Some(3));
    assert_eq!(snapshot.http_started, 1);
    assert_eq!(snapshot.http_send_returned, Some(1));
    assert_eq!(snapshot.http_completed, 1);
    assert_eq!(snapshot.response_body_completed, Some(1));
}
```

Run:

```bash
cargo test -p previa-runner snapshot_includes_http_lifecycle_counters
```

Expected before implementation: compile failure for missing methods/fields.

- [ ] **Step 2: Implement runner metric fields**

Add fields to `MetricsAccumulator`:

```rust
dispatch_submitted: usize,
http_send_returned: usize,
response_body_completed: usize,
dependency_limited_starts: usize,
runtime_lagged_starts: usize,
```

Add methods:

```rust
pub fn record_dispatch_submitted_count(&mut self, count: usize) {
    self.dispatch_submitted = self.dispatch_submitted.saturating_add(count);
}

pub fn record_http_send_returned(&mut self) {
    self.http_send_returned = self.http_send_returned.saturating_add(1);
}

pub fn record_response_body_completed_count(&mut self, count: usize) {
    self.response_body_completed = self.response_body_completed.saturating_add(count);
}

pub fn record_dependency_limited_starts_count(&mut self, count: usize) {
    self.dependency_limited_starts = self.dependency_limited_starts.saturating_add(count);
}

pub fn record_runtime_lagged_start(&mut self) {
    self.runtime_lagged_starts = self.runtime_lagged_starts.saturating_add(1);
}
```

Expose them on `LoadTestMetrics` as camelCase JSON:

```rust
#[serde(rename = "dispatchSubmitted", skip_serializing_if = "Option::is_none")]
pub dispatch_submitted: Option<usize>,
#[serde(rename = "httpSendReturned", skip_serializing_if = "Option::is_none")]
pub http_send_returned: Option<usize>,
#[serde(rename = "responseBodyCompleted", skip_serializing_if = "Option::is_none")]
pub response_body_completed: Option<usize>,
#[serde(rename = "dependencyLimitedStarts", skip_serializing_if = "Option::is_none")]
pub dependency_limited_starts: Option<usize>,
#[serde(rename = "runtimeLaggedStarts", skip_serializing_if = "Option::is_none")]
pub runtime_lagged_starts: Option<usize>,
```

- [ ] **Step 3: Aggregate through main and app types**

Parse and sum the new fields wherever `httpStarted`, `missedStarts`, and `outstandingRequests` are already handled:

```rust
dispatch_submitted: get_usize_field(payload, "dispatchSubmitted"),
http_send_returned: get_usize_field(payload, "httpSendReturned"),
response_body_completed: get_usize_field(payload, "responseBodyCompleted"),
dependency_limited_starts: get_usize_field(payload, "dependencyLimitedStarts"),
runtime_lagged_starts: get_usize_field(payload, "runtimeLaggedStarts"),
```

In TypeScript, add:

```ts
dispatchSubmitted?: number;
httpSendReturned?: number;
responseBodyCompleted?: number;
dependencyLimitedStarts?: number;
runtimeLaggedStarts?: number;
```

- [ ] **Step 4: Verify**

Run:

```bash
cargo test -p previa-runner snapshot_includes_http_lifecycle_counters
cargo test -p previa-main load_batch
npm --prefix app test -- LoadTestResultsPanel
```

Expected: all pass.

---

## Stage 2: Extract Single-Step HTTP Execution From The Engine

### Task 2: Create a reusable prepared HTTP step helper

**Files:**
- Create: `engine/src/execution/http_step.rs`
- Modify: `engine/src/execution/engine.rs`
- Modify: `engine/src/execution/mod.rs`
- Modify: `engine/src/lib.rs`
- Modify: `runner/src/lib.rs`

- [ ] **Step 1: Add helper tests**

Add tests in `engine/src/execution/http_step.rs`:

```rust
#[tokio::test]
async fn sends_prepared_step_and_returns_success_result() {
    let server = httpmock::MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/users");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({"ok": true}));
        })
        .await;

    let client = reqwest::Client::new();
    let step = crate::core::types::PipelineStep {
        id: "get-users".to_owned(),
        name: "GET users".to_owned(),
        description: None,
        method: "GET".to_owned(),
        url: format!("{}/users", server.base_url()),
        headers: std::collections::HashMap::new(),
        body: None,
        operation_id: None,
        delay: None,
        retry: None,
        asserts: vec![],
    };
    let context = std::collections::HashMap::new();

    let prepared = prepare_http_step(
        &step,
        &context,
        None,
        None,
        None,
        1,
        1,
    )
    .expect("step should prepare");

    let result = send_prepared_http_step(
        &client,
        prepared,
        &step,
        &context,
        None,
        None,
        None,
        || false,
    )
    .await
    .expect("send should not be cancelled");

    assert_eq!(result.step_id, "get-users");
    assert_eq!(result.status, "success");
    assert_eq!(result.response.as_ref().map(|r| r.status), Some(200));
}
```

Run:

```bash
cargo test -p previa-engine sends_prepared_step_and_returns_success_result
```

Expected before implementation: compile failure because `http_step` does not exist.

- [ ] **Step 2: Implement `PreparedHttpStep`**

Create `engine/src/execution/http_step.rs` with a focused data contract:

```rust
use reqwest::{Client, Method, Url};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::assertions::{evaluate_assertions, has_status_assertion};
use crate::core::types::{
    PipelineStep, RuntimeEnvGroup, RuntimeSpec, StepExecutionResult, StepRequest, StepResponse,
};
use crate::execution::cancel::await_with_cancel;
use crate::execution::http::{parse_absolute_http_url, parse_method};
use crate::execution::logging::{log_step_request, log_step_response};
use crate::template::resolve::resolve_template_variables;

#[derive(Debug, Clone)]
pub struct PreparedHttpStep {
    pub step_id: String,
    pub attempt: usize,
    pub max_attempts: usize,
    pub method: Method,
    pub url: Url,
    pub request: StepRequest,
}
```

Implement:

```rust
pub fn prepare_http_step(
    step: &PipelineStep,
    context: &HashMap<String, StepExecutionResult>,
    specs: Option<&[RuntimeSpec]>,
    env_groups: Option<&[RuntimeEnvGroup]>,
    selected_env_group_slug: Option<&str>,
    attempt: usize,
    max_attempts: usize,
) -> Result<PreparedHttpStep, StepExecutionResult>
```

The `Err(StepExecutionResult)` branch must contain the same invalid method/URL result shape currently built inline in `engine.rs`.

Implement:

```rust
pub async fn send_prepared_http_step<FCancel>(
    client: &Client,
    prepared: PreparedHttpStep,
    step: &PipelineStep,
    context: &HashMap<String, StepExecutionResult>,
    specs: Option<&[RuntimeSpec]>,
    env_groups: Option<&[RuntimeEnvGroup]>,
    selected_env_group_slug: Option<&str>,
    should_cancel: FCancel,
) -> Option<StepExecutionResult>
where
    FCancel: FnMut() -> bool,
```

This function owns `request_builder.send().await`, body parsing, assertion evaluation, status handling, and `StepExecutionResult` creation.

- [ ] **Step 3: Refactor `engine.rs` to call the helper**

Replace the inline request-build/send/result block inside `execute_pipeline_with_client_runtime_hooks` with:

```rust
let prepared = match prepare_http_step(
    step,
    &context,
    specs,
    env_groups,
    selected_env_group_slug,
    attempt,
    max_attempts,
) {
    Ok(prepared) => prepared,
    Err(result) => {
        log_step_response(&step.id, None, result.error.as_deref());
        if attempt < max_attempts {
            continue;
        }
        if finalize_step_result(&step.id, result, &mut context, &mut results, &mut on_step_result) {
            break 'steps;
        }
        break;
    }
};

log_step_request(&step.id, &prepared.request);
let request_admitted = on_request_start(&prepared.request).await;
if !request_admitted {
    break 'steps;
}

let Some(result) = send_prepared_http_step(
    client,
    prepared,
    step,
    &context,
    specs,
    env_groups,
    selected_env_group_slug,
    &mut should_cancel,
)
.await else {
    break 'steps;
};
```

Keep existing retry/finalization behavior identical.

- [ ] **Step 4: Export the helper**

Add `pub mod http_step;` to `engine/src/execution/mod.rs` and export the helper types/functions from `engine/src/lib.rs` and `runner/src/lib.rs`.

- [ ] **Step 5: Verify legacy behavior**

Run:

```bash
cargo test -p previa-engine execution
cargo test -p previa-runner
```

Expected: existing sequential execution tests still pass.

---

## Stage 3: Add Fire-And-Observe Wave Execution

### Task 3: Build the emitter and observer lanes

**Files:**
- Create: `runner/src/server/wave_emitter.rs`
- Create: `runner/src/server/response_observer.rs`
- Create: `runner/src/server/wave_executor.rs`
- Modify: `runner/src/server/mod.rs`
- Modify: `runner/src/server/handlers/load.rs`
- Modify: `runner/src/server/metrics.rs`

- [ ] **Step 1: Add emitter tests**

Create `runner/src/server/wave_emitter.rs` with tests for pure scheduling behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_runtime_lag_when_send_start_happens_after_tick_window() {
        let tick_ms = 100;
        let lag = classify_start_lag(1_000, 1_135, tick_ms);
        assert_eq!(lag, StartLagClass::RuntimeLagged);
    }

    #[test]
    fn accepts_start_inside_tick_window() {
        let tick_ms = 100;
        let lag = classify_start_lag(1_000, 1_075, tick_ms);
        assert_eq!(lag, StartLagClass::OnTime);
    }
}
```

Run:

```bash
cargo test -p previa-runner wave_emitter
```

Expected before implementation: compile failure.

- [ ] **Step 2: Implement start-lag classification**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartLagClass {
    OnTime,
    RuntimeLagged,
}

pub fn classify_start_lag(tick_elapsed_ms: u64, actual_elapsed_ms: u64, tick_ms: u64) -> StartLagClass {
    if actual_elapsed_ms <= tick_elapsed_ms.saturating_add(tick_ms) {
        StartLagClass::OnTime
    } else {
        StartLagClass::RuntimeLagged
    }
}
```

- [ ] **Step 3: Implement response observer**

Create `runner/src/server/response_observer.rs`:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::task::JoinSet;

#[derive(Debug)]
pub struct ResponseObserver {
    in_flight: Arc<AtomicUsize>,
    tasks: JoinSet<()>,
}

impl ResponseObserver {
    pub fn new(in_flight: Arc<AtomicUsize>) -> Self {
        Self {
            in_flight,
            tasks: JoinSet::new(),
        }
    }

    pub fn response_in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    pub async fn drain_finished(&mut self) {
        while let Ok(Some(result)) =
            tokio::time::timeout(std::time::Duration::from_millis(0), self.tasks.join_next()).await
        {
            if let Err(err) = result {
                tracing::error!("response observer join error: {}", err);
            }
        }
    }

    pub async fn shutdown_until(&mut self, deadline: tokio::time::Instant) {
        while !self.tasks.is_empty() {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match tokio::time::timeout(remaining, self.tasks.join_next()).await {
                Ok(Some(Err(err))) => tracing::error!("response observer join error: {}", err),
                Ok(Some(Ok(()))) => {}
                Ok(None) => break,
                Err(_) => break,
            }
        }
        if !self.tasks.is_empty() {
            self.tasks.abort_all();
        }
    }
}
```

Add a `spawn_observed` method in the same file during implementation. It should:

```text
1. increment response_in_flight
2. call metrics.record_http_start() immediately before request send
3. await send_prepared_http_step(...)
4. increment http_send_returned once reqwest send returns
5. update completion, network, success/error metrics
6. decrement response_in_flight
7. emit an SSE metrics snapshot
```

- [ ] **Step 4: Implement wave executor service**

Move the body of `run_wave_load` from `runner/src/server/handlers/load.rs` into `runner/src/server/wave_executor.rs`.

Expose:

```rust
pub async fn run_wave_load(
    load: LoadProfile,
    pipeline: Pipeline,
    selected_key: Option<String>,
    selected_env_group_slug: Option<String>,
    specs: Vec<RuntimeSpec>,
    env_groups: Vec<RuntimeEnvGroup>,
    tx: mpsc::UnboundedSender<SseMessage>,
    token: tokio_util::sync::CancellationToken,
)
```

`handlers/load.rs` should call this function and keep only request handling/wiring.

- [ ] **Step 5: Change emission model**

Inside `wave_executor.rs`, replace the "spawn active pipelines until desired stock" loop with this model:

```text
for each tick:
  1. compute target_rps_limit
  2. plan DispatchTick
  3. record dispatchSubmitted += tick.scheduled_starts
  4. for each scheduled start:
       prepare a dispatchable HTTP step
       spawn response observer task
  5. emit metrics snapshot
  6. sleep until next tick
```

For the first implementation, support the common load-test case where the pipeline can prepare its first HTTP step without previous response context. Use the existing sequential path as fallback when a later step depends on prior responses. Record `dependencyLimitedStarts` for scheduled slots that cannot prepare a request because required context is not available.

The first dispatchable request can be prepared with:

```rust
let context = std::collections::HashMap::new();
let step = pipeline.steps.first().expect("validated pipeline has at least one step");
let prepared = previa_runner::execution::http_step::prepare_http_step(
    step,
    &context,
    Some(specs.as_slice()),
    Some(env_groups.as_slice()),
    selected_env_group_slug.as_deref(),
    1,
    step.retry.unwrap_or(0).saturating_add(1),
);
```

When `prepared` is `Err(result)`, record a completed error result without consuming response observation capacity.

- [ ] **Step 6: Keep response completion independent of future starts**

Do not check `response_in_flight` before submitting a scheduled start. Use `response_in_flight` only for metrics and shutdown/grace handling.

If a safety cap is needed to protect local development, name it separately as `maxObservedInFlight` and expose when it is hit:

```text
observedInFlight
maxObservedInFlight
observerSaturatedStarts
```

Do not reuse `maxInFlight` as a pre-send throttle in the emitter.

- [ ] **Step 7: Verify focused runner tests**

Run:

```bash
cargo test -p previa-runner wave_emitter
cargo test -p previa-runner load_dispatch
cargo test -p previa-runner metrics
```

Expected: all pass.

---

## Stage 4: Main/UI Visibility

### Task 4: Surface the new diagnostic shape

**Files:**
- Modify: `main/src/server/utils.rs`
- Modify: `main/src/server/execution/load_batch.rs`
- Modify: `main/src/server/models.rs`
- Modify: `app/src/types/load-test.ts`
- Modify: `app/src/lib/remote-executor.ts`
- Modify: `app/src/components/LoadTestResultsPanel.tsx`
- Modify: `app/src/i18n/locales/pt-BR.json`
- Modify: `app/src/i18n/locales/en.json`

- [ ] **Step 1: Preserve per-runner lifecycle metrics**

In every runner sample, include:

```json
{
  "dispatchSubmitted": 1200,
  "httpStarted": 1188,
  "httpSendReturned": 900,
  "httpCompleted": 870,
  "responseBodyCompleted": 850,
  "dependencyLimitedStarts": 0,
  "runtimeLaggedStarts": 3,
  "outstandingRequests": 318
}
```

- [ ] **Step 2: Add UI labels**

Add Portuguese labels:

```json
"loadTestResults.dispatchSubmitted": "Disparos planejados",
"loadTestResults.httpSendReturned": "Sends retornados",
"loadTestResults.responseBodyCompleted": "Bodies concluídos",
"loadTestResults.dependencyLimitedStarts": "Limitados por dependência",
"loadTestResults.runtimeLaggedStarts": "Atrasados pelo runtime"
```

Add equivalent English labels.

- [ ] **Step 3: Add a diagnostic strip under the RPS chart**

Show compact counters:

```text
Planejado: scheduledStarts
Submetido: dispatchSubmitted
HTTP iniciado: httpStarted
Send retornou: httpSendReturned
Body concluído: responseBodyCompleted
Dependência: dependencyLimitedStarts
Runtime: runtimeLaggedStarts
```

This makes it clear whether the curve failed before send, inside send, or after response.

- [ ] **Step 4: Verify UI tests**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
npm --prefix app test -- LoadTestTab
```

Expected: tests pass and the panel renders new labels when metrics are present.

---

## Stage 5: End-To-End Validation

### Task 5: Validate with 3 local runners

**Files:**
- No source files unless validation reveals a bug.

- [ ] **Step 1: Build release binaries**

Run:

```bash
cargo build --release
```

Expected: release build succeeds.

- [ ] **Step 2: Restart local main and runners**

Use the same local topology already used for this branch:

```text
main: 127.0.0.1:5610
runner-1: 127.0.0.1:5611
runner-2: 127.0.0.1:5612
runner-3: 127.0.0.1:5613
```

- [ ] **Step 3: Execute the CRUD Users load test**

Run the load test from:

```text
http://127.0.0.1:5610/projects/019de1a7-4dfd-7662-8b53-a305e5714ca5/pipeline/019de1a7-4dfd-7662-8b53-a317b9bdbe23/load-test
```

- [ ] **Step 4: Pull latest history through the API**

Run:

```bash
curl -s "http://127.0.0.1:5610/api/v1/projects/019de1a7-4dfd-7662-8b53-a305e5714ca5/tests/load?pipelineIndex=0&limit=1"
```

Expected: latest item contains the new lifecycle fields.

- [ ] **Step 5: Compare curve adherence**

For every 10s bucket, compute:

```text
targetRpsLimit
httpStarted delta/sec
dispatchSubmitted delta/sec
httpSendReturned delta/sec
responseBodyCompleted delta/sec
runtimeLaggedStarts delta
dependencyLimitedStarts delta
```

Success criteria:

```text
dispatchSubmitted follows target within 1%
httpStarted follows target within 3% while runtimeLaggedStarts remains near zero
responseBodyCompleted may lag without reducing future dispatchSubmitted/httpStarted
missedStarts stays near zero unless runtimeLaggedStarts rises
```

- [ ] **Step 6: Commit and push after release build**

After all verification passes:

```bash
git add engine runner main app docs
git commit -m "feat: decouple wave dispatch from response observation"
git push origin codex/wave-load-test
```

---

## Implementation Notes

- The first version should optimize the common load-test path: one dispatchable HTTP step per scheduled wave slot.
- Sequential multi-step execution remains available for normal execution and as a fallback.
- Full continuation-based multi-step load execution can be a follow-up: after each response completes, the observer can resume the pipeline state and enqueue the next dispatchable request into the emitter. That should be a separate plan if the first fire-and-observe path proves stable.
- The RPS chart should use `httpStarted`, not `httpSendReturned` or `httpCompleted`.
- If `httpStarted` stops following `dispatchSubmitted`, the local runtime/client/socket is the bottleneck.
- If `httpSendReturned` and `responseBodyCompleted` lag while `httpStarted` follows the curve, the target/network/response path is the bottleneck but the wave dispatch is doing its job.
