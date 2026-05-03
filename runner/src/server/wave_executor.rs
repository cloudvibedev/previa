use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::error;

use previa_runner::{
    Pipeline, PipelineStep, RuntimeEnvGroup, RuntimeSpec, StepExecutionResult, prepare_http_step,
};

use crate::server::load_dispatch::DispatchClock;
use crate::server::load_wave::{
    calculate_dispatch_tick_ms, local_rps_limit, sample_intensity, timeline_end_ms,
};
use crate::server::metrics::{MetricsAccumulator, WaveMetricsSnapshot};
use crate::server::models::LoadProfile;
use crate::server::runtime::RuntimeSampler;
use crate::server::sse::{SseMessage, send_sse_or_cancel};
use crate::server::wave_emitter::{StartLagClass, classify_start_lag};
use crate::server::wave_sender::{ReadyWaveRequest, WaveObserverEvent, WaveSender};

#[derive(Debug)]
struct PipelineCursor {
    step_index: usize,
    attempt: usize,
    context: HashMap<String, StepExecutionResult>,
    pipeline_started_at: Instant,
}

impl PipelineCursor {
    fn new(started_at: Instant) -> Self {
        Self {
            step_index: 0,
            attempt: 1,
            context: HashMap::new(),
            pipeline_started_at: started_at,
        }
    }
}

type ObserverEvent = WaveObserverEvent<PipelineCursor>;
const OBSERVER_EVENTS_PER_TICK_BUDGET: usize = 1024;

fn next_cursor_for_slot(
    ready: &mut VecDeque<PipelineCursor>,
    create: impl FnOnce() -> PipelineCursor,
) -> PipelineCursor {
    ready.pop_front().unwrap_or_else(create)
}

