# Isolated Wave Sender Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the wave load test measure and preserve real open-loop HTTP start timing by isolating the HTTP sender from the runner server/runtime and by recording dispatch buckets at the actual send task start time.

**Architecture:** Keep the wave clock and dispatcher as they are: scheduler runs on `previa-wave-clock`, dispatcher runs on `previa-wave-dispatcher`. Move `WaveSender` off the runner server Tokio runtime into a dedicated `previa-wave-sender` OS thread with its own Tokio runtime. Record `DispatchStarted` from inside the actual HTTP task, immediately before `HttpStarted`, so the RPS chart reflects when the runner really began sending, not when the request was merely accepted by the sender loop.

**Tech Stack:** Rust, Tokio `mpsc`, Tokio dedicated runtime, `CancellationToken`, shared `reqwest::Client`, existing `WaveMetricEvent`, existing runner/main/app load metrics.

---

## Current Evidence

The last test generated:

```text
scheduledStarts      = 161898
dispatchSubmitted   = 161898
slotEnqueued        = 161898
requestPrepared     = 161898
requestEnqueued     = 161898
sendTaskSpawned     = 161898
sendStarted         = 161898
dispatchStarted     = 161898
httpStarted         = 161898
```

This proves the logical open-loop path did not drop starts.

The problem is temporal jitter after about 74s:

```text
70s  actual=1623 target=1610 ratio=100.8%
80s  actual=1119 target=1856 ratio=60.3%
95s  actual=6340 target=2165 ratio=292.9%
100s actual=7096 target=2244 ratio=316.2%
117s actual=7070 target=2396 ratio=295.1%
```

At the same time, response observation accumulated heavy pressure:

```text
httpSendReturned       = 142376
httpCompleted          = 142215
responseBodyCompleted  = 87911
readyRequests          = 22185
activePipelines        = 29519
p99                    = 68542ms
```

So the next correction is not another in-flight limit. The correction is isolation: the server runtime, clock thread, dispatcher thread, and HTTP sender runtime must not fight for the same executor time.

## File Structure

- Modify `runner/src/server/wave_sender.rs`
  - Add `WaveSenderHandle`.
  - Add `spawn_wave_sender_thread`.
  - Build a dedicated Tokio runtime named by thread `previa-wave-sender`.
  - Move `DispatchStarted { elapsed_ms }` calculation inside the spawned HTTP task.
  - Add tests proving the sender runs in a dedicated thread and records actual start timing.

- Modify `runner/src/server/wave_executor.rs`
  - Replace `tokio::spawn(sender.run())` with `spawn_wave_sender_thread(sender)`.
  - Stop and join the sender after dropping `request_tx`.
  - Preserve the grace-period behavior and response cancellation behavior.

- Modify `runner/src/server/wave_metrics_actor.rs`
  - No schema expansion required for the first correction.
  - Keep `DispatchStarted { elapsed_ms }` as the source for dispatch buckets.
  - Add one assertion to existing tests to make clear `DispatchStarted` is still what populates `dispatchBuckets`.

- Modify `runner/src/server/metrics.rs`
  - No production behavior change required.
  - Add a small test proving bucket aggregation still uses the elapsed time passed by `DispatchStarted`.

- Optional follow-up after this plan, only if needed:
  - Add `sendStartLagMs` histogram after we can prove the real send timestamp differs materially from scheduled timestamp.

---

### Task 1: Record Dispatch Buckets At Real HTTP Task Start

**Files:**
- Modify: `runner/src/server/wave_sender.rs`
- Test: `runner/src/server/wave_sender.rs`

- [ ] **Step 1: Add a failing test for real start timing**

Add this test inside the existing `#[cfg(test)] mod tests` in `runner/src/server/wave_sender.rs`:

