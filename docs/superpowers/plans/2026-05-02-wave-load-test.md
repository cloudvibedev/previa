# Wave Load Test Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make percentage-based Wave Load Test the canonical Previa load-test model, with runner-local wave sampling and flow limiting.

**Architecture:** `previa-main` accepts project load requests, selects runners, injects each runner's safe `runnerMaxRps`, forwards the full `load` wave config, aggregates SSE, and records history. `previa-runner` owns wave validation, interpolation, automatic tick selection, local token/leaky-bucket flow control, `maxInFlight`, timeline completion, and wave metrics. The existing classic `config` shape remains accepted during migration and is converted or forwarded through a compatibility path.

**Tech Stack:** Rust, Axum, Tokio, SQLx, serde, utoipa for `previa-main` and `previa-runner`; React, TypeScript, Zustand, Vitest for the app.

---

## File Map

- `runner/src/server/models.rs`: add `LoadProfile`, `LoadPoint`, `LoadInterpolation`, optional `load` on `LoadTestRequest`, and wave metric fields.
- `runner/src/server/load_wave.rs`: new focused module for validation, tick calculation, and interpolation.
- `runner/src/server/load_bucket.rs`: new focused module for token/leaky-bucket admission.
- `runner/src/server/handlers/load.rs`: execute wave load profiles with bucket + `maxInFlight`, while preserving legacy classic execution behavior until migration is complete.
- `runner/src/server/mod.rs`: register the new modules.
- `runner/src/server/docs.rs`: expose new schemas in runner OpenAPI.
- `main/src/server/models.rs`: mirror the load profile contract for project and internal load requests.
- `main/src/server/execution/load.rs`: resolve the canonical load config, calculate runner `load` payloads, save requested config/history, and forward wave payloads.
- `main/src/server/execution/load_batch.rs`: parse and aggregate new wave metrics from runners.
- `main/src/server/utils.rs`: parse new optional metric fields.
- `main/src/server/mcp/models.rs`: accept wave load payloads through MCP.
- `main/src/server/mcp/service.rs`: update tool schema and load-test designer prompt.
- `main/src/server/docs.rs`: expose new schemas in main OpenAPI.
- `app/src/types/load-test.ts`: add wave load config and wave metrics types.
- `app/src/lib/remote-executor.ts`: send wave load payloads.
- `app/src/lib/api-client.ts`: send wave load payloads for project executions.
- `app/src/components/LoadTestConfigPanel.tsx`: replace classic sliders with wave controls and presets.
- `app/src/stores/useLoadTestHistoryStore.ts`: store pending wave config and render wave metrics.
- `app/src/components/LoadTestResultsPanel.tsx`: display target intensity, target RPS, actual RPS, and in-flight count.
- `docs/previa/api-workflows.md`: document wave load API example.
- `docs/previa/examples-cookbook.md`: document wave presets as editable points.
- `docs/previa/glossary.md`: update the load-test definition.
- `app/docs/test-execution-api.yaml`: update API docs snapshot when the OpenAPI generation command produces a diff.

## Task 1: Runner Wave Contract And Validation

**Files:**
- Modify: `runner/src/server/models.rs`
- Create: `runner/src/server/load_wave.rs`
- Modify: `runner/src/server/mod.rs`

- [ ] **Step 1: Write failing validation tests**

Create `runner/src/server/load_wave.rs` with only this test module first:

```rust
#[cfg(test)]
mod tests {
    use crate::server::models::{LoadInterpolation, LoadPoint, LoadProfile};

    #[test]
    fn accepts_valid_wave_profile() {
        let profile = LoadProfile {
            points: vec![
                LoadPoint { at_ms: 0, intensity: 10.0 },
                LoadPoint { at_ms: 60_000, intensity: 80.0 },
            ],
            interpolation: LoadInterpolation::Smooth,
            runner_max_rps: 1000.0,
            max_in_flight: 200,
            grace_period_ms: 30_000,
        };

        assert!(super::validate_load_profile(&profile).is_ok());
    }

    #[test]
    fn rejects_wave_without_zero_start() {
        let profile = LoadProfile {
            points: vec![
                LoadPoint { at_ms: 100, intensity: 10.0 },
                LoadPoint { at_ms: 60_000, intensity: 80.0 },
            ],
            interpolation: LoadInterpolation::Smooth,
            runner_max_rps: 1000.0,
            max_in_flight: 200,
            grace_period_ms: 30_000,
        };

        assert_eq!(
            super::validate_load_profile(&profile).unwrap_err(),
            "load.points[0].atMs must be 0"
        );
    }

    #[test]
    fn rejects_non_increasing_points() {
        let profile = LoadProfile {
            points: vec![
                LoadPoint { at_ms: 0, intensity: 10.0 },
                LoadPoint { at_ms: 0, intensity: 80.0 },
            ],
            interpolation: LoadInterpolation::Smooth,
            runner_max_rps: 1000.0,
            max_in_flight: 200,
            grace_period_ms: 30_000,
        };

        assert_eq!(
            super::validate_load_profile(&profile).unwrap_err(),
            "load.points must be strictly increasing by atMs"
        );
    }

    #[test]
    fn rejects_out_of_range_intensity() {
        let profile = LoadProfile {
            points: vec![
                LoadPoint { at_ms: 0, intensity: 10.0 },
                LoadPoint { at_ms: 60_000, intensity: 120.0 },
            ],
            interpolation: LoadInterpolation::Smooth,
            runner_max_rps: 1000.0,
            max_in_flight: 200,
            grace_period_ms: 30_000,
        };

        assert_eq!(
            super::validate_load_profile(&profile).unwrap_err(),
            "load.points intensity must be between 0 and 100"
        );
    }
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p previa-runner load_wave::tests::accepts_valid_wave_profile
```

Expected: compile failure because `load_wave` is not registered and the wave model types do not exist.

- [ ] **Step 3: Add wave model types**

