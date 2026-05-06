# Wave Sender Start Accuracy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make wave load tests keep the requested send curve more closely at peak while fixing the `response_in_flight` counter underflow seen in the latest run.

**Architecture:** Keep the open-loop model: scheduling and request start must not wait for HTTP response body/assertions. Replace per-request `tokio::spawn` in the sender hot path with fixed sender workers that start HTTP inline, and make in-flight accounting monotonic/saturating so shutdown/cancel paths cannot wrap `usize` to `18446744073709551615`.

**Tech Stack:** Rust, Tokio, reqwest, Axum SSE metrics, React/TypeScript charts.

---

## File Structure

- Modify: `runner/src/server/wave_sender.rs`
  - Owns wave sender workers, HTTP start, observer handoff, and `response_in_flight` decrement paths.
- Modify: `runner/src/server/wave_executor.rs`
  - Owns final shutdown/cancel sequence and final wave snapshot values.
- Modify: `runner/src/server/metrics.rs`
  - Owns lifecycle buckets and metric snapshot fields.
- Modify: `runner/src/server/wave_metrics_actor.rs`
  - Owns event-to-metric accumulation tests.
- Modify: `app/src/lib/load-lifecycle-chart.ts`
  - Builds lifecycle chart series shown in the UI.
- Modify: `app/src/components/LoadTestResultsPanel.tsx`
  - Renders the wave lifecycle chart and labels.
- Test: `runner/src/server/wave_sender.rs`
- Test: `runner/src/server/metrics.rs`
- Test: `runner/src/server/wave_metrics_actor.rs`
- Test: `app/src/components/LoadTestResultsPanel.test.tsx`

---

### Task 1: Fix `response_in_flight` Underflow

**Files:**
- Modify: `runner/src/server/wave_sender.rs`
- Test: `runner/src/server/wave_sender.rs`

- [ ] **Step 1: Add a failing unit test for saturating decrement**

Add this test inside `mod tests` in `runner/src/server/wave_sender.rs`:

```rust
#[test]
fn response_in_flight_decrement_does_not_underflow() {
    let counter = AtomicUsize::new(0);

    decrement_response_in_flight(&counter);

    assert_eq!(counter.load(Ordering::SeqCst), 0);

    counter.store(2, Ordering::SeqCst);
    decrement_response_in_flight(&counter);
    decrement_response_in_flight(&counter);
    decrement_response_in_flight(&counter);

    assert_eq!(counter.load(Ordering::SeqCst), 0);
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p previa-runner response_in_flight_decrement_does_not_underflow -- --nocapture
```

Expected: FAIL because `decrement_response_in_flight` does not exist.

- [ ] **Step 3: Add the saturating decrement helper**

Add near the sender helper functions in `runner/src/server/wave_sender.rs`:

```rust
fn decrement_response_in_flight(counter: &AtomicUsize) {
    let mut current = counter.load(Ordering::SeqCst);
    while current > 0 {
        match counter.compare_exchange(
            current,
            current - 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => return,
            Err(next) => current = next,
        }
    }
}
```

- [ ] **Step 4: Replace raw decrements**

In `runner/src/server/wave_sender.rs`, replace every decrement of `response_in_flight`:

```rust
response_in_flight.fetch_sub(1, Ordering::SeqCst);
```

with:

```rust
decrement_response_in_flight(&response_in_flight);
```