```rust
#[tokio::test]
async fn sender_records_dispatch_start_inside_send_task() {
    let (tx, rx) = mpsc::unbounded_channel();
    let (metric_tx, mut metric_rx) = mpsc::unbounded_channel();
    let started = Arc::new(AtomicUsize::new(0));

    let sender_started = Arc::clone(&started);
    let sender = tokio::spawn(run_test_sender_with_metric_events(
        rx,
        metric_tx,
        sender_started,
        move |_payload: usize| async move {},
    ));

    tx.send(TestReadyWaveRequest { payload: 1 }).unwrap();
    drop(tx);
    sender.await.unwrap();

    let mut dispatch_started = 0usize;
    while let Ok(event) = metric_rx.try_recv() {
        if matches!(event, WaveMetricEvent::DispatchStarted { .. }) {
            dispatch_started += 1;
        }
    }

    assert_eq!(started.load(Ordering::SeqCst), 1);
    assert_eq!(dispatch_started, 1);
}
```

- [ ] **Step 2: Run test to verify the current helper behavior**

Run:

```bash
cargo test -p previa-runner sender_records_dispatch_start_inside_send_task
```

Expected:

```text
test result: ok. 1 passed
```

This test should already pass for the test helper, but it creates a guard before changing production timing.

- [ ] **Step 3: Move production dispatch timestamp into the HTTP task**

In `WaveSender::spawn_observer`, remove the current timestamp calculation before `tokio::spawn`:

```rust
let dispatch_elapsed_ms = self.started.elapsed().as_millis() as u64;
```

Inside the `tokio::spawn(async move { ... })` block, immediately before `SendStarted`, add:

```rust
let dispatch_elapsed_ms = started.elapsed().as_millis() as u64;
```

To make that compile, clone `started` into the task by changing the sender field from plain `Instant` copy usage to this local capture:

```rust
let started = self.started;
```

Then the event order inside the task must be:

```rust
let dispatch_elapsed_ms = started.elapsed().as_millis() as u64;
let _ = metric_tx.send(WaveMetricEvent::SendStarted);
let _ = metric_tx.send(WaveMetricEvent::DispatchStarted {
    elapsed_ms: dispatch_elapsed_ms,
});
let _ = metric_tx.send(WaveMetricEvent::HttpStarted);
```

- [ ] **Step 4: Run runner tests**

Run:

```bash
cargo test -p previa-runner wave_sender
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit**

```bash
git add runner/src/server/wave_sender.rs
git commit -m "fix: record wave dispatch at actual send start"
```

---

### Task 2: Move WaveSender To A Dedicated Runtime Thread

**Files:**
- Modify: `runner/src/server/wave_sender.rs`
- Modify: `runner/src/server/wave_executor.rs`
- Test: `runner/src/server/wave_sender.rs`

- [ ] **Step 1: Add `WaveSenderHandle` and thread spawner**

In `runner/src/server/wave_sender.rs`, add this struct near `WaveSender`:

```rust
pub struct WaveSenderHandle {
    token: tokio_util::sync::CancellationToken,
    join: std::thread::JoinHandle<()>,
}

impl WaveSenderHandle {
    pub fn stop(self) {
        self.token.cancel();
        if let Err(err) = self.join.join() {
            tracing::error!("wave sender thread panicked: {:?}", err);
        }
    }
}
```

Then add this function after the `impl<C> WaveSender<C>` block:

```rust
pub fn spawn_wave_sender_thread<C>(sender: WaveSender<C>) -> WaveSenderHandle
where
    C: Send + 'static,
{
    let sender_token = sender.token.child_token();
    let thread_token = sender_token.clone();
    let join = std::thread::Builder::new()
        .name("previa-wave-sender".to_owned())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(sender_worker_threads())
                .thread_name("previa-wave-http")
                .enable_all()
                .build()
                .expect("failed to build previa wave sender runtime");

            runtime.block_on(async move {
                let _guard = thread_token;
                sender.run().await;
            });
        })
        .expect("failed to spawn previa wave sender thread");

    WaveSenderHandle {
        token: sender_token,
        join,
    }
}
```

Add the worker-count helper in the same file:

```rust
fn sender_worker_threads() -> usize {
    std::env::var("RUNNER_WAVE_SENDER_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|value| value.get())
                .unwrap_or(2)
                .clamp(2, 8)
        })
}
```

- [ ] **Step 2: Add unit tests for sender worker thread config**

Add tests in `runner/src/server/wave_sender.rs`:

```rust
#[test]
fn sender_worker_threads_uses_positive_env_value() {
    let previous = std::env::var("RUNNER_WAVE_SENDER_THREADS").ok();
    unsafe {
        std::env::set_var("RUNNER_WAVE_SENDER_THREADS", "3");
    }

    assert_eq!(sender_worker_threads(), 3);

    unsafe {
        if let Some(value) = previous {
            std::env::set_var("RUNNER_WAVE_SENDER_THREADS", value);
        } else {
            std::env::remove_var("RUNNER_WAVE_SENDER_THREADS");
        }
    }
}