pub async fn run_wave_load(
    load: LoadProfile,
    pipeline: Pipeline,
    _selected_key: Option<String>,
    selected_env_group_slug: Option<String>,
    specs: Vec<RuntimeSpec>,
    env_groups: Vec<RuntimeEnvGroup>,
    tx: mpsc::UnboundedSender<SseMessage>,
    token: tokio_util::sync::CancellationToken,
) {
    let tick_ms = calculate_dispatch_tick_ms(&load);
    let started = Instant::now();
    let end_ms = timeline_end_ms(&load);
    let pipeline = Arc::new(pipeline);
    let specs = Arc::new(specs);
    let env_groups = Arc::new(env_groups);
    let metrics = Arc::new(tokio::sync::Mutex::new(MetricsAccumulator::new()));
    let runtime_sampler = Arc::new(tokio::sync::Mutex::new(RuntimeSampler::new()));
    let response_in_flight = Arc::new(AtomicUsize::new(0));
    let ready_to_send = Arc::new(AtomicUsize::new(0));
    let missed_starts = Arc::new(AtomicUsize::new(0));
    let observer_token = token.child_token();
    let http_client = Arc::new(Client::new());
    let mut dispatch_clock = DispatchClock::new(tick_ms);
    let mut ready = VecDeque::new();
    let (request_tx, request_rx) = mpsc::unbounded_channel::<ReadyWaveRequest<PipelineCursor>>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<ObserverEvent>();
    let sender = WaveSender::new(
        Arc::clone(&http_client),
        Arc::clone(&metrics),
        Arc::clone(&response_in_flight),
        Arc::clone(&ready_to_send),
        request_rx,
        event_tx,
        observer_token.clone(),
    );
    let sender_task = tokio::spawn(sender.run());
    let mut scheduled_total = 0usize;

    loop {
        if token.is_cancelled() {
            break;
        }

        let elapsed_ms = started.elapsed().as_millis() as u64;
        if elapsed_ms >= end_ms {
            break;
        }

        let target_rps_limit = local_rps_limit(&load, elapsed_ms);
        let tick = dispatch_clock.plan_tick(elapsed_ms, target_rps_limit);
        scheduled_total = tick.scheduled_total;
        if tick.scheduler_lag_ms > 0 || tick.missed_due_to_scheduler_lag > 0 {
            missed_starts.fetch_add(tick.missed_due_to_scheduler_lag, Ordering::SeqCst);
            let mut lock = metrics.lock().await;
            lock.record_scheduler_lag_ms(tick.scheduler_lag_ms);
            lock.record_scheduler_lagged_starts_count(tick.missed_due_to_scheduler_lag);
        }
        {
            let mut lock = metrics.lock().await;
            lock.record_dispatch_submitted_count(tick.scheduled_starts);
        }

        for _ in 0..tick.scheduled_starts {
            if token.is_cancelled() {
                break;
            }

            let was_ready_empty = ready.is_empty();
            let cursor = next_cursor_for_slot(&mut ready, || PipelineCursor::new(Instant::now()));
            if was_ready_empty && cursor.step_index == 0 && cursor.context.is_empty() {
                let mut lock = metrics.lock().await;
                lock.record_start();
            }

            let Some(step) = pipeline.steps.get(cursor.step_index).cloned() else {
                record_terminal_pipeline(&metrics, cursor, false, None).await;
                continue;
            };
            let max_attempts = max_attempts_for_step(&step);
            let prepared = match prepare_http_step(
                &step,
                &cursor.context,
                Some(specs.as_slice()),
                Some(env_groups.as_slice()),
                selected_env_group_slug.as_deref(),
                cursor.attempt,
                max_attempts,
            ) {
                Ok(prepared) => prepared,
                Err(result) => {
                    handle_prepare_error(
                        result,
                        cursor,
                        &mut ready,
                        &pipeline,
                        &metrics,
                        &missed_starts,
                    )
                    .await;
                    continue;
                }
            };

            let actual_elapsed_ms = started.elapsed().as_millis() as u64;
            if classify_start_lag(tick.elapsed_ms, actual_elapsed_ms, tick_ms)
                == StartLagClass::RuntimeLagged
            {
                missed_starts.fetch_add(1, Ordering::SeqCst);
                let mut lock = metrics.lock().await;
                lock.record_runtime_lagged_start();
            }

            ready_to_send.fetch_add(1, Ordering::SeqCst);
            if request_tx
                .send(ReadyWaveRequest {
                    step,
                    context: cursor.context.clone(),
                    cursor,
                    prepared,
                    specs: Arc::clone(&specs),
                    env_groups: Arc::clone(&env_groups),
                    selected_env_group_slug: selected_env_group_slug.clone(),
                })
                .is_err()
            {
                ready_to_send.fetch_sub(1, Ordering::SeqCst);
                error!("wave sender stopped before accepting prepared request");
                break;
            }
        }

        drain_observer_events_budgeted(
            &mut event_rx,
            &mut ready,
            &pipeline,
            &metrics,
            OBSERVER_EVENTS_PER_TICK_BUDGET,
        )
        .await;

        send_metrics_snapshot(SnapshotArgs {
            load: &load,
            started,
            end_ms,
            tick_ms,
            scheduled_total,
            missed_total: missed_starts.load(Ordering::SeqCst),
            ready_requests: ready
                .len()
                .saturating_add(ready_to_send.load(Ordering::SeqCst)),
            response_in_flight: response_in_flight.load(Ordering::SeqCst),
            metrics: &metrics,
            runtime_sampler: &runtime_sampler,
            tx: &tx,
            token: &token,
            event: "metrics",
            duration_ms: None,
        })
        .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(tick_ms)).await;
    }

    let grace_deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_millis(load.grace_period_ms);
    while response_in_flight.load(Ordering::SeqCst) > 0 || ready_to_send.load(Ordering::SeqCst) > 0
    {
        drain_all_observer_events(&mut event_rx, &mut ready, &pipeline, &metrics).await;
        if token.is_cancelled() || tokio::time::Instant::now() >= grace_deadline {
            break;
        }

        send_metrics_snapshot(SnapshotArgs {
            load: &load,
            started,
            end_ms,
            tick_ms,
            scheduled_total,
            missed_total: missed_starts.load(Ordering::SeqCst),
            ready_requests: ready
                .len()
                .saturating_add(ready_to_send.load(Ordering::SeqCst)),
            response_in_flight: response_in_flight.load(Ordering::SeqCst),
            metrics: &metrics,
            runtime_sampler: &runtime_sampler,
            tx: &tx,
            token: &token,
            event: "metrics",
            duration_ms: None,
        })
        .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(tick_ms.min(250))).await;
    }

    drop(request_tx);
    if response_in_flight.load(Ordering::SeqCst) > 0 {
        observer_token.cancel();
        let observer_shutdown_deadline =
            tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);
        while response_in_flight.load(Ordering::SeqCst) > 0
            && tokio::time::Instant::now() < observer_shutdown_deadline
        {
            tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
        }
    }
    if let Err(err) = sender_task.await {
        if !err.is_cancelled() {
            error!("wave sender task failed: {err}");
        }
    }
    drain_all_observer_events(&mut event_rx, &mut ready, &pipeline, &metrics).await;

    if send_metrics_snapshot(SnapshotArgs {
        load: &load,
        started,
        end_ms,
        tick_ms,
        scheduled_total,
        missed_total: missed_starts.load(Ordering::SeqCst),
        ready_requests: ready
            .len()
            .saturating_add(ready_to_send.load(Ordering::SeqCst)),
        response_in_flight: response_in_flight.load(Ordering::SeqCst),
        metrics: &metrics,
        runtime_sampler: &runtime_sampler,
        tx: &tx,
        token: &token,
        event: "metrics",
        duration_ms: None,
    })
    .await
        && !token.is_cancelled()
    {
        let complete = build_snapshot(SnapshotBuildArgs {
            load: &load,
            started,
            end_ms,
            tick_ms,
            scheduled_total,
            missed_total: missed_starts.load(Ordering::SeqCst),
            ready_requests: ready
                .len()
                .saturating_add(ready_to_send.load(Ordering::SeqCst)),
            response_in_flight: response_in_flight.load(Ordering::SeqCst),
            metrics: &metrics,
            runtime_sampler: &runtime_sampler,
            duration_ms: None,
        })
        .await;
        let _ = send_sse_or_cancel(
            &tx,
            "complete",
            serde_json::to_value(complete).unwrap_or(Value::Null),
            &token,
        );
    }
}