In `runner/src/server/models.rs`, add these definitions near `LoadTestConfig`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadProfile {
    pub points: Vec<LoadPoint>,
    #[serde(default)]
    pub interpolation: LoadInterpolation,
    pub runner_max_rps: f64,
    pub max_in_flight: usize,
    #[serde(default = "default_load_grace_period_ms")]
    pub grace_period_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadPoint {
    pub at_ms: u64,
    pub intensity: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoadInterpolation {
    Smooth,
    Linear,
    Step,
}

impl Default for LoadInterpolation {
    fn default() -> Self {
        Self::Smooth
    }
}

fn default_load_grace_period_ms() -> u64 {
    30_000
}
```

Update `LoadTestRequest` in the same file:

```rust
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoadTestRequest {
    pub pipeline: Pipeline,
    #[serde(default)]
    pub config: Option<LoadTestConfig>,
    #[serde(default)]
    pub load: Option<LoadProfile>,
    pub selected_base_url_key: Option<String>,
    pub selected_env_group_slug: Option<String>,
    #[serde(default)]
    pub specs: Vec<RuntimeSpec>,
    #[serde(default)]
    pub env_groups: Vec<RuntimeEnvGroup>,
}
```

- [ ] **Step 4: Implement validation and register module**

Add to `runner/src/server/mod.rs`:

```rust
mod load_bucket;
mod load_wave;
```

Replace the contents of `runner/src/server/load_wave.rs` above the tests with:

```rust
use crate::server::models::LoadProfile;

pub fn validate_load_profile(profile: &LoadProfile) -> Result<(), String> {
    if profile.points.len() < 2 {
        return Err("load.points must contain at least two points".to_owned());
    }
    if profile.points[0].at_ms != 0 {
        return Err("load.points[0].atMs must be 0".to_owned());
    }
    if profile.runner_max_rps <= 0.0 {
        return Err("load.runnerMaxRps must be positive".to_owned());
    }
    if profile.max_in_flight == 0 {
        return Err("load.maxInFlight must be positive".to_owned());
    }

    for point in &profile.points {
        if !(0.0..=100.0).contains(&point.intensity) {
            return Err("load.points intensity must be between 0 and 100".to_owned());
        }
    }

    for pair in profile.points.windows(2) {
        if pair[1].at_ms <= pair[0].at_ms {
            return Err("load.points must be strictly increasing by atMs".to_owned());
        }
    }

    Ok(())
}
```

- [ ] **Step 5: Run validation tests**

Run:

```bash
cargo test -p previa-runner load_wave::tests -- --nocapture
```

Expected: all validation tests pass.

- [ ] **Step 6: Commit**

```bash
git add runner/src/server/models.rs runner/src/server/load_wave.rs runner/src/server/mod.rs
git commit -m "feat(runner): add wave load contract"
```

## Task 2: Runner Wave Math

**Files:**
- Modify: `runner/src/server/load_wave.rs`

- [ ] **Step 1: Add failing interpolation and tick tests**

Append these tests inside `runner/src/server/load_wave.rs` test module:

```rust
#[test]
fn calculates_dynamic_tick_with_minimum_and_maximum() {
    let long_profile = LoadProfile {
        points: vec![
            LoadPoint { at_ms: 0, intensity: 10.0 },
            LoadPoint { at_ms: 60_000, intensity: 80.0 },
        ],
        interpolation: LoadInterpolation::Smooth,
        runner_max_rps: 1000.0,
        max_in_flight: 200,
        grace_period_ms: 30_000,
    };
    assert_eq!(super::calculate_tick_ms(&long_profile), 1000);

    let short_profile = LoadProfile {
        points: vec![
            LoadPoint { at_ms: 0, intensity: 10.0 },
            LoadPoint { at_ms: 500, intensity: 80.0 },
        ],
        interpolation: LoadInterpolation::Smooth,
        runner_max_rps: 1000.0,
        max_in_flight: 200,
        grace_period_ms: 30_000,
    };
    assert_eq!(super::calculate_tick_ms(&short_profile), 100);
}

#[test]
fn interpolates_linear_values() {
    let profile = LoadProfile {
        points: vec![
            LoadPoint { at_ms: 0, intensity: 10.0 },
            LoadPoint { at_ms: 1000, intensity: 90.0 },
        ],
        interpolation: LoadInterpolation::Linear,
        runner_max_rps: 1000.0,
        max_in_flight: 200,
        grace_period_ms: 30_000,
    };
    assert_eq!(super::sample_intensity(&profile, 500), 50.0);
}

#[test]
fn interpolates_smoothstep_values() {
    let profile = LoadProfile {
        points: vec![
            LoadPoint { at_ms: 0, intensity: 0.0 },
            LoadPoint { at_ms: 1000, intensity: 100.0 },
        ],
        interpolation: LoadInterpolation::Smooth,
        runner_max_rps: 1000.0,
        max_in_flight: 200,
        grace_period_ms: 30_000,
    };
    assert!((super::sample_intensity(&profile, 250) - 15.625).abs() < 0.001);
    assert_eq!(super::sample_intensity(&profile, 500), 50.0);
}

#[test]
fn interpolates_step_values() {
    let profile = LoadProfile {
        points: vec![
            LoadPoint { at_ms: 0, intensity: 10.0 },
            LoadPoint { at_ms: 1000, intensity: 90.0 },
        ],
        interpolation: LoadInterpolation::Step,
        runner_max_rps: 1000.0,
        max_in_flight: 200,
        grace_period_ms: 30_000,
    };
    assert_eq!(super::sample_intensity(&profile, 999), 10.0);
    assert_eq!(super::sample_intensity(&profile, 1000), 90.0);
}
```

- [ ] **Step 2: Run the failing math tests**

Run:

```bash
cargo test -p previa-runner load_wave::tests::calculates_dynamic_tick_with_minimum_and_maximum
```

Expected: compile failure for missing `calculate_tick_ms`.

- [ ] **Step 3: Implement tick and interpolation**

Add this to `runner/src/server/load_wave.rs`:

```rust
use crate::server::models::{LoadInterpolation, LoadPoint};

pub fn calculate_tick_ms(profile: &LoadProfile) -> u64 {
    let min_interval = profile
        .points
        .windows(2)
        .map(|pair| pair[1].at_ms.saturating_sub(pair[0].at_ms))
        .filter(|interval| *interval > 0)
        .min()
        .unwrap_or(10_000);

    (min_interval / 10).clamp(100, 1000)
}

pub fn sample_intensity(profile: &LoadProfile, elapsed_ms: u64) -> f64 {
    let last = profile
        .points
        .last()
        .expect("validated profile must contain points");
    if elapsed_ms >= last.at_ms {
        return last.intensity;
    }

    let (start, end) = find_segment(&profile.points, elapsed_ms);
    if elapsed_ms >= end.at_ms {
        return end.intensity;
    }

    match profile.interpolation {
        LoadInterpolation::Step => start.intensity,
        LoadInterpolation::Linear => interpolate_linear(start, end, elapsed_ms),
        LoadInterpolation::Smooth => {
            let raw_t = segment_t(start, end, elapsed_ms);
            let smooth_t = raw_t * raw_t * (3.0 - 2.0 * raw_t);
            start.intensity + (end.intensity - start.intensity) * smooth_t
        }
    }
}

pub fn local_rps_limit(profile: &LoadProfile, elapsed_ms: u64) -> f64 {
    profile.runner_max_rps * sample_intensity(profile, elapsed_ms) / 100.0
}

pub fn timeline_end_ms(profile: &LoadProfile) -> u64 {
    profile
        .points
        .last()
        .map(|point| point.at_ms)
        .unwrap_or_default()
}

fn find_segment(points: &[LoadPoint], elapsed_ms: u64) -> (&LoadPoint, &LoadPoint) {
    points
        .windows(2)
        .find(|pair| elapsed_ms >= pair[0].at_ms && elapsed_ms < pair[1].at_ms)
        .map(|pair| (&pair[0], &pair[1]))
        .unwrap_or_else(|| (&points[points.len() - 2], &points[points.len() - 1]))
}

fn interpolate_linear(start: &LoadPoint, end: &LoadPoint, elapsed_ms: u64) -> f64 {
    let t = segment_t(start, end, elapsed_ms);
    start.intensity + (end.intensity - start.intensity) * t
}

fn segment_t(start: &LoadPoint, end: &LoadPoint, elapsed_ms: u64) -> f64 {
    let span = end.at_ms.saturating_sub(start.at_ms).max(1) as f64;
    let offset = elapsed_ms.saturating_sub(start.at_ms) as f64;
    (offset / span).clamp(0.0, 1.0)
}
```

- [ ] **Step 4: Run math tests**

Run:

```bash
cargo test -p previa-runner load_wave::tests -- --nocapture
```

Expected: all wave math and validation tests pass.

- [ ] **Step 5: Commit**

```bash
git add runner/src/server/load_wave.rs
git commit -m "feat(runner): sample wave load profiles"
```

## Task 3: Runner Flow Bucket

**Files:**
- Create: `runner/src/server/load_bucket.rs`

- [ ] **Step 1: Add failing bucket tests**

Create `runner/src/server/load_bucket.rs`:

```rust
#[derive(Debug, Clone)]
pub struct FlowBucket {
    capacity: f64,
    tokens: f64,
    last_refill_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::FlowBucket;

    #[test]
    fn admits_when_tokens_are_available() {
        let mut bucket = FlowBucket::new(10.0, 0);
        bucket.refill(10.0, 0);

        assert!(bucket.try_acquire());
        assert_eq!(bucket.available_tokens().floor(), 9.0);
    }

    #[test]
    fn blocks_when_empty_until_refilled() {
        let mut bucket = FlowBucket::new(1.0, 0);
        bucket.refill(1.0, 0);

        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire());

        bucket.refill(1.0, 1000);
        assert!(bucket.try_acquire());
    }

    #[test]
    fn updates_capacity_when_limit_changes() {
        let mut bucket = FlowBucket::new(100.0, 0);
        bucket.refill(100.0, 1000);
        assert!(bucket.available_tokens() <= 100.0);

        bucket.refill(10.0, 2000);
        assert!(bucket.available_tokens() <= 10.0);
    }
}
```

- [ ] **Step 2: Run failing bucket tests**

Run:

```bash
cargo test -p previa-runner load_bucket::tests
```

Expected: compile failure because `new`, `refill`, `try_acquire`, and `available_tokens` are missing.

- [ ] **Step 3: Implement bucket**

Replace the top of `runner/src/server/load_bucket.rs` with:

```rust
#[derive(Debug, Clone)]
pub struct FlowBucket {
    capacity: f64,
    tokens: f64,
    last_refill_ms: u64,
}

impl FlowBucket {
    pub fn new(initial_rps: f64, now_ms: u64) -> Self {
        let capacity = initial_rps.max(0.0);
        Self {
            capacity,
            tokens: capacity,
            last_refill_ms: now_ms,
        }
    }

    pub fn refill(&mut self, rps_limit: f64, now_ms: u64) {
        let next_capacity = rps_limit.max(0.0);
        let elapsed_ms = now_ms.saturating_sub(self.last_refill_ms) as f64;
        let earned = next_capacity * elapsed_ms / 1000.0;
        self.capacity = next_capacity;
        self.tokens = (self.tokens + earned).min(self.capacity);
        self.last_refill_ms = now_ms;
    }

    pub fn try_acquire(&mut self) -> bool {
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub fn available_tokens(&self) -> f64 {
        self.tokens
    }
}
```

- [ ] **Step 4: Run bucket tests**

Run:

```bash
cargo test -p previa-runner load_bucket::tests -- --nocapture
```

Expected: all bucket tests pass.

- [ ] **Step 5: Commit**

```bash
git add runner/src/server/load_bucket.rs
git commit -m "feat(runner): add load flow bucket"
```

## Task 4: Runner Wave Execution

**Files:**
- Modify: `runner/src/server/handlers/load.rs`
- Modify: `runner/src/server/models.rs`
- Modify: `runner/src/server/metrics.rs`

- [ ] **Step 1: Add wave metric fields**

Modify `LoadTestMetrics` in `runner/src/server/models.rs`:

```rust
#[derive(Debug, Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoadTestMetrics {
    pub total_sent: usize,
    pub total_success: usize,
    pub total_error: usize,
    pub rps: f64,
    pub start_time: u64,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_intensity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_rps_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_flight: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner_max_rps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RunnerInfoResponse>,
}
```

Update `MetricsAccumulator::snapshot` in `runner/src/server/metrics.rs` to call a new detailed method:

```rust
pub fn snapshot(
    &self,
    duration_ms: Option<u64>,
    runtime: Option<RunnerInfoResponse>,
) -> LoadTestMetrics {
    self.snapshot_with_wave(duration_ms, runtime, None)
}

pub fn snapshot_with_wave(
    &self,
    duration_ms: Option<u64>,
    runtime: Option<RunnerInfoResponse>,
    wave: Option<WaveMetricsSnapshot>,
) -> LoadTestMetrics {
    let elapsed = (now_ms().saturating_sub(self.start_time) as f64 / 1000.0).max(0.001);
    let rps = if elapsed > 0.0 {
        self.total_sent as f64 / elapsed
    } else {
        0.0
    };

    LoadTestMetrics {
        total_sent: self.total_sent,
        total_success: self.total_success,
        total_error: self.total_error,
        rps: round2(rps),
        start_time: self.start_time,
        elapsed_ms: now_ms().saturating_sub(self.start_time),
        target_intensity: wave.as_ref().map(|value| round2(value.target_intensity)),
        target_rps_limit: wave.as_ref().map(|value| round2(value.target_rps_limit)),
        in_flight: wave.as_ref().map(|value| value.in_flight),
        runner_max_rps: wave.as_ref().map(|value| round2(value.runner_max_rps)),
        tick_ms: wave.as_ref().map(|value| value.tick_ms),
        duration_ms,
        runtime,
    }
}
```

Add this struct in the same file:

```rust
#[derive(Debug, Clone, Copy)]
pub struct WaveMetricsSnapshot {
    pub target_intensity: f64,
    pub target_rps_limit: f64,
    pub in_flight: usize,
    pub runner_max_rps: f64,
    pub tick_ms: u64,
}
```

- [ ] **Step 2: Run metric compile test**

Run:

```bash
cargo test -p previa-runner metrics
```

Expected: compile errors in tests or call sites that construct `LoadTestMetrics` without the new fields.

- [ ] **Step 3: Fix metric call sites**

Update any `LoadTestMetrics` literals in runner tests with:

```rust
target_intensity: None,
target_rps_limit: None,
in_flight: None,
runner_max_rps: None,
tick_ms: None,
```

- [ ] **Step 4: Add request-shape validation in handler**

At the start of `run_load_test` in `runner/src/server/handlers/load.rs`, replace:

```rust
let config = payload.config;
```

with:

```rust
let load = payload.load.clone();
let config = payload.config.clone();
if load.is_none() && config.is_none() {
    return bad_request_message_response("either load or config must be provided");
}
if let Some(load) = load.as_ref() {
    if let Err(message) = crate::server::load_wave::validate_load_profile(load) {
        return bad_request_message_response(&message);
    }
}
```

- [ ] **Step 5: Extract classic path**

Move the existing classic worker loop in `runner/src/server/handlers/load.rs` into:

```rust
async fn run_classic_load(
    config: crate::server::models::LoadTestConfig,
    pipeline: previa_runner::Pipeline,
    selected_key: Option<String>,
    selected_env_group_slug: Option<String>,
    specs: Vec<previa_runner::RuntimeSpec>,
    env_groups: Vec<previa_runner::RuntimeEnvGroup>,
    tx: mpsc::UnboundedSender<SseMessage>,
    token: tokio_util::sync::CancellationToken,
) {
    // Body is the current totalRequests/concurrency/rampUpSeconds implementation
    // from the spawned task, starting at `let total_requests = config.total_requests.max(1);`
    // and ending before the final execution cleanup block.
}
```

Do not change the classic worker behavior in this step. The moved body still
calculates `total_requests`, `concurrency`, `ramp_interval_ms`, spawns
`concurrency` workers, uses the shared `AtomicUsize` counter, emits `metrics`,
awaits all handles, and emits `complete`.

- [ ] **Step 6: Implement wave path**

Add this function beside `run_classic_load`:

```rust
async fn run_wave_load(
    load: crate::server::models::LoadProfile,
    pipeline: previa_runner::Pipeline,
    selected_key: Option<String>,
    selected_env_group_slug: Option<String>,
    specs: Vec<previa_runner::RuntimeSpec>,
    env_groups: Vec<previa_runner::RuntimeEnvGroup>,
    tx: mpsc::UnboundedSender<SseMessage>,
    token: tokio_util::sync::CancellationToken,
) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use crate::server::load_bucket::FlowBucket;
    use crate::server::load_wave::{calculate_tick_ms, local_rps_limit, sample_intensity, timeline_end_ms};
    use crate::server::metrics::WaveMetricsSnapshot;

    let tick_ms = calculate_tick_ms(&load);
    let started = Instant::now();
    let end_ms = timeline_end_ms(&load);
    let metrics = Arc::new(tokio::sync::Mutex::new(MetricsAccumulator::new()));
    let runtime_sampler = Arc::new(tokio::sync::Mutex::new(RuntimeSampler::new()));
    let in_flight = Arc::new(AtomicUsize::new(0));
    let mut bucket = FlowBucket::new(local_rps_limit(&load, 0), 0);
    let mut handles = Vec::new();

    loop {
        if token.is_cancelled() {
            break;
        }
        let elapsed_ms = started.elapsed().as_millis() as u64;
        if elapsed_ms >= end_ms {
            break;
        }

        let target_intensity = sample_intensity(&load, elapsed_ms);
        let target_rps_limit = local_rps_limit(&load, elapsed_ms);
        bucket.refill(target_rps_limit, elapsed_ms);

        while in_flight.load(Ordering::SeqCst) < load.max_in_flight && bucket.try_acquire() {
            let pipeline = pipeline.clone();
            let selected_key = selected_key.clone();
            let selected_env_group_slug = selected_env_group_slug.clone();
            let specs = specs.clone();
            let env_groups = env_groups.clone();
            let tx = tx.clone();
            let token = token.clone();
            let metrics = Arc::clone(&metrics);
            let runtime_sampler = Arc::clone(&runtime_sampler);
            let in_flight = Arc::clone(&in_flight);
            let wave_snapshot = WaveMetricsSnapshot {
                target_intensity,
                target_rps_limit,
                in_flight: in_flight.load(Ordering::SeqCst) + 1,
                runner_max_rps: load.runner_max_rps,
                tick_ms,
            };

            in_flight.fetch_add(1, Ordering::SeqCst);
            handles.push(tokio::spawn(async move {
                let start = Instant::now();
                let results = execute_pipeline_with_runtime_hooks(
                    &pipeline,
                    selected_key.as_deref(),
                    Some(specs.as_slice()),
                    Some(env_groups.as_slice()),
                    selected_env_group_slug.as_deref(),
                    |_| {},
                    |_| {},
                    || token.is_cancelled(),
                )
                .await;
                let duration_ms = start.elapsed().as_millis() as u64;
                let success = !results.iter().any(|result| result.status == "error");
                let (network_tx_bytes, network_rx_bytes) =
                    estimate_results_network_bytes(&results);
                let runtime = {
                    let mut lock = runtime_sampler.lock().await;
                    lock.snapshot()
                };
                let snapshot = {
                    let mut lock = metrics.lock().await;
                    lock.update(duration_ms as f64, success);
                    lock.add_network_bytes(network_tx_bytes, network_rx_bytes);
                    lock.snapshot_with_wave(Some(duration_ms), runtime, Some(wave_snapshot))
                };
                in_flight.fetch_sub(1, Ordering::SeqCst);
                let _ = send_sse_or_cancel(
                    &tx,
                    "metrics",
                    serde_json::to_value(snapshot).unwrap_or(Value::Null),
                    &token,
                );
            }));
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(tick_ms)).await;
    }

    let grace_deadline = tokio::time::Instant::now()
        + tokio::time::Duration::from_millis(load.grace_period_ms);
    for handle in handles {
        let remaining = grace_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = tokio::time::timeout(remaining, handle).await;
    }

    let complete = {
        let lock = metrics.lock().await;
        let runtime = {
            let mut sampler = runtime_sampler.lock().await;
            sampler.snapshot()
        };
        lock.snapshot_with_wave(
            None,
            runtime,
            Some(WaveMetricsSnapshot {
                target_intensity: sample_intensity(&load, end_ms),
                target_rps_limit: local_rps_limit(&load, end_ms),
                in_flight: in_flight.load(Ordering::SeqCst),
                runner_max_rps: load.runner_max_rps,
                tick_ms,
            }),
        )
    };

    if !token.is_cancelled() {
        let _ = send_sse_or_cancel(
            &tx,
            "complete",
            serde_json::to_value(complete).unwrap_or(Value::Null),
            &token,
        );
    }
}
```

Adjust imports in `runner/src/server/handlers/load.rs` so the extracted functions compile.

- [ ] **Step 7: Dispatch to wave or classic**

Inside the spawned execution task in `run_load_test`, after `execution:init`, call:

```rust
match (load, config) {
    (Some(load), _) => {
        run_wave_load(
            load,
            pipeline,
            selected_key,
            selected_env_group_slug,
            specs,
            env_groups,
            tx.clone(),
            token.clone(),
        )
        .await;
    }
    (None, Some(config)) => {
        run_classic_load(
            config,
            pipeline,
            selected_key,
            selected_env_group_slug,
            specs,
            env_groups,
            tx.clone(),
            token.clone(),
        )
        .await;
    }
    (None, None) => {}
}
```

- [ ] **Step 8: Run runner load tests**

Run:

```bash
cargo test -p previa-runner
```

Expected: all runner tests pass.

- [ ] **Step 9: Commit**

```bash
git add runner/src/server/handlers/load.rs runner/src/server/models.rs runner/src/server/metrics.rs
git commit -m "feat(runner): execute wave load profiles"
```

## Task 5: Main Wave Models And Forwarding

**Files:**
- Modify: `main/src/server/models.rs`
- Modify: `main/src/server/execution/load.rs`
- Modify: `main/src/server/utils.rs`
- Modify: `main/src/server/execution/load_batch.rs`

- [ ] **Step 1: Add main model types**

Mirror the runner load types in `main/src/server/models.rs` near `LoadTestConfig`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadProfile {
    pub points: Vec<LoadPoint>,
    #[serde(default)]
    pub interpolation: LoadInterpolation,
    #[serde(default)]
    pub runner_max_rps: Option<f64>,
    #[serde(default)]
    pub max_in_flight: Option<usize>,
    #[serde(default)]
    pub grace_period_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadPoint {
    pub at_ms: u64,
    pub intensity: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LoadInterpolation {
    Smooth,
    Linear,
    Step,
}

impl Default for LoadInterpolation {
    fn default() -> Self {
        Self::Smooth
    }
}
```