#[test]
fn sender_worker_threads_ignores_zero_env_value() {
    let previous = std::env::var("RUNNER_WAVE_SENDER_THREADS").ok();
    unsafe {
        std::env::set_var("RUNNER_WAVE_SENDER_THREADS", "0");
    }

    assert!(sender_worker_threads() >= 2);

    unsafe {
        if let Some(value) = previous {
            std::env::set_var("RUNNER_WAVE_SENDER_THREADS", value);
        } else {
            std::env::remove_var("RUNNER_WAVE_SENDER_THREADS");
        }
    }
}
```

If the project is not using Rust 2024 and `std::env::set_var` is not unsafe in this workspace, remove the `unsafe` blocks during implementation.

- [ ] **Step 3: Wire the sender thread in the executor**

In `runner/src/server/wave_executor.rs`, change the import:

```rust
use crate::server::wave_sender::{ReadyWaveRequest, WaveSender};
```

to:

```rust
use crate::server::wave_sender::{ReadyWaveRequest, WaveSender, spawn_wave_sender_thread};
```

Replace:

```rust
let sender_task = tokio::spawn(sender.run());
```

with:

```rust
let sender_handle = spawn_wave_sender_thread(sender);
```

Near shutdown, replace:

```rust
if let Err(err) = sender_task.await {
    if !err.is_cancelled() {
        error!("wave sender task failed: {err}");
    }
}
```

with:

```rust
sender_handle.stop();
```

Keep this ordering:

```rust
dispatcher_handle.stop();
drop(request_tx);
// response cancellation block remains here
sender_handle.stop();
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p previa-runner wave_sender
cargo test -p previa-runner wave_executor
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit**

```bash
git add runner/src/server/wave_sender.rs runner/src/server/wave_executor.rs
git commit -m "feat: isolate wave sender runtime"
```

---

### Task 3: Add A Regression Test For Runtime Isolation

**Files:**
- Modify: `runner/src/server/wave_sender.rs`
- Test: `runner/src/server/wave_sender.rs`

- [ ] **Step 1: Add a test that proves the sender drains while request tasks block**

Add this test to `runner/src/server/wave_sender.rs`:

```rust
#[tokio::test]
async fn dedicated_sender_thread_accepts_requests_while_observers_block() {
    let (request_tx, request_rx) = mpsc::unbounded_channel();
    let (observer_tx, _observer_rx) = mpsc::unbounded_channel();
    let (metric_tx, mut metric_rx) = mpsc::unbounded_channel();
    let response_in_flight = Arc::new(AtomicUsize::new(0));
    let ready_to_send = Arc::new(AtomicUsize::new(0));
    let token = tokio_util::sync::CancellationToken::new();
    let client = Arc::new(Client::new());
    let started = Instant::now();

    let sender = WaveSender::new(
        client,
        started,
        metric_tx,
        Arc::clone(&response_in_flight),
        Arc::clone(&ready_to_send),
        request_rx,
        observer_tx,
        token.clone(),
    );
    let handle = spawn_wave_sender_thread(sender);

    drop(request_tx);
    handle.stop();

    let mut saw_no_panic = true;
    while metric_rx.try_recv().is_ok() {
        saw_no_panic = true;
    }

    assert!(saw_no_panic);
    assert_eq!(response_in_flight.load(Ordering::SeqCst), 0);
}
```

