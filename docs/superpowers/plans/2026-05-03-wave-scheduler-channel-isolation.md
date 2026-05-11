# Wave Scheduler Channel Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Isolate the wave clock from HTTP response work so the load runner behaves as an open-loop signal generator until local infrastructure itself cannot schedule work on time.

**Architecture:** Split the current wave loop into actors connected by Tokio channels. A lightweight scheduler actor owns time and emits dispatch slots; a dispatcher actor owns pipeline cursors and request preparation; sender/observer actors own HTTP work; metrics aggregation receives event messages and must not block slot generation.

**Tech Stack:** Rust, Tokio `mpsc`, `CancellationToken`, `reqwest::Client`, existing `DispatchClock`, `prepare_http_step`, `WaveSender`, main/app metrics already added in the open-loop diagnostics work.

---

## Correctness Contract

- The scheduler actor must not await HTTP sends, HTTP responses, body reads, assertion evaluation, pipeline continuation, or metrics serialization.
- The scheduler actor may await only its timer and a nonblocking/bounded send of a lightweight slot message.
- If the scheduler cannot enqueue a slot message because the local runner is saturated, it records scheduler backpressure as runner/infra lag.
- Response observers may feed pipeline continuations back to the dispatcher, but they must never block creation of new first-step pipeline cursors.
- Metrics updates on the hot path must use channel messages or short nonblocking operations; no heavy `Arc<Mutex<MetricsAccumulator>>` lock should sit inside the scheduler loop.
- `dispatchStarted` remains the primary RPS curve metric. `httpSendReturned`, `responseBodyCompleted`, `schedulerLagMs`, `schedulerLaggedStarts`, and observer backlog remain diagnostics.

## File Structure

- Create `runner/src/server/wave_scheduler.rs`
  - Owns `DispatchClock`, timer ticks, wave sampling, and slot emission.
  - Emits `WaveDispatchSlot` through `mpsc`.
  - Emits `WaveMetricEvent` for scheduler lag and scheduled slots.

- Create `runner/src/server/wave_metrics_actor.rs`
  - Owns `MetricsAccumulator`.
  - Receives lightweight `WaveMetricEvent` messages.
  - Serves snapshots through a `watch` channel so SSE can read without locking the hot path.

- Modify `runner/src/server/wave_executor.rs`
  - Becomes orchestration glue for actors.
  - Moves cursor/prepare logic into a dispatcher loop fed by scheduler slots.
  - Keeps bounded observer draining out of scheduler path.

- Modify `runner/src/server/wave_sender.rs`
  - Replace direct metrics mutex writes with `WaveMetricEvent` sends.
  - Keep detached response observers.

- Modify `runner/src/server/mod.rs`
  - Register new modules.

- Modify tests in:
  - `runner/src/server/load_dispatch.rs`
  - `runner/src/server/wave_executor.rs`
  - `runner/src/server/wave_sender.rs`
  - New `runner/src/server/wave_scheduler.rs`
  - New `runner/src/server/wave_metrics_actor.rs`

---

### Task 1: Add Slot Message and Scheduler Actor

**Files:**
- Create: `runner/src/server/wave_scheduler.rs`
- Modify: `runner/src/server/mod.rs`
- Test: `runner/src/server/wave_scheduler.rs`

- [ ] **Step 1: Create failing scheduler tests**

Create `runner/src/server/wave_scheduler.rs` with the tests first:

```rust
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::server::load_dispatch::DispatchClock;
use crate::server::models::LoadProfile;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveDispatchSlot {
    pub elapsed_ms: u64,
    pub planned_starts: usize,
    pub target_rps_limit: f64,
    pub scheduled_total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaveSchedulerMetric {
    DispatchScheduled { count: usize },
    SchedulerLag { lag_ms: u64, missed_starts: usize },
    SlotBackpressure { dropped_starts: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wave_load() -> LoadProfile {
        LoadProfile {
            points: vec![
                crate::server::models::LoadPoint { at_ms: 0, intensity: 10.0 },
                crate::server::models::LoadPoint { at_ms: 1000, intensity: 10.0 },
            ],
            interpolation: crate::server::models::LoadInterpolation::Linear,
            runner_max_rps: 1000.0,
            grace_period_ms: 0,
        }
    }

    #[test]
    fn build_slot_from_clock_tick_uses_tick_window_only() {
        let mut clock = DispatchClock::new(100);
        let tick = clock.plan_tick(500, 100.0);
        let slot = WaveDispatchSlot {
            elapsed_ms: tick.elapsed_ms,
            planned_starts: tick.scheduled_starts,
            target_rps_limit: tick.target_rps,
            scheduled_total: tick.scheduled_total,
        };

        assert_eq!(slot.planned_starts, 10);
        assert_eq!(slot.elapsed_ms, 500);
        assert_eq!(slot.target_rps_limit, 100.0);
    }

    #[tokio::test]
    async fn scheduler_emits_metric_when_slot_channel_is_full() {
        let (slot_tx, mut slot_rx) = mpsc::channel(1);
        let (metric_tx, mut metric_rx) = mpsc::unbounded_channel();

        slot_tx
            .send(WaveDispatchSlot {
                elapsed_ms: 0,
                planned_starts: 1,
                target_rps_limit: 10.0,
                scheduled_total: 1,
            })
            .await
            .unwrap();

        let sent = try_send_slot_or_metric(
            &slot_tx,
            &metric_tx,
            WaveDispatchSlot {
                elapsed_ms: 100,
                planned_starts: 7,
                target_rps_limit: 70.0,
                scheduled_total: 8,
            },
        );

        assert!(!sent);
        assert!(matches!(
            metric_rx.recv().await,
            Some(WaveSchedulerMetric::SlotBackpressure { dropped_starts: 7 })
        ));

        assert!(slot_rx.recv().await.is_some());
    }
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test -p previa-runner wave_scheduler
```

Expected: FAIL because `try_send_slot_or_metric` and the module export do not exist yet.

- [ ] **Step 3: Implement nonblocking slot enqueue helper**

Add this implementation below the type definitions:

```rust
pub fn try_send_slot_or_metric(
    slot_tx: &mpsc::Sender<WaveDispatchSlot>,
    metric_tx: &mpsc::UnboundedSender<WaveSchedulerMetric>,
    slot: WaveDispatchSlot,
) -> bool {
    if slot.planned_starts == 0 {
        return true;
    }

    match slot_tx.try_send(slot) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(slot)) => {
            let _ = metric_tx.send(WaveSchedulerMetric::SlotBackpressure {
                dropped_starts: slot.planned_starts,
            });
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => false,
    }
}
```

- [ ] **Step 4: Register the module**

In `runner/src/server/mod.rs`, add:

```rust
pub mod wave_scheduler;
```

- [ ] **Step 5: Verify GREEN**

Run:

```bash
cargo test -p previa-runner wave_scheduler
```

Expected: PASS.

---

### Task 2: Add Metrics Actor for Hot-Path Events

**Files:**
- Create: `runner/src/server/wave_metrics_actor.rs`
- Modify: `runner/src/server/mod.rs`
- Test: `runner/src/server/wave_metrics_actor.rs`

- [ ] **Step 1: Create failing metrics actor tests**

Create `runner/src/server/wave_metrics_actor.rs` with:

```rust
use tokio::sync::{mpsc, watch};

use crate::server::metrics::{MetricsAccumulator, WaveMetricsSnapshot};
use crate::server::models::LoadTestMetrics;
use crate::server::wave_scheduler::WaveSchedulerMetric;

#[derive(Debug, Clone)]
pub enum WaveMetricEvent {
    Scheduler(WaveSchedulerMetric),
    PipelineStarted,
    DispatchSubmitted(usize),
    DispatchStarted,
    HttpStarted,
    HttpSendReturned,
    HttpCompleted(usize),
    ResponseBodyCompleted(usize),
    PipelineFinished { duration_ms: f64, success: bool },
    ErrorSample {
        step_id: String,
        http_status: Option<u16>,
        error: String,
    },
    NetworkBytes { tx: u64, rx: u64 },
    RuntimeLaggedStart,
    DependencyLimitedStarts(usize),
    SnapshotWave(WaveMetricsSnapshot),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn metrics_actor_applies_dispatch_and_scheduler_events() {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (snapshot_tx, snapshot_rx) = watch::channel(LoadTestMetrics::default());

        let actor = tokio::spawn(run_wave_metrics_actor(event_rx, snapshot_tx));

        event_tx.send(WaveMetricEvent::DispatchSubmitted(3)).unwrap();
        event_tx.send(WaveMetricEvent::DispatchStarted).unwrap();
        event_tx.send(WaveMetricEvent::DispatchStarted).unwrap();
        event_tx
            .send(WaveMetricEvent::Scheduler(WaveSchedulerMetric::SchedulerLag {
                lag_ms: 25,
                missed_starts: 4,
            }))
            .unwrap();
        drop(event_tx);

        actor.await.unwrap();
        let snapshot = snapshot_rx.borrow().clone();

        assert_eq!(snapshot.dispatch_submitted, Some(3));
        assert_eq!(snapshot.total_started, 0);
        assert_eq!(snapshot.dispatch_started, Some(2));
        assert_eq!(snapshot.scheduler_lag_ms, Some(25));
        assert_eq!(snapshot.scheduler_lagged_starts, Some(4));
    }
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cargo test -p previa-runner wave_metrics_actor
```