Change `LoadTestRequest` and `ProjectLoadTestRequest` to accept both shapes:

```rust
#[serde(default)]
pub config: Option<LoadTestConfig>,
#[serde(default)]
pub load: Option<LoadProfile>,
```

- [ ] **Step 2: Run compile to find call sites**

Run:

```bash
cargo test -p previa-main load
```

Expected: compile errors where code assumes `payload.config` is non-optional.

- [ ] **Step 3: Add canonical runner load helper**

In `main/src/server/execution/load.rs`, add:

```rust
fn runner_load_profile(
    profile: &crate::server::models::LoadProfile,
    runner_max_rps: u64,
) -> serde_json::Value {
    json!({
        "points": profile.points,
        "interpolation": profile.interpolation,
        "runnerMaxRps": profile.runner_max_rps.unwrap_or(runner_max_rps as f64),
        "maxInFlight": profile.max_in_flight.unwrap_or_else(|| runner_max_rps.max(1) as usize),
        "gracePeriodMs": profile.grace_period_ms.unwrap_or(30_000)
    })
}
```

- [ ] **Step 4: Validate load shape before scheduling**

Near the start of `start_load_execution`, after the pipeline empty check, add:

```rust
if payload.load.is_none() && payload.config.is_none() {
    return Err(StartLoadExecutionError::BadRequest(
        "either load or config must be provided".to_owned(),
    ));
}
if let Some(load) = payload.load.as_ref() {
    validate_main_load_profile(load)?;
}
```