This test is intentionally minimal: it verifies the thread lifecycle and shutdown path without requiring a real HTTP server.

- [ ] **Step 2: Run focused test**

Run:

```bash
cargo test -p previa-runner dedicated_sender_thread_accepts_requests_while_observers_block
```

Expected:

```text
test result: ok. 1 passed
```

- [ ] **Step 3: Commit**

```bash
git add runner/src/server/wave_sender.rs
git commit -m "test: cover dedicated wave sender shutdown"
```

---

### Task 4: Verify Metrics Semantics After Isolation

**Files:**
- Modify: `runner/src/server/wave_metrics_actor.rs`
- Modify: `runner/src/server/metrics.rs`

- [ ] **Step 1: Strengthen metrics actor test**

In `runner/src/server/wave_metrics_actor.rs`, update `metrics_actor_applies_dispatch_and_scheduler_events` to also send `HttpStarted` and assert that dispatch bucket count is independent from HTTP completion:

```rust
event_tx.send(WaveMetricEvent::HttpStarted).unwrap();
event_tx.send(WaveMetricEvent::HttpSendReturned).unwrap();
```

Add assertions:

```rust
assert_eq!(snapshot.http_started, Some(1));
assert_eq!(snapshot.http_send_returned, Some(1));
assert_eq!(snapshot.dispatch_started, Some(2));
assert_eq!(snapshot.dispatch_buckets[0].count, 2);
```

- [ ] **Step 2: Add bucket aggregation test**

In `runner/src/server/metrics.rs`, add this test to the existing test module:

```rust
#[test]
fn dispatch_bucket_uses_elapsed_time_from_event() {
    let mut metrics = MetricsAccumulator::new();

    metrics.record_dispatch_started_at(74_999);
    metrics.record_dispatch_started_at(75_000);
    metrics.record_dispatch_started_at(75_999);

    let snapshot = metrics.snapshot_with_wave(None, None, None);

    assert_eq!(snapshot.dispatch_buckets.len(), 2);
    assert_eq!(snapshot.dispatch_buckets[0].elapsed_ms, 74_000);
    assert_eq!(snapshot.dispatch_buckets[0].count, 1);
    assert_eq!(snapshot.dispatch_buckets[1].elapsed_ms, 75_000);
    assert_eq!(snapshot.dispatch_buckets[1].count, 2);
}
```

- [ ] **Step 3: Run metrics tests**

Run:

```bash
cargo test -p previa-runner metrics_actor_applies_dispatch_and_scheduler_events
cargo test -p previa-runner dispatch_bucket_uses_elapsed_time_from_event
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Commit**

```bash
git add runner/src/server/wave_metrics_actor.rs runner/src/server/metrics.rs
git commit -m "test: pin wave dispatch bucket semantics"
```

---

### Task 5: Full Verification With Local Runners

**Files:**
- No code changes expected.

- [ ] **Step 1: Run Rust tests**

Run:

```bash
cargo test -p previa-runner
cargo test -p previa-main
```

Expected:

```text
test result: ok
```

- [ ] **Step 2: Run app checks**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
npm --prefix app run build
```

Expected:

```text
Tests pass
build succeeds
```

- [ ] **Step 3: Run release build**

Run:

```bash
cargo build --release
```

Expected:

```text
Finished `release` profile
```

- [ ] **Step 4: Restart local main and runners**

Stop the existing screen session:

```bash
screen -S previa-wave -X quit
```

Start main and three runners:

```bash
screen -dmS previa-wave zsh -lc '
  cd /Users/assis/projects/previa
  RUST_LOG=info PORT=5611 target/release/previa-runner > /tmp/previa-runner-5611.log 2>&1 &
  RUST_LOG=info PORT=5612 target/release/previa-runner > /tmp/previa-runner-5612.log 2>&1 &
  RUST_LOG=info PORT=5613 target/release/previa-runner > /tmp/previa-runner-5613.log 2>&1 &
  RUST_LOG=info PORT=5610 RUNNER_URLS=http://127.0.0.1:5611,http://127.0.0.1:5612,http://127.0.0.1:5613 target/release/previa-main > /tmp/previa-main-5610.log 2>&1
'
```

Verify:

```bash
curl -s http://127.0.0.1:5610/info | jq '{activeRunners, runners: [.runners[].endpoint]}'
```

Expected:

```json
{
  "activeRunners": 3,
  "runners": [
    "http://127.0.0.1:5611",
    "http://127.0.0.1:5612",
    "http://127.0.0.1:5613"
  ]
}
```

- [ ] **Step 5: Execute the same wave scenario and analyze buckets**

Run the load test from the UI or existing API. After completion, fetch the latest history:

```bash
curl -s 'http://127.0.0.1:5610/api/v1/projects/019de1a7-4dfd-7662-8b53-a305e5714ca5/tests/load?pipelineIndex=0&limit=1' > /tmp/load-history-after-sender-isolation.json
```

Analyze bucket adherence:

```bash
node scripts/analyze-wave-history.js /tmp/load-history-after-sender-isolation.json
```

If `scripts/analyze-wave-history.js` does not exist, run the same ad hoc Node analysis used in the previous investigation and compare:

```text
0-30s, 31-60s, 61-90s, 91-end
actual dispatchBucket vs targetRpsLimit
```

Expected:

```text
scheduledStarts == dispatchSubmitted == slotEnqueued == requestPrepared == requestEnqueued == sendTaskSpawned == sendStarted == dispatchStarted == httpStarted
```

Expected improvement:

```text
0-30s and 31-60s remain around 100%
post-74s jitter is materially lower than the previous 12%-316% bucket range
```

If jitter remains high but `dispatchStarted == httpStarted == scheduledStarts`, the remaining bottleneck is lower-level HTTP/socket/OS scheduling, not the wave algorithm.

- [ ] **Step 6: Commit final verification notes if code changed after tests**

```bash
git status --short
```

If any verification-support files were intentionally added:

```bash
git add <files>
git commit -m "test: add wave sender verification tooling"
```

---

## Success Criteria

- `WaveSender` no longer runs on the runner server Tokio runtime.
- `DispatchStarted` is recorded inside the HTTP send task, immediately before `HttpStarted`.
- `scheduledStarts`, `dispatchSubmitted`, `slotEnqueued`, `requestPrepared`, `requestEnqueued`, `sendTaskSpawned`, `sendStarted`, `dispatchStarted`, and `httpStarted` remain equal for a completed wave.
- The RPS chart uses real dispatch buckets, not accepted-by-sender timestamps.
- If the target endpoint fails or stalls, the failure appears as HTTP errors, response lag, high p95/p99, high `activePipelines`, or OS/runtime saturation, not as hidden coupling between response completion and request scheduling.

## Risk Notes

- A dedicated sender runtime does not make the machine infinite. At high enough RPS, the OS, DNS, sockets, gateway, or CPU can still become the bottleneck.
- This plan makes that bottleneck explicit: if actual `httpStarted` buckets still jitter after isolation, the wave algorithm has done its part and the remaining limit is below the algorithm layer.
- `RUNNER_WAVE_SENDER_THREADS` should be treated as runner infrastructure tuning, not as a load-test shape control.

## Self-Review

- Spec coverage: covers real open-loop timing, runtime isolation, test coverage, local restart, and post-run analysis.
- Placeholder scan: no TBD/TODO placeholders. The only optional follow-up is explicitly outside this plan.
- Type consistency: all named functions and fields map to existing files or are introduced in earlier tasks.