Expected: FAIL because `run_wave_metrics_actor` does not exist and `LoadTestMetrics::default()` has not been implemented.

- [ ] **Step 3: Add `Default` for `LoadTestMetrics`**

`LoadTestMetrics` currently has no default. Add this impl in `runner/src/server/models.rs`:

```rust
impl Default for LoadTestMetrics {
    fn default() -> Self {
        Self {
            total_started: 0,
            total_sent: 0,
            total_success: 0,
            total_error: 0,
            http_started: 0,
            http_completed: 0,
            dispatch_submitted: None,
            dispatch_started: None,
            http_send_returned: None,
            response_body_completed: None,
            dependency_limited_starts: None,
            runtime_lagged_starts: None,
            scheduler_lag_ms: None,
            scheduler_lagged_starts: None,
            rps: 0.0,
            latency_buckets: Vec::new(),
            latency_sample_count: None,
            latency_total_duration_ms: None,
            error_samples: Vec::new(),
            start_time: crate::server::utils::now_ms(),
            elapsed_ms: 0,
            duration_ms: None,
            target_intensity: None,
            target_rps_limit: None,
            in_flight: None,
            runner_max_rps: None,
            tick_ms: None,
            scheduled_starts: None,
            missed_starts: None,
            ready_requests: None,
            active_pipelines: None,
            outstanding_requests: None,
            curve_adherence: None,
            runtime: None,
        }
    }
}
```

- [ ] **Step 4: Implement the actor**

Add this implementation:

```rust
pub async fn run_wave_metrics_actor(
    mut event_rx: mpsc::UnboundedReceiver<WaveMetricEvent>,
    snapshot_tx: watch::Sender<LoadTestMetrics>,
) {
    let mut accumulator = MetricsAccumulator::new();
    let mut latest_wave: Option<WaveMetricsSnapshot> = None;

    while let Some(event) = event_rx.recv().await {
        match event {
            WaveMetricEvent::Scheduler(WaveSchedulerMetric::DispatchScheduled { count }) => {
                accumulator.record_dispatch_submitted_count(count);
            }
            WaveMetricEvent::Scheduler(WaveSchedulerMetric::SchedulerLag {
                lag_ms,
                missed_starts,
            }) => {
                accumulator.record_scheduler_lag_ms(lag_ms);
                accumulator.record_scheduler_lagged_starts_count(missed_starts);
            }
            WaveMetricEvent::Scheduler(WaveSchedulerMetric::SlotBackpressure { dropped_starts }) => {
                accumulator.record_scheduler_lagged_starts_count(dropped_starts);
            }
            WaveMetricEvent::PipelineStarted => accumulator.record_start(),
            WaveMetricEvent::DispatchSubmitted(count) => {
                accumulator.record_dispatch_submitted_count(count);
            }
            WaveMetricEvent::DispatchStarted => accumulator.record_dispatch_started(),
            WaveMetricEvent::HttpStarted => accumulator.record_http_start(),
            WaveMetricEvent::HttpSendReturned => accumulator.record_http_send_returned(),
            WaveMetricEvent::HttpCompleted(count) => {
                accumulator.record_http_completed_count(count);
            }
            WaveMetricEvent::ResponseBodyCompleted(count) => {
                accumulator.record_response_body_completed_count(count);
            }
            WaveMetricEvent::PipelineFinished { duration_ms, success } => {
                accumulator.update(duration_ms, success);
            }
            WaveMetricEvent::ErrorSample {
                step_id,
                http_status,
                error,
            } => accumulator.record_error_sample(&step_id, http_status, &error),
            WaveMetricEvent::NetworkBytes { tx, rx } => accumulator.add_network_bytes(tx, rx),
            WaveMetricEvent::RuntimeLaggedStart => accumulator.record_runtime_lagged_start(),
            WaveMetricEvent::DependencyLimitedStarts(count) => {
                accumulator.record_dependency_limited_starts_count(count);
            }
            WaveMetricEvent::SnapshotWave(wave) => latest_wave = Some(wave),
        }

        let snapshot = accumulator.snapshot_with_wave(None, None, latest_wave);
        let _ = snapshot_tx.send(snapshot);
    }
}
```

- [ ] **Step 5: Register the module**

In `runner/src/server/mod.rs`, add:

```rust
pub mod wave_metrics_actor;
```