async fn drain_observer_events_budgeted(
    event_rx: &mut mpsc::UnboundedReceiver<ObserverEvent>,
    ready: &mut VecDeque<PipelineCursor>,
    pipeline: &Pipeline,
    metrics: &Arc<tokio::sync::Mutex<MetricsAccumulator>>,
    budget: usize,
) -> usize {
    let mut drained = 0usize;
    while drained < budget {
        let Ok(event) = event_rx.try_recv() else {
            break;
        };
        handle_step_result(event.result, event.cursor, ready, pipeline, metrics).await;
        drained += 1;
    }
    drained
}

async fn drain_all_observer_events(
    event_rx: &mut mpsc::UnboundedReceiver<ObserverEvent>,
    ready: &mut VecDeque<PipelineCursor>,
    pipeline: &Pipeline,
    metrics: &Arc<tokio::sync::Mutex<MetricsAccumulator>>,
) {
    while drain_observer_events_budgeted(
        event_rx,
        ready,
        pipeline,
        metrics,
        OBSERVER_EVENTS_PER_TICK_BUDGET,
    )
    .await
        > 0
    {}
}

async fn handle_step_result(
    result: StepExecutionResult,
    mut cursor: PipelineCursor,
    ready: &mut VecDeque<PipelineCursor>,
    pipeline: &Pipeline,
    metrics: &Arc<tokio::sync::Mutex<MetricsAccumulator>>,
) {
    let Some(step) = pipeline.steps.get(cursor.step_index) else {
        record_terminal_pipeline(metrics, cursor, false, Some(&result)).await;
        return;
    };
    let max_attempts = max_attempts_for_step(step);

    if result.status == "error" && cursor.attempt < max_attempts {
        cursor.attempt += 1;
        ready.push_back(cursor);
        return;
    }

    if result.status == "error" {
        record_terminal_pipeline(metrics, cursor, false, Some(&result)).await;
        return;
    }

    cursor.context.insert(result.step_id.clone(), result);
    cursor.step_index += 1;
    cursor.attempt = 1;

    if cursor.step_index >= pipeline.steps.len() {
        record_terminal_pipeline(metrics, cursor, true, None).await;
    } else {
        ready.push_back(cursor);
    }
}