The affected locations are:
- Cancelled start path in `start_ready_request`
- Observer channel closed path in `start_ready_request`
- Completion path in `run_observer_request`

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p previa-runner wave_sender -- --nocapture
```

Expected: all `wave_sender` tests pass and the new underflow test passes.

---

### Task 2: Remove Per-Request Spawn From Sender Hot Path

**Files:**
- Modify: `runner/src/server/wave_sender.rs`
- Test: `runner/src/server/wave_sender.rs`

- [ ] **Step 1: Add a behavior test for fixed-worker start path**

Add this test inside `mod tests` in `runner/src/server/wave_sender.rs`:

```rust
#[tokio::test]
async fn sender_fixed_workers_start_http_without_waiting_for_body_observation() {
    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let (metric_tx, mut metric_rx) = mpsc::unbounded_channel();
    let (observer_tx, mut observer_rx) = mpsc::unbounded_channel();
    let ready_to_send = Arc::new(AtomicUsize::new(0));
    let response_in_flight = Arc::new(AtomicUsize::new(0));
    let token = tokio_util::sync::CancellationToken::new();
    let started = Instant::now();

    for index in 0..32usize {
        ready_to_send.fetch_add(1, Ordering::SeqCst);
        request_tx
            .send(test_ready_wave_request(index, started, 0, 60_000))
            .expect("request should enqueue");
    }
    drop(request_tx);

    run_fire_only_sender_for_test(
        Arc::new(Client::new()),
        started,
        metric_tx,
        Arc::clone(&response_in_flight),
        Arc::clone(&ready_to_send),
        request_rx,
        observer_tx,
        token,
    )
    .await;

    let mut observer_commands = 0usize;
    while observer_rx.try_recv().is_ok() {
        observer_commands += 1;
    }

    let mut send_started = 0usize;
    let mut http_started = 0usize;
    while let Ok(event) = metric_rx.try_recv() {
        if matches!(event, WaveMetricEvent::SendStarted { .. }) {
            send_started += 1;
        }
        if matches!(event, WaveMetricEvent::HttpStarted { .. }) {
            http_started += 1;
        }
    }

    assert_eq!(observer_commands, 32);
    assert_eq!(send_started, 32);
    assert_eq!(http_started, 32);
    assert_eq!(ready_to_send.load(Ordering::SeqCst), 0);
    assert_eq!(response_in_flight.load(Ordering::SeqCst), 32);
}
```

- [ ] **Step 2: Run the focused test**

Run:

```bash
cargo test -p previa-runner sender_fixed_workers_start_http_without_waiting_for_body_observation -- --nocapture
```

Expected: PASS before refactor and after refactor; this is a behavior guard.

- [ ] **Step 3: Remove the per-request `JoinSet` from `run_sender_worker`**

In `runner/src/server/wave_sender.rs`, change `run_sender_worker` from spawning one task per request:

```rust
start_tasks.spawn(start_ready_request(
    Arc::clone(&client),
    started,
    metric_tx.clone(),
    request,
    Arc::clone(&response_in_flight),
    observer_tx.clone(),
    token.clone(),
));
```

to starting HTTP inline inside the fixed worker:

```rust
start_ready_request(
    Arc::clone(&client),
    started,
    metric_tx.clone(),
    request,
    Arc::clone(&response_in_flight),
    observer_tx.clone(),
    token.clone(),
)
.await;
```

Also remove:

```rust
let mut start_tasks = JoinSet::new();
Some(_) = start_tasks.join_next(), if !start_tasks.is_empty() => {}
while start_tasks.join_next().await.is_some() {}
```

and restore the loop exit condition to:

```rust
if worker_closed {
    break;
}
```

- [ ] **Step 4: Run sender tests**

Run:

```bash
cargo test -p previa-runner wave_sender -- --nocapture
```

Expected: all sender tests pass. `sendStarted` and `httpStarted` should remain equal in the fixed-worker tests.

---

### Task 3: Make Final Snapshot Resistant To Shutdown Races

**Files:**
- Modify: `runner/src/server/wave_executor.rs`
- Test: `runner/src/server/wave_sender.rs`

- [ ] **Step 1: Add a shutdown-focused test**

Add this test in `runner/src/server/wave_sender.rs`:

```rust
#[tokio::test]
async fn cancelled_start_after_observer_shutdown_does_not_underflow_in_flight() {
    let (observer_tx, observer_rx) = mpsc::unbounded_channel::<ObserverCommand<usize>>();
    drop(observer_rx);

    let metric_tx = mpsc::unbounded_channel().0;
    let response_in_flight = Arc::new(AtomicUsize::new(0));
    let token = tokio_util::sync::CancellationToken::new();
    token.cancel();

    let started = Instant::now();
    let request = test_ready_wave_request(1, started, 0, 60_000);

    start_ready_request(
        Arc::new(Client::new()),
        started,
        metric_tx,
        request,
        Arc::clone(&response_in_flight),
        observer_tx,
        token,
    )
    .await;

    assert_eq!(response_in_flight.load(Ordering::SeqCst), 0);
}
```

- [ ] **Step 2: Run the shutdown test**

Run:

```bash
cargo test -p previa-runner cancelled_start_after_observer_shutdown_does_not_underflow_in_flight -- --nocapture
```

Expected: PASS after Task 1.

- [ ] **Step 3: Keep final wave snapshot from reporting impossible values**

In `runner/src/server/wave_executor.rs`, keep using `response_in_flight.load(Ordering::SeqCst)` for final snapshots, but ensure all writer paths use `decrement_response_in_flight`. No new clamp should be added here unless a test still produces impossible values. The source of truth must be fixed in `wave_sender.rs`, not masked in `wave_executor.rs`.

- [ ] **Step 4: Run executor and metrics tests**

Run:

```bash
cargo test -p previa-runner wave_executor wave_metrics_actor metrics -- --nocapture
```

Expected: all matching tests pass.

---

### Task 4: Expose `sendStarted` vs `httpStarted` More Clearly In The UI

**Files:**
- Modify: `app/src/lib/load-lifecycle-chart.ts`
- Modify: `app/src/components/LoadTestResultsPanel.tsx`
- Test: `app/src/components/LoadTestResultsPanel.test.tsx`

- [ ] **Step 1: Add/adjust chart test**

In `app/src/components/LoadTestResultsPanel.test.tsx`, add a test that renders lifecycle buckets with `sendStarted` and `httpStarted` divergence and asserts both labels appear:

```tsx
it("shows send and http start lifecycle series separately", () => {
  render(
    <LoadTestResultsPanel
      run={{
        id: "run-1",
        executionId: "exec-1",
        pipelineName: "Wave",
        status: "completed",
        startedAt: new Date(0).toISOString(),
        finishedAt: new Date(120_000).toISOString(),
        durationMs: 120_000,
        config: {
          load: {
            points: [
              { atMs: 0, intensity: 10 },
              { atMs: 60_000, intensity: 80 },
            ],
            interpolation: "step",
            gracePeriodMs: 30_000,
          },
        },
        metrics: {
          lifecycleBuckets: [
            {
              elapsedMs: 90_000,
              planned: 4_800,
              sendStarted: 4_800,
              httpStarted: 4_200,
              httpSendReturned: 3_000,
              responseBodyCompleted: 2_000,
            },
          ],
        },
        errors: [],
      }}
    />
  );

  expect(screen.getByText(/send started/i)).toBeInTheDocument();
  expect(screen.getByText(/http started/i)).toBeInTheDocument();
});
```

If the existing test helper uses another prop shape, adapt only the wrapper object, not the assertion intent.

- [ ] **Step 2: Run the UI test**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
```