Add the validation function in the same file:

```rust
fn validate_main_load_profile(
    profile: &crate::server::models::LoadProfile,
) -> Result<(), StartLoadExecutionError> {
    if profile.points.len() < 2 {
        return Err(StartLoadExecutionError::BadRequest(
            "load.points must contain at least two points".to_owned(),
        ));
    }
    if profile.points[0].at_ms != 0 {
        return Err(StartLoadExecutionError::BadRequest(
            "load.points[0].atMs must be 0".to_owned(),
        ));
    }
    for point in &profile.points {
        if !(0.0..=100.0).contains(&point.intensity) {
            return Err(StartLoadExecutionError::BadRequest(
                "load.points intensity must be between 0 and 100".to_owned(),
            ));
        }
    }
    for pair in profile.points.windows(2) {
        if pair[1].at_ms <= pair[0].at_ms {
            return Err(StartLoadExecutionError::BadRequest(
                "load.points must be strictly increasing by atMs".to_owned(),
            ));
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Preserve scheduling for classic and wave**

Replace direct `payload.config` plan inputs with:

```rust
let planning_rps = payload
    .load
    .as_ref()
    .map(|_| state.rps_per_node)
    .unwrap_or_else(|| payload.config.as_ref().map(|config| config.concurrency as u64).unwrap_or(1))
    .max(1);