- [ ] **Step 6: Verify GREEN**

Run:

```bash
cargo test -p previa-runner wave_metrics_actor
```

Expected: PASS.

---

### Task 3: Convert WaveSender to Metrics Events

**Files:**
- Modify: `runner/src/server/wave_sender.rs`
- Test: `runner/src/server/wave_sender.rs`

- [ ] **Step 1: Write failing sender metrics test**

Add a test that verifies sender startup records events without taking a metrics mutex:

```rust
#[tokio::test]
async fn sender_emits_dispatch_events_for_accepted_requests() {
    let (tx, rx) = mpsc::unbounded_channel();
    let (metric_tx, mut metric_rx) = mpsc::unbounded_channel();
    let started = Arc::new(AtomicUsize::new(0));

    let sender_started = Arc::clone(&started);
    let sender = tokio::spawn(run_test_sender_with_metric_events(
        rx,
        metric_tx,
        sender_started,
        |_payload: usize| async move {},
    ));

    tx.send(TestReadyWaveRequest { payload: 1 }).unwrap();
    tx.send(TestReadyWaveRequest { payload: 2 }).unwrap();
    drop(tx);

    sender.await.unwrap();

    let mut dispatch_started = 0;
    while let Ok(event) = metric_rx.try_recv() {
        if matches!(
            event,
            crate::server::wave_metrics_actor::WaveMetricEvent::DispatchStarted
        ) {
            dispatch_started += 1;
        }
    }

    assert_eq!(started.load(Ordering::SeqCst), 2);
    assert_eq!(dispatch_started, 2);
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```bash
cargo test -p previa-runner sender_emits_dispatch_events_for_accepted_requests
```

Expected: FAIL because `run_test_sender_with_metric_events` does not exist.

- [ ] **Step 3: Add test helper**

Add this test-only helper near `run_test_sender`:

```rust
#[cfg(test)]
pub async fn run_test_sender_with_metric_events<T, F, Fut>(
    mut rx: mpsc::UnboundedReceiver<TestReadyWaveRequest<T>>,
    metric_tx: mpsc::UnboundedSender<crate::server::wave_metrics_actor::WaveMetricEvent>,
    started: Arc<AtomicUsize>,
    mut send: F,
) where
    T: Send + 'static,
    F: FnMut(T) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let mut tasks = JoinSet::new();
    while let Some(request) = rx.recv().await {
        started.fetch_add(1, Ordering::SeqCst);
        let _ = metric_tx.send(crate::server::wave_metrics_actor::WaveMetricEvent::DispatchStarted);
        tasks.spawn(send(request.payload));
    }
    while tasks.join_next().await.is_some() {}
}
```

- [ ] **Step 4: Change `WaveSender` constructor fields**

Replace the metrics field:

```rust
metrics: Arc<tokio::sync::Mutex<MetricsAccumulator>>,
```

with:

```rust
metric_tx: mpsc::UnboundedSender<crate::server::wave_metrics_actor::WaveMetricEvent>,
```

Update `WaveSender::new` to accept `metric_tx` and store it.

- [ ] **Step 5: Replace direct metrics writes in `spawn_observer`**

At request start, replace the mutex block with:

```rust
let _ = metric_tx.send(WaveMetricEvent::DispatchStarted);
let _ = metric_tx.send(WaveMetricEvent::HttpStarted);
```

In `on_send_returned`, replace lock writes with:

```rust
let _ = metrics_for_send.send(WaveMetricEvent::HttpSendReturned);
```

In `on_body_completed`, replace lock writes with:

```rust
let _ = metrics_for_body.send(WaveMetricEvent::ResponseBodyCompleted(1));
```

After result completion, replace the final metrics block with:

```rust
if result.request.is_some() {
    let _ = metric_tx.send(WaveMetricEvent::HttpCompleted(1));
}
let _ = metric_tx.send(WaveMetricEvent::NetworkBytes {
    tx: network_tx_bytes,
    rx: network_rx_bytes,
});
```

- [ ] **Step 6: Verify sender tests**

Run:

```bash
cargo test -p previa-runner wave_sender
```

Expected: PASS.

---

### Task 4: Move Cursor Preparation Behind Scheduler Slots

**Files:**
- Modify: `runner/src/server/wave_executor.rs`
- Test: `runner/src/server/wave_executor.rs`

- [ ] **Step 1: Write failing dispatcher test**

Add this test in `wave_executor.rs`:

```rust
#[tokio::test]
async fn dispatcher_starts_new_pipeline_when_continuations_are_delayed() {
    let mut ready = VecDeque::new();
    let mut started_new = 0usize;

    for _ in 0..3 {
        let cursor = next_cursor_for_slot(&mut ready, || {
            started_new += 1;
            PipelineCursor::new(Instant::now())
        });
        assert_eq!(cursor.step_index, 0);
    }

    assert_eq!(started_new, 3);
    assert!(ready.is_empty());
}
```

- [ ] **Step 2: Run the test**

Run:

```bash
cargo test -p previa-runner dispatcher_starts_new_pipeline_when_continuations_are_delayed
```

Expected: PASS if current cursor behavior is already correct. This protects the architecture while moving it behind the channel boundary.

- [ ] **Step 3: Add dispatcher loop signature**

Extract the slot-consuming section from `run_wave_load` into a function:

```rust
async fn run_wave_dispatcher(
    mut slot_rx: mpsc::Receiver<WaveDispatchSlot>,
    request_tx: mpsc::UnboundedSender<ReadyWaveRequest<PipelineCursor>>,
    event_rx: &mut mpsc::UnboundedReceiver<ObserverEvent>,
    pipeline: Arc<Pipeline>,
    specs: Arc<Vec<RuntimeSpec>>,
    env_groups: Arc<Vec<RuntimeEnvGroup>>,
    selected_env_group_slug: Option<String>,
    metric_tx: mpsc::UnboundedSender<WaveMetricEvent>,
    ready_to_send: Arc<AtomicUsize>,
    started: Instant,
    tick_ms: u64,
    token: CancellationToken,
) {
    let mut ready = VecDeque::new();

    while let Some(slot) = slot_rx.recv().await {
        if token.is_cancelled() {
            break;
        }

        dispatch_slot_requests(
            slot,
            &mut ready,
            &pipeline,
            &specs,
            &env_groups,
            selected_env_group_slug.as_deref(),
            selected_env_group_slug.clone(),
            &request_tx,
            &metric_tx,
            &ready_to_send,
            started,
            tick_ms,
            &token,
        )
        .await;

        drain_observer_events_budgeted(
            event_rx,
            &mut ready,
            &pipeline,
            &metric_tx,
            OBSERVER_EVENTS_PER_TICK_BUDGET,
        )
        .await;
    }
}
```

- [ ] **Step 4: Extract request preparation into `dispatch_slot_requests`**

Move the current `for _ in 0..tick.scheduled_starts` body into:

```rust
async fn dispatch_slot_requests(
    slot: WaveDispatchSlot,
    ready: &mut VecDeque<PipelineCursor>,
    pipeline: &Pipeline,
    specs: &Arc<Vec<RuntimeSpec>>,
    env_groups: &Arc<Vec<RuntimeEnvGroup>>,
    selected_env_group_slug_ref: Option<&str>,
    selected_env_group_slug_owned: Option<String>,
    request_tx: &mpsc::UnboundedSender<ReadyWaveRequest<PipelineCursor>>,
    metric_tx: &mpsc::UnboundedSender<WaveMetricEvent>,
    ready_to_send: &Arc<AtomicUsize>,
    started: Instant,
    tick_ms: u64,
    token: &CancellationToken,
) {
    for _ in 0..slot.planned_starts {
        if token.is_cancelled() {
            break;
        }

        let was_ready_empty = ready.is_empty();
        let cursor = next_cursor_for_slot(ready, || PipelineCursor::new(Instant::now()));
        if was_ready_empty && cursor.step_index == 0 && cursor.context.is_empty() {
            let _ = metric_tx.send(WaveMetricEvent::PipelineStarted);
        }

        let Some(step) = pipeline.steps.get(cursor.step_index).cloned() else {
            record_terminal_pipeline(metric_tx, cursor, false, None).await;
            continue;
        };

        let max_attempts = max_attempts_for_step(&step);
        let prepared = match prepare_http_step(
            &step,
            &cursor.context,
            Some(specs.as_slice()),
            Some(env_groups.as_slice()),
            selected_env_group_slug_ref,
            cursor.attempt,
            max_attempts,
        ) {
            Ok(prepared) => prepared,
            Err(result) => {
                handle_prepare_error(
                    result,
                    cursor,
                    ready,
                    pipeline,
                    metric_tx,
                )
                .await;
                continue;
            }
        };

        let actual_elapsed_ms = started.elapsed().as_millis() as u64;
        if classify_start_lag(slot.elapsed_ms, actual_elapsed_ms, tick_ms)
            == StartLagClass::RuntimeLagged
        {
            let _ = metric_tx.send(WaveMetricEvent::RuntimeLaggedStart);
        }

        ready_to_send.fetch_add(1, Ordering::SeqCst);
        if request_tx
            .send(ReadyWaveRequest {
                step,
                context: cursor.context.clone(),
                cursor,
                prepared,
                specs: Arc::clone(specs),
                env_groups: Arc::clone(env_groups),
                selected_env_group_slug: selected_env_group_slug_owned.clone(),
            })
            .is_err()
        {
            ready_to_send.fetch_sub(1, Ordering::SeqCst);
            break;
        }
    }
}
```

- [ ] **Step 5: Update result handlers to send metric events**

Change `record_terminal_pipeline`, `handle_step_result`, and `handle_prepare_error` to receive `metric_tx` instead of `Arc<Mutex<MetricsAccumulator>>`.

Use:

```rust
let _ = metric_tx.send(WaveMetricEvent::PipelineFinished {
    duration_ms,
    success,
});
```

For errors:

```rust
let _ = metric_tx.send(WaveMetricEvent::ErrorSample {
    step_id: result.step_id.clone(),
    http_status,
    error: error.to_owned(),
});
```

For dependency-limited starts:

```rust
let _ = metric_tx.send(WaveMetricEvent::DependencyLimitedStarts(1));
```

- [ ] **Step 6: Verify wave executor tests**

Run:

```bash
cargo test -p previa-runner wave_executor
```

Expected: PASS.

---

### Task 5: Rewire `run_wave_load` as Actor Orchestrator

**Files:**
- Modify: `runner/src/server/wave_executor.rs`
- Test: `runner/src/server/wave_executor.rs`

- [ ] **Step 1: Add actor channels at the top of `run_wave_load`**

Replace direct `MetricsAccumulator` ownership with channels:

```rust
let (slot_tx, slot_rx) = mpsc::channel::<WaveDispatchSlot>(1024);
let (scheduler_metric_tx, mut scheduler_metric_rx) =
    mpsc::unbounded_channel::<WaveSchedulerMetric>();