async fn handle_prepare_error(
    result: StepExecutionResult,
    mut cursor: PipelineCursor,
    ready: &mut VecDeque<PipelineCursor>,
    pipeline: &Pipeline,
    metrics: &Arc<tokio::sync::Mutex<MetricsAccumulator>>,
    missed_starts: &Arc<AtomicUsize>,
) {
    let max_attempts = pipeline
        .steps
        .get(cursor.step_index)
        .map(max_attempts_for_step)
        .unwrap_or(1);

    if cursor.attempt < max_attempts {
        cursor.attempt += 1;
        ready.push_back(cursor);
        return;
    }

    if cursor.step_index > 0 {
        missed_starts.fetch_add(1, Ordering::SeqCst);
        let mut lock = metrics.lock().await;
        lock.record_dependency_limited_starts_count(1);
    }
    record_terminal_pipeline(metrics, cursor, false, Some(&result)).await;
}

async fn record_terminal_pipeline(
    metrics: &Arc<tokio::sync::Mutex<MetricsAccumulator>>,
    cursor: PipelineCursor,
    success: bool,
    result: Option<&StepExecutionResult>,
) {
    let duration_ms = cursor.pipeline_started_at.elapsed().as_millis() as f64;
    let mut lock = metrics.lock().await;
    lock.update(duration_ms, success);
    if !success {
        if let Some(result) = result {
            let http_status = result.response.as_ref().map(|response| response.status);
            let error = result.error.as_deref().unwrap_or("pipeline failed");
            lock.record_error_sample(&result.step_id, http_status, error);
        }
    }
}

fn max_attempts_for_step(step: &PipelineStep) -> usize {
    step.retry.unwrap_or(0).saturating_add(1)
}

struct SnapshotArgs<'a> {
    load: &'a LoadProfile,
    started: Instant,
    end_ms: u64,
    tick_ms: u64,
    scheduled_total: usize,
    missed_total: usize,
    ready_requests: usize,
    response_in_flight: usize,
    metrics: &'a Arc<tokio::sync::Mutex<MetricsAccumulator>>,
    runtime_sampler: &'a Arc<tokio::sync::Mutex<RuntimeSampler>>,
    tx: &'a mpsc::UnboundedSender<SseMessage>,
    token: &'a tokio_util::sync::CancellationToken,
    event: &'static str,
    duration_ms: Option<u64>,
}

async fn send_metrics_snapshot(args: SnapshotArgs<'_>) -> bool {
    let snapshot = build_snapshot(SnapshotBuildArgs {
        load: args.load,
        started: args.started,
        end_ms: args.end_ms,
        tick_ms: args.tick_ms,
        scheduled_total: args.scheduled_total,
        missed_total: args.missed_total,
        ready_requests: args.ready_requests,
        response_in_flight: args.response_in_flight,
        metrics: args.metrics,
        runtime_sampler: args.runtime_sampler,
        duration_ms: args.duration_ms,
    })
    .await;
    send_sse_or_cancel(
        args.tx,
        args.event,
        serde_json::to_value(snapshot).unwrap_or(Value::Null),
        args.token,
    )
}

struct SnapshotBuildArgs<'a> {
    load: &'a LoadProfile,
    started: Instant,
    end_ms: u64,
    tick_ms: u64,
    scheduled_total: usize,
    missed_total: usize,
    ready_requests: usize,
    response_in_flight: usize,
    metrics: &'a Arc<tokio::sync::Mutex<MetricsAccumulator>>,
    runtime_sampler: &'a Arc<tokio::sync::Mutex<RuntimeSampler>>,
    duration_ms: Option<u64>,
}