let planning_total_requests = payload
    .config
    .as_ref()
    .map(|config| config.total_requests.max(1))
    .unwrap_or(usize::MAX);
let planning_concurrency = payload
    .config
    .as_ref()
    .map(|config| config.concurrency.max(1))
    .unwrap_or(active_nodes.len().max(1));
```

Use those values in `calculate_node_plan`.

- [ ] **Step 6: Forward wave payloads to runners**

In the runner loop that builds `child_request`, replace the `"config"`-only object with:

```rust
let child_request = if let Some(load_profile) = runner_load_profile_for_children.as_ref() {
    json!({
        "pipeline": pipeline,
        "selectedBaseUrlKey": selected_base_url_key,
        "selectedEnvGroupSlug": selected_env_group_slug,
        "specs": specs,
        "envGroups": env_groups,
        "load": runner_load_profile(load_profile, state_clone.rps_per_node)
    })
} else {
    json!({
        "pipeline": pipeline,
        "selectedBaseUrlKey": selected_base_url_key,
        "selectedEnvGroupSlug": selected_env_group_slug,
        "specs": specs,
        "envGroups": env_groups,
        "config": {
            "totalRequests": split_requests[index],
            "concurrency": split_concurrency[index],
            "rampUpSeconds": runner_ramp_up_seconds
        }
    })
};
```

Define `runner_load_profile_for_children` before the runner loop:

```rust
let runner_load_profile_for_children = runner_config_wave.clone();
```

Use an earlier variable:

```rust
let runner_config_wave = payload.load.clone();
let runner_config_classic = payload.config.clone();
```

- [ ] **Step 7: Parse new metric fields**

In `main/src/server/models.rs`, add optional fields to `RunnerLoadMetricsPoint`:

```rust
pub target_intensity: Option<f64>,
pub target_rps_limit: Option<f64>,
pub in_flight: Option<usize>,
pub runner_max_rps: Option<f64>,
pub tick_ms: Option<u64>,
```

In `main/src/server/utils.rs`, update `parse_runner_load_metrics`:

```rust
target_intensity: get_f64_field(payload, "targetIntensity"),
target_rps_limit: get_f64_field(payload, "targetRpsLimit"),
in_flight: get_usize_field(payload, "inFlight"),
runner_max_rps: get_f64_field(payload, "runnerMaxRps"),
tick_ms: get_u64_field(payload, "tickMs"),
```

- [ ] **Step 8: Run main tests**

Run:

```bash
cargo test -p previa-main
```

Expected: all main tests pass after updating any model literals with the new optional fields.

- [ ] **Step 9: Commit**

```bash
git add main/src/server/models.rs main/src/server/execution/load.rs main/src/server/utils.rs main/src/server/execution/load_batch.rs
git commit -m "feat(main): forward wave load profiles"
```

## Task 6: MCP And OpenAPI Surface

**Files:**
- Modify: `runner/src/server/docs.rs`
- Modify: `main/src/server/docs.rs`
- Modify: `main/src/server/mcp/models.rs`
- Modify: `main/src/server/mcp/service.rs`

- [ ] **Step 1: Add wave schemas to docs**

In both docs modules, include the new schema types beside `LoadTestConfig`:

```rust
LoadProfile,
LoadPoint,
LoadInterpolation,
```

- [ ] **Step 2: Update MCP argument model**

In `main/src/server/mcp/models.rs`, change `RunProjectLoadTestArgs`:

```rust
pub struct RunProjectLoadTestArgs {
    pub project_id: String,
    pub pipeline_id: String,
    pub config: Option<LoadTestConfig>,
    pub load: Option<crate::server::models::LoadProfile>,
    pub selected_base_url_key: Option<String>,
    pub selected_env_group_slug: Option<String>,
}
```

- [ ] **Step 3: Update MCP resolver**

In `resolve_project_load_request`, build:

```rust
Ok(LoadTestRequest {
    pipeline,
    config: args.config,
    load: args.load,
    selected_base_url_key: args.selected_base_url_key,
    selected_env_group_slug: args.selected_env_group_slug,
    project_id: Some(args.project_id),
    pipeline_index: position,
    specs,
    env_groups,
})
```

- [ ] **Step 4: Update MCP tool schema**

In `run_project_load_test` schema in `main/src/server/mcp/service.rs`, replace the config-only required list with:

```json
"oneOf": [
  { "required": ["config"] },
  { "required": ["load"] }
]
```

Add `load` properties:

```json
"load": {
  "type": "object",
  "required": ["points"],
  "properties": {
    "points": {
      "type": "array",
      "minItems": 2,
      "items": {
        "type": "object",
        "required": ["atMs", "intensity"],
        "properties": {
          "atMs": { "type": "integer", "minimum": 0 },
          "intensity": { "type": "number", "minimum": 0, "maximum": 100 }
        }
      }
    },
    "interpolation": { "type": "string", "enum": ["smooth", "linear", "step"] },
    "maxInFlight": { "type": "integer", "minimum": 1 },
    "gracePeriodMs": { "type": "integer", "minimum": 0 }
  }
}
```

- [ ] **Step 5: Update load test designer prompt**

In `load_test_designer_prompt`, replace references to exact `totalRequests`, `concurrency`, and `rampUpSeconds` as the primary model with:

```text
3. Prefer a wave load payload with points as { atMs, intensity }, where intensity is 0-100 percent of each runner's configured safe RPS capacity.
4. Use smooth interpolation by default. Use step only for explicit spike/degradation tests.
5. Highlight operational risks such as overly high intensity, missing assertions, unstable environments, or maxInFlight pressure.
```

- [ ] **Step 6: Run MCP tests**

Run:

```bash
cargo test -p previa-main mcp
```

Expected: all MCP tests pass after updating expected schema/prompt strings.

- [ ] **Step 7: Commit**

```bash
git add runner/src/server/docs.rs main/src/server/docs.rs main/src/server/mcp/models.rs main/src/server/mcp/service.rs
git commit -m "feat(main): expose wave load API"
```

## Task 7: App Types And API Client

**Files:**
- Modify: `app/src/types/load-test.ts`
- Modify: `app/src/lib/remote-executor.ts`
- Modify: `app/src/stores/useLoadTestHistoryStore.ts`
- Modify: `app/src/lib/remote-executor.test.ts`

- [ ] **Step 1: Add TypeScript wave types**

In `app/src/types/load-test.ts`, add:

```ts
export type LoadInterpolation = "smooth" | "linear" | "step";