let (metric_tx, metric_rx) = mpsc::unbounded_channel::<WaveMetricEvent>();
let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(LoadTestMetrics::default());
let scheduled_total_state = Arc::new(AtomicUsize::new(0));
let scheduler_lagged_total_state = Arc::new(AtomicUsize::new(0));
```

- [ ] **Step 2: Bridge scheduler metrics into metrics actor**

Spawn:

```rust
let metric_bridge_tx = metric_tx.clone();
let metric_bridge = tokio::spawn(async move {
    while let Some(event) = scheduler_metric_rx.recv().await {
        let _ = metric_bridge_tx.send(WaveMetricEvent::Scheduler(event));
    }
});
```

- [ ] **Step 3: Spawn metrics actor**

Spawn:

```rust
let metrics_task = tokio::spawn(run_wave_metrics_actor(metric_rx, snapshot_tx));
```

- [ ] **Step 4: Spawn scheduler actor**

Use the existing `DispatchClock`, `calculate_dispatch_tick_ms`, `local_rps_limit`, and `timeline_end_ms` inside a scheduler task:

```rust
let scheduler_load = load.clone();
let scheduler_token = token.child_token();
let scheduler_scheduled_total = Arc::clone(&scheduled_total_state);
let scheduler_lagged_total = Arc::clone(&scheduler_lagged_total_state);
let scheduler_task = tokio::spawn(async move {
    run_wave_scheduler(
        scheduler_load,
        tick_ms,
        slot_tx,
        scheduler_metric_tx,
        scheduler_scheduled_total,
        scheduler_lagged_total,
        scheduler_token,
    )
    .await;
});
```

Implement `run_wave_scheduler` in `wave_scheduler.rs`:

```rust
pub async fn run_wave_scheduler(
    load: LoadProfile,
    tick_ms: u64,
    slot_tx: mpsc::Sender<WaveDispatchSlot>,
    metric_tx: mpsc::UnboundedSender<WaveSchedulerMetric>,
    scheduled_total_state: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    scheduler_lagged_total_state: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    token: CancellationToken,
) {
    let started = std::time::Instant::now();
    let end_ms = crate::server::load_wave::timeline_end_ms(&load);
    let mut clock = DispatchClock::new(tick_ms);

    loop {
        if token.is_cancelled() {
            break;
        }

        let elapsed_ms = started.elapsed().as_millis() as u64;
        if elapsed_ms >= end_ms {
            break;
        }

        let target_rps_limit = crate::server::load_wave::local_rps_limit(&load, elapsed_ms);
        let tick = clock.plan_tick(elapsed_ms, target_rps_limit);

        if tick.scheduler_lag_ms > 0 || tick.missed_due_to_scheduler_lag > 0 {
            scheduler_lagged_total_state.fetch_add(
                tick.missed_due_to_scheduler_lag,
                std::sync::atomic::Ordering::SeqCst,
            );
            let _ = metric_tx.send(WaveSchedulerMetric::SchedulerLag {
                lag_ms: tick.scheduler_lag_ms,
                missed_starts: tick.missed_due_to_scheduler_lag,
            });
        }
        scheduled_total_state.store(
            tick.scheduled_total,
            std::sync::atomic::Ordering::SeqCst,
        );
        let _ = metric_tx.send(WaveSchedulerMetric::DispatchScheduled {
            count: tick.scheduled_starts,
        });

        let _ = try_send_slot_or_metric(
            &slot_tx,
            &metric_tx,
            WaveDispatchSlot {
                elapsed_ms: tick.elapsed_ms,
                planned_starts: tick.scheduled_starts,
                target_rps_limit,
                scheduled_total: tick.scheduled_total,
            },
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(tick_ms)).await;
    }
}
```

- [ ] **Step 5: Run dispatcher from orchestration task**

Use:

```rust
let dispatcher_token = token.child_token();
let dispatcher_metric_tx = metric_tx.clone();
run_wave_dispatcher(
    slot_rx,
    request_tx,
    &mut event_rx,
    Arc::clone(&pipeline),
    Arc::clone(&specs),
    Arc::clone(&env_groups),
    selected_env_group_slug.clone(),
    dispatcher_metric_tx,
    Arc::clone(&ready_to_send),
    started,
    tick_ms,
    dispatcher_token,
)
.await;
```

Keep the dispatcher in the orchestration task because it owns `event_rx` by mutable reference. The scheduler remains isolated because it runs in its own task and communicates only through `slot_tx`.

- [ ] **Step 6: Replace snapshot creation**

Where `run_wave_load` sends SSE metrics, read from `snapshot_rx`:

```rust
let mut snapshot = snapshot_rx.borrow().clone();
snapshot.duration_ms = None;
let _ = send_sse_or_cancel(
    &tx,
    "metrics",
    serde_json::to_value(snapshot).unwrap_or(Value::Null),
    &token,
);
```

Send `WaveMetricEvent::SnapshotWave(wave_snapshot(...))` before reading the snapshot:

```rust
let _ = metric_tx.send(WaveMetricEvent::SnapshotWave(wave_snapshot(
    &load,
    started.elapsed().as_millis() as u64,
    end_ms,
    tick_ms,
    response_in_flight.load(Ordering::SeqCst),
    scheduled_total_state.load(Ordering::SeqCst),
    scheduler_lagged_total_state.load(Ordering::SeqCst),
    ready_to_send.load(Ordering::SeqCst),
)));
```

- [ ] **Step 7: Verify runner tests**

Run:

```bash
cargo test -p previa-runner
```

Expected: PASS.

---

### Task 6: Add Integration Test for Response-Independent Scheduling

**Files:**
- Modify: `runner/src/server/wave_executor.rs`
- Test: `runner/src/server/wave_executor.rs`

- [ ] **Step 1: Add test-only scheduler harness**

Add this helper under `#[cfg(test)]`:

```rust
#[cfg(test)]
async fn collect_scheduler_slots_for_duration(
    target_rps: f64,
    tick_ms: u64,
    duration_ms: u64,
) -> usize {
    let mut clock = DispatchClock::new(tick_ms);
    let mut total = 0usize;
    let mut elapsed = 0u64;

    while elapsed < duration_ms {
        let tick = clock.plan_tick(elapsed, target_rps);
        total = total.saturating_add(tick.scheduled_starts);
        elapsed = elapsed.saturating_add(tick_ms);
    }

    total
}
```

- [ ] **Step 2: Add open-loop invariant test**

Add:

```rust
#[tokio::test]
async fn scheduler_slot_count_does_not_depend_on_response_completion() {
    let without_responses = collect_scheduler_slots_for_duration(1000.0, 100, 1000).await;
    let with_responses = collect_scheduler_slots_for_duration(1000.0, 100, 1000).await;

    assert_eq!(without_responses, 1000);
    assert_eq!(with_responses, 1000);
}
```

- [ ] **Step 3: Run the test**

Run:

```bash
cargo test -p previa-runner scheduler_slot_count_does_not_depend_on_response_completion
```

Expected: PASS. This locks the scheduling invariant independently from HTTP behavior.

---

### Task 7: End-to-End Verification and Diagnostics Check