async fn build_snapshot(args: SnapshotBuildArgs<'_>) -> crate::server::models::LoadTestMetrics {
    let runtime = {
        let mut sampler = args.runtime_sampler.lock().await;
        sampler.snapshot()
    };
    let elapsed_ms = args.started.elapsed().as_millis() as u64;
    let lock = args.metrics.lock().await;
    lock.snapshot_with_wave(
        args.duration_ms,
        runtime,
        Some(wave_snapshot(
            args.load,
            elapsed_ms,
            args.end_ms,
            args.tick_ms,
            args.response_in_flight,
            args.scheduled_total,
            args.missed_total,
            args.ready_requests,
        )),
    )
}

fn wave_snapshot(
    load: &LoadProfile,
    elapsed_ms: u64,
    end_ms: u64,
    tick_ms: u64,
    response_in_flight: usize,
    scheduled_starts: usize,
    missed_starts: usize,
    ready_requests: usize,
) -> WaveMetricsSnapshot {
    let load_phase_active = elapsed_ms <= end_ms;
    let target_intensity = if load_phase_active {
        sample_intensity(load, elapsed_ms)
    } else {
        0.0
    };
    let target_rps_limit = if load_phase_active {
        local_rps_limit(load, elapsed_ms)
    } else {
        0.0
    };

    WaveMetricsSnapshot {
        target_intensity,
        target_rps_limit,
        in_flight: response_in_flight,
        runner_max_rps: load.runner_max_rps,
        tick_ms,
        scheduled_starts,
        missed_starts,
        ready_requests,
        active_pipelines: response_in_flight.saturating_add(ready_requests),
        outstanding_requests: response_in_flight,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::time::Instant;

    #[test]
    fn next_cursor_prefers_ready_continuations_before_starting_new_pipeline() {
        let mut ready = VecDeque::new();
        ready.push_back(PipelineCursor {
            step_index: 2,
            attempt: 1,
            context: HashMap::new(),
            pipeline_started_at: Instant::now(),
        });
        let mut started_new = false;

        let cursor = next_cursor_for_slot(&mut ready, || {
            started_new = true;
            PipelineCursor {
                step_index: 0,
                attempt: 1,
                context: HashMap::new(),
                pipeline_started_at: Instant::now(),
            }
        });

        assert_eq!(cursor.step_index, 2);
        assert!(!started_new);
    }

    #[test]
    fn next_cursor_starts_new_pipeline_when_no_continuation_is_ready() {
        let mut ready = VecDeque::new();
        let cursor = next_cursor_for_slot(&mut ready, || PipelineCursor::new(Instant::now()));

        assert_eq!(cursor.step_index, 0);
        assert_eq!(cursor.attempt, 1);
        assert!(cursor.context.is_empty());
    }

    #[tokio::test]
    async fn observer_drain_respects_per_tick_budget() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ObserverEvent>();
        let mut ready = VecDeque::new();
        let pipeline = Pipeline {
            id: Some("p".to_owned()),
            name: "pipeline".to_owned(),
            description: None,
            steps: Vec::new(),
        };
        let metrics = Arc::new(tokio::sync::Mutex::new(MetricsAccumulator::new()));

        for _ in 0..3 {
            tx.send(WaveObserverEvent {
                cursor: PipelineCursor::new(Instant::now()),
                result: StepExecutionResult {
                    step_id: "missing".to_owned(),
                    status: "error".to_owned(),
                    request: None,
                    response: None,
                    error: Some("synthetic".to_owned()),
                    duration: Some(0),
                    attempts: None,
                    attempt: Some(1),
                    max_attempts: Some(1),
                    assert_results: None,
                },
            })
            .unwrap();
        }

        let drained =
            drain_observer_events_budgeted(&mut rx, &mut ready, &pipeline, &metrics, 2).await;

        assert_eq!(drained, 2);
        assert!(
            rx.try_recv().is_ok(),
            "one event should remain for a later tick"
        );
    }
}