export interface LoadPoint {
  atMs: number;
  intensity: number;
}

export interface WaveLoadConfig {
  points: LoadPoint[];
  interpolation: LoadInterpolation;
  maxInFlight?: number;
  gracePeriodMs?: number;
}
```

Extend `LoadTestMetrics`:

```ts
targetIntensity?: number;
targetRpsLimit?: number;
inFlight?: number;
runnerMaxRps?: number;
tickMs?: number;
```

- [ ] **Step 2: Change pending config shape**

Where stores accept `LoadTestConfig`, change the union to:

```ts
export type LoadRunConfig = LoadTestConfig | WaveLoadConfig;
```

When sending a run, use:

```ts
const isWaveConfig = (cfg: LoadRunConfig): cfg is WaveLoadConfig =>
  Array.isArray((cfg as WaveLoadConfig).points);
```

- [ ] **Step 3: Send `load` payloads**

In `app/src/lib/remote-executor.ts`, change `runRemoteLoadTest` to accept
`LoadRunConfig` and build the request body with this branch:

```ts
const body = isWaveConfig(cfg)
  ? { pipelineId, selectedBaseUrlKey, selectedEnvGroupSlug, load: cfg, specs, envGroups }
  : { pipelineId, selectedBaseUrlKey, selectedEnvGroupSlug, config: cfg, specs, envGroups };