Expected: FAIL if the chart does not expose both labels clearly.

- [ ] **Step 3: Update lifecycle chart labels**

In `app/src/lib/load-lifecycle-chart.ts`, ensure the lifecycle series includes these separate names:

```ts
{
  key: "sendStarted",
  label: "Send started",
}
{
  key: "httpStarted",
  label: "HTTP started",
}
```

Keep `planned`, `httpSendReturned`, and `responseBodyCompleted` as separate series. The visual goal is to make these gaps obvious:

- `planned` vs `sendStarted`: scheduler/dispatcher/sender queue delay.
- `sendStarted` vs `httpStarted`: HTTP start scheduling delay.
- `httpStarted` vs `httpSendReturned`: network/client send wait.
- `httpSendReturned` vs `responseBodyCompleted`: response/body/assertion observation delay.

- [ ] **Step 4: Run UI tests and typecheck**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel load-lifecycle-chart
cd app && ./node_modules/.bin/tsc --noEmit
```

Expected: tests and typecheck pass.

---

### Task 5: Full Verification, Restart, And Compare Against Latest Test

**Files:**
- No source edits expected.

- [ ] **Step 1: Run Rust tests**

Run:

```bash
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 2: Run release build**

Run:

```bash
cargo build --release
```

Expected: release build succeeds.