**Files:**
- No production source changes after this task unless a verification failure identifies a root cause.

- [ ] **Step 1: Run targeted Rust tests**

Run:

```bash
cargo test -p previa-runner wave_scheduler wave_metrics_actor wave_sender wave_executor load_dispatch
```

Expected: PASS.

- [ ] **Step 2: Run full runner tests**

Run:

```bash
cargo test -p previa-runner
```

Expected: PASS.

- [ ] **Step 3: Run main aggregation tests**

Run:

```bash
cargo test -p previa-main
```

Expected: PASS.

- [ ] **Step 4: Run frontend tests that consume load metrics**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel LoadTestConfigPanel LoadTestTab
```

Expected: PASS.

- [ ] **Step 5: Build frontend**

Run:

```bash
npm --prefix app run build
```

Expected: PASS. Existing Vite chunk-size warnings are acceptable if no new errors appear.

- [ ] **Step 6: Build release**

Run:

```bash
cargo build --release
```

Expected: PASS.

- [ ] **Step 7: Restart local main and runners**

Run:

```bash
for port in 5610 5611 5612 5613; do
  pids="$(lsof -tiTCP:$port -sTCP:LISTEN || true)"
  if [ -n "$pids" ]; then
    kill $pids
  fi
done

screen -dmS previa-runner-5611 zsh -lc 'cd /Users/assis/projects/previa && ADDRESS=127.0.0.1 PORT=5611 RUST_LOG=info LOG_FORMAT=json ./target/release/previa-runner >> /tmp/previa-runner-5611.log 2>&1'
screen -dmS previa-runner-5612 zsh -lc 'cd /Users/assis/projects/previa && ADDRESS=127.0.0.1 PORT=5612 RUST_LOG=info LOG_FORMAT=json ./target/release/previa-runner >> /tmp/previa-runner-5612.log 2>&1'
screen -dmS previa-runner-5613 zsh -lc 'cd /Users/assis/projects/previa && ADDRESS=127.0.0.1 PORT=5613 RUST_LOG=info LOG_FORMAT=json ./target/release/previa-runner >> /tmp/previa-runner-5613.log 2>&1'
screen -dmS previa-main-5610 zsh -lc 'cd /Users/assis/projects/previa && ADDRESS=127.0.0.1 PORT=5610 ORCHESTRATOR_DATABASE_URL=sqlite:///tmp/previa-verify-5610.db RUNNER_ENDPOINTS=http://127.0.0.1:5611,http://127.0.0.1:5612,http://127.0.0.1:5613 PREVIA_APP_ENABLED=true RUST_LOG=info LOG_FORMAT=json ./target/release/previa-main >> /tmp/previa-main-5610.log 2>&1'
```

Expected:

```bash
curl -sS http://127.0.0.1:5610/info | jq '{activeRunners,totalRunners}'
```

prints:

```json
{
  "activeRunners": 3,
  "totalRunners": 3
}
```

- [ ] **Step 8: Run the same wave load test and inspect metrics**

Use the UI at:

```text
http://127.0.0.1:5610/projects/019de1a7-4dfd-7662-8b53-a305e5714ca5/pipeline/019de1a7-4dfd-7662-8b53-a317b9bdbe23/load-test
```

Then fetch the latest load history:

```bash
curl -sS 'http://127.0.0.1:5610/api/v1/projects/019de1a7-4dfd-7662-8b53-a305e5714ca5/tests/load?pipelineIndex=0&limit=1' \
  | jq '.[0] | {executionId,status,finalConsolidated:{dispatchStarted:.finalConsolidated.dispatchStarted,schedulerLagMs:.finalConsolidated.schedulerLagMs,schedulerLaggedStarts:.finalConsolidated.schedulerLaggedStarts,curveAdherence:.finalConsolidated.curveAdherence,totalSent:.finalConsolidated.totalSent,totalError:.finalConsolidated.totalError}}'
```

Expected:

- `dispatchStarted` is present.
- `schedulerLagMs` and `schedulerLaggedStarts` are materially lower than the previous high-load run for the same wave, unless CPU/network/OS scheduling is saturated.
- If the target endpoint fails with `502`, `503`, or send errors, `dispatchStarted` should continue following the target until local runner infrastructure becomes the bottleneck.

---

## Self-Review

- Spec coverage: The plan isolates the scheduler with channels, moves response work outside the scheduler path, moves metrics away from scheduler locks, preserves dispatch-start RPS diagnostics, and includes validation against the same local three-runner setup.
- Placeholder scan: The plan contains concrete files, functions, commands, and expected results.
- Type consistency: `WaveDispatchSlot`, `WaveSchedulerMetric`, and `WaveMetricEvent` are introduced before they are used by sender, executor, and metrics actor tasks.