```

- [ ] **Step 4: Add unit tests for payload shape**

In `app/src/lib/remote-executor.test.ts`, add:

```ts
it("sends wave load config as load payload", async () => {
  const fetchMock = vi.fn().mockResolvedValue(
    new Response("event: execution:init\ndata: {\"executionId\":\"exec-1\"}\n\n", {
      status: 200,
      headers: { "content-type": "text/event-stream" },
    }),
  );
  vi.stubGlobal("fetch", fetchMock);

  const controller = runRemoteLoadTest(
    "http://localhost:5588",
    { id: "pipe-1", name: "Pipe", description: null, steps: [] },
    {
      points: [
        { atMs: 0, intensity: 10 },
        { atMs: 60_000, intensity: 80 },
      ],
      interpolation: "smooth",
      maxInFlight: 200,
      gracePeriodMs: 30_000,
    },
    {
      onMetricsUpdate: vi.fn(),
      onComplete: vi.fn(),
      onError: vi.fn(),
      onExecutionInit: vi.fn(),
    },
    "project-1",
  );

  await vi.waitFor(() => expect(fetchMock).toHaveBeenCalled());
  controller.cancel();

  const body = JSON.parse(fetchMock.mock.calls[0][1].body);
  expect(body.load.points).toEqual([
    { atMs: 0, intensity: 10 },
    { atMs: 60000, intensity: 80 },
  ]);
  expect(body.config).toBeUndefined();
});
```

- [ ] **Step 5: Run app tests**

Run:

```bash
cd app && npm test -- api-client
```

Expected: API client tests pass.

- [ ] **Step 6: Commit**

```bash
git add app/src/types/load-test.ts app/src/lib/remote-executor.ts app/src/stores/useLoadTestHistoryStore.ts app/src/lib/remote-executor.test.ts
git commit -m "feat(app): send wave load configs"
```

## Task 8: Wave Load UI

**Files:**
- Modify: `app/src/components/LoadTestConfigPanel.tsx`
- Modify: `app/src/components/LoadTestResultsPanel.tsx`
- Modify: `app/src/i18n/locales/en.json`
- Modify: `app/src/i18n/locales/pt-BR.json`

- [ ] **Step 1: Add preset helpers**

In `LoadTestConfigPanel.tsx`, add:

```ts
const wavePresets = {
  baseline: [
    { atMs: 0, intensity: 30 },
    { atMs: 300_000, intensity: 30 },
  ],
  ramp: [
    { atMs: 0, intensity: 10 },
    { atMs: 120_000, intensity: 80 },
  ],
  spike: [
    { atMs: 0, intensity: 20 },
    { atMs: 30_000, intensity: 100 },
    { atMs: 60_000, intensity: 20 },
  ],
  soak: [
    { atMs: 0, intensity: 50 },
    { atMs: 7_200_000, intensity: 50 },
  ],
} satisfies Record<string, LoadPoint[]>;