- [ ] **Step 3: Run frontend validation**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel load-lifecycle-chart
cd app && ./node_modules/.bin/tsc --noEmit
```

Expected: frontend tests and typecheck pass.

- [ ] **Step 4: Restart local stack**

Run:

```bash
screen -S previa-wave -X quit || true
sleep 1
for port in 5610 5611 5612 5613; do
  pids=$(lsof -ti tcp:$port || true)
  if [ -n "$pids" ]; then kill $pids || true; fi
done
sleep 1
screen -dmS previa-wave zsh -lc '
  cd /Users/assis/projects/previa
  RUST_LOG=info PORT=5611 target/release/previa-runner > /tmp/previa-runner-5611.log 2>&1 &
  RUST_LOG=info PORT=5612 target/release/previa-runner > /tmp/previa-runner-5612.log 2>&1 &
  RUST_LOG=info PORT=5613 target/release/previa-runner > /tmp/previa-runner-5613.log 2>&1 &
  RUST_LOG=info PREVIA_APP_ENABLED=1 ORCHESTRATOR_DATABASE_URL=sqlite:///private/tmp/previa-verify-5610.db PORT=5610 RUNNER_ENDPOINTS=http://127.0.0.1:5611,http://127.0.0.1:5612,http://127.0.0.1:5613 target/release/previa-main > /tmp/previa-main-5610.log 2>&1
'
sleep 2
curl -s http://127.0.0.1:5610/info | jq '{activeRunners,totalRunners,runners:[.runners[]? | {endpoint,runtime:.runtime.pid}]}'
```

Expected: `activeRunners` is `3`.

- [ ] **Step 5: Compare next run with current baseline**

After running the same load test again, query the latest run:

```bash
curl -s 'http://127.0.0.1:5610/api/v1/projects/019de1a7-4dfd-7662-8b53-a305e5714ca5/tests/load?limit=1&order=desc' \
  | jq '[.[] | {id,executionId,curveAdherence:.finalConsolidated.curveAdherence,scheduledStarts:.finalConsolidated.scheduledStarts,sendStarted:.finalConsolidated.sendStarted,httpStarted:.finalConsolidated.httpStarted,senderLaggedStarts:.finalConsolidated.senderLaggedStarts,readyRequests:.finalConsolidated.readyRequests,outstandingRequests:.finalConsolidated.outstandingRequests,activePipelines:.finalConsolidated.activePipelines}]'
```

Expected:
- `outstandingRequests` is not `18446744073709551615`.
- `activePipelines` is not `18446744073709551615`.
- `sendStarted == scheduledStarts` or very close.
- `httpStarted == sendStarted` or closer than the latest baseline.
- `curveAdherence` remains at or above the latest baseline of `92.12%`.

- [ ] **Step 6: Commit and push**

Run:

```bash
git add runner/src/server/wave_sender.rs runner/src/server/wave_executor.rs runner/src/server/metrics.rs runner/src/server/wave_metrics_actor.rs app/src/lib/load-lifecycle-chart.ts app/src/components/LoadTestResultsPanel.tsx app/src/components/LoadTestResultsPanel.test.tsx
git commit -m "Improve wave sender start accuracy"
git push origin codex/wave-load-test
```

Expected: commit and push succeed.

---

## Self-Review

- Spec coverage: fixes counter underflow, reduces sender runtime scheduling pressure, and makes `sendStarted` vs `httpStarted` visible.
- Placeholder scan: no TBD/TODO/fill-later instructions.
- Type consistency: uses existing Rust names `WaveMetricEvent`, `ReadyWaveRequest`, `ObserverCommand`, `LoadTestResultsPanel`, `lifecycleBuckets`.