function normalizeWavePoints(points: LoadPoint[]): LoadPoint[] {
  return [...points].sort((a, b) => a.atMs - b.atMs);
}
```

- [ ] **Step 2: Replace classic sliders with wave state**

Use state:

```ts
const [points, setPoints] = useState<LoadPoint[]>(wavePresets.ramp);
const [interpolation, setInterpolation] = useState<LoadInterpolation>("smooth");
const [maxInFlight, setMaxInFlight] = useState(200);
const [gracePeriodMs, setGracePeriodMs] = useState(30_000);
```

Call `onConfigChange` with:

```ts
onConfigChange?.(
  {
    points: normalizeWavePoints(points),
    interpolation,
    maxInFlight,
    gracePeriodMs,
  },
  selectedEnv,
);
```

- [ ] **Step 3: Render point editor**

Use existing `Input`, `Button`, `Select`, and `SliderWithManual` patterns in the component to render:

```tsx
{points.map((point, index) => (
  <div key={`${point.atMs}-${index}`} className="grid grid-cols-[1fr_1fr_auto] gap-2">
    <Input
      type="number"
      min={0}
      value={point.atMs}
      onChange={(event) => {
        const next = [...points];
        next[index] = { ...point, atMs: Number(event.target.value) };
        setPoints(normalizeWavePoints(next));
      }}
    />
    <Input
      type="number"
      min={0}
      max={100}
      value={point.intensity}
      onChange={(event) => {
        const next = [...points];
        next[index] = {
          ...point,
          intensity: Math.min(100, Math.max(0, Number(event.target.value))),
        };
        setPoints(next);
      }}
    />
    <Button
      type="button"
      variant="ghost"
      size="icon"
      disabled={points.length <= 2}
      onClick={() => setPoints(points.filter((_, i) => i !== index))}
    >
      <Trash2 className="h-4 w-4" />
    </Button>
  </div>
))}
```

Add `Plus`, `Trash2`, and fitting lucide imports if they are not already present.

- [ ] **Step 4: Render visual preview**

Add a compact SVG preview below the point editor:

```tsx
<svg viewBox="0 0 100 40" className="h-24 w-full">
  <polyline
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    points={points
      .map((point) => {
        const maxMs = Math.max(...points.map((p) => p.atMs), 1);
        const x = (point.atMs / maxMs) * 100;
        const y = 40 - (point.intensity / 100) * 40;
        return `${x},${y}`;
      })
      .join(" ")}
  />
</svg>
```

- [ ] **Step 5: Show wave metrics in results**

In `LoadTestResultsPanel.tsx`, add metric rows when present:

```tsx
{typeof metrics.targetIntensity === "number" && (
  <Metric label={t("loadTest.targetIntensity")} value={`${metrics.targetIntensity.toFixed(1)}%`} />
)}
{typeof metrics.targetRpsLimit === "number" && (
  <Metric label={t("loadTest.targetRpsLimit")} value={metrics.targetRpsLimit.toFixed(1)} />
)}
{typeof metrics.inFlight === "number" && (
  <Metric label={t("loadTest.inFlight")} value={metrics.inFlight.toLocaleString()} />
)}
```

Use the panel's existing metric component/style rather than creating nested cards.

- [ ] **Step 6: Add translations**

In `en.json`:

```json
"targetIntensity": "Target intensity",
"targetRpsLimit": "Target RPS limit",
"inFlight": "In flight"
```

In `pt-BR.json`:

```json
"targetIntensity": "Intensidade alvo",
"targetRpsLimit": "Limite de RPS alvo",
"inFlight": "Em voo"
```

Place these keys under the existing `loadTest` namespace.

- [ ] **Step 7: Run UI tests and typecheck**

Run:

```bash
cd app && npm test -- LoadTest
cd app && npm run typecheck
```

Expected: tests and typecheck pass.

- [ ] **Step 8: Commit**

```bash
git add app/src/components/LoadTestConfigPanel.tsx app/src/components/LoadTestResultsPanel.tsx app/src/i18n/locales/en.json app/src/i18n/locales/pt-BR.json
git commit -m "feat(app): add wave load editor"
```

## Task 9: Documentation And Release Verification

**Files:**
- Modify: `docs/previa/api-workflows.md`
- Modify: `docs/previa/examples-cookbook.md`
- Modify: `docs/previa/glossary.md`
- Modify: `app/docs/test-execution-api.yaml` if generated docs changed

- [ ] **Step 1: Update API workflow example**

In `docs/previa/api-workflows.md`, replace the load curl body with:

```bash
curl -N http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/load \
  -H 'content-type: application/json' \
  -d "{\"pipelineId\":\"$PIPELINE_ID\",\"selectedBaseUrlKey\":\"hml\",\"load\":{\"points\":[{\"atMs\":0,\"intensity\":10},{\"atMs\":60000,\"intensity\":80},{\"atMs\":120000,\"intensity\":30}],\"interpolation\":\"smooth\",\"maxInFlight\":200,\"gracePeriodMs\":30000},\"specs\":[]}"
```

- [ ] **Step 2: Update cookbook**

In `docs/previa/examples-cookbook.md`, add:

```md
## Wave Load Test

Wave load tests use elapsed time and intensity percentage. A runner treats
`100%` as its configured safe RPS capacity.

```bash
curl -N http://127.0.0.1:5588/api/v1/projects/$PROJECT_ID/tests/load \
  -H 'content-type: application/json' \
  -d '{"pipelineId":"users-crud","selectedBaseUrlKey":"hml","load":{"points":[{"atMs":0,"intensity":10},{"atMs":60000,"intensity":80},{"atMs":120000,"intensity":30}],"interpolation":"smooth","maxInFlight":200,"gracePeriodMs":30000},"specs":[]}'
```
```

- [ ] **Step 3: Update glossary**

In `docs/previa/glossary.md`, replace the load-test definition with:

```md
## load test

A pipeline execution repeated under a timeline-based wave of load intensity.
The wave maps elapsed time to an intensity percentage, and runners translate
that percentage into local request flow using their configured safe capacity.
```

- [ ] **Step 4: Run full verification**

Run:

```bash
cargo test
cd app && npm test
cd app && npm run typecheck
cargo build --release
```

Expected: all commands pass.

- [ ] **Step 5: Commit**

```bash
git add docs/previa/api-workflows.md docs/previa/examples-cookbook.md docs/previa/glossary.md app/docs/test-execution-api.yaml
git commit -m "docs: document wave load tests"
```

## Self-Review

- Spec coverage: the plan covers wave as the canonical load model, runner-local execution, percentage points, smooth/linear/step interpolation, automatic tick calculation, bucket limiting, `maxInFlight`, timeline completion, `RUNNER_RPS_PER_NODE`-derived runner capacity, metrics, main aggregation, compatibility, UI, docs, and tests.
- Placeholder scan: the plan uses concrete file paths, commands, expected outcomes, and code snippets for each implementation step.
- Type consistency: model names are consistent across Rust and TypeScript: `LoadProfile`, `LoadPoint`, `LoadInterpolation`, `load.points`, `atMs`, `intensity`, `runnerMaxRps`, `maxInFlight`, and `gracePeriodMs`.
