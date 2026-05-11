# Local Load Target Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a deterministic local HTTP API target so Previa wave load tests can validate scheduler/runner behavior without depending on an external service.

**Architecture:** Add a separate workspace binary crate named `previa-load-target`. It runs outside `previa-main` and outside the runners, exposes load endpoints plus metrics endpoints, and can be started together with the local Previa stack by a helper script. The Previa project/pipeline is seeded through existing HTTP APIs so the UI can run the reference load test normally.

**Tech Stack:** Rust 2024, Axum 0.8, Tokio, Serde, existing Previa HTTP APIs, Bash, curl, jq.

---

## File Structure

- Create `load-target/Cargo.toml`: package manifest for the isolated local load target binary.
- Create `load-target/src/main.rs`: Axum app, request counters, delay/failure behavior, metrics/reset endpoints, and unit tests.
- Modify `Cargo.toml`: add `load-target` as a workspace member.
- Create `fixtures/local-load-target.previa.yaml`: importable pipeline that hits `{{envs.local.api}}`.
- Create `scripts/start-local-load-target-stack.sh`: builds the app, starts load target + main + three runners, and seeds a reference project through the main API.
- Modify `README.md`: add a short local load target workflow.

---

### Task 1: Create the Local Load Target Crate Skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `load-target/Cargo.toml`
- Create: `load-target/src/main.rs`

- [ ] **Step 1: Write the failing workspace package check**

Run:

```bash
cargo metadata --format-version=1 --no-deps | jq -r '.packages[].name' | rg '^previa-load-target$'
```

Expected: FAIL because the package does not exist yet.

- [ ] **Step 2: Add the workspace member**

In `Cargo.toml`, change the workspace members from:

```toml
members = ['engine', 'main', 'previa', 'runner']
```

to:

```toml
members = ['engine', 'main', 'previa', 'runner', 'load-target']
```

- [ ] **Step 3: Create `load-target/Cargo.toml`**

```toml
[package]
name = "previa-load-target"
description = "Deterministic local HTTP target for Previa load-test validation."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
publish = false

[dependencies]
axum = { workspace = true }
dotenvy = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tower-http = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[dev-dependencies]
http-body-util = "0.1"
tower = { version = "0.5", features = ["util"] }
```

- [ ] **Step 4: Create the minimal binary**

Create `load-target/src/main.rs`:

```rust
use axum::{Json, Router, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
}

fn app() -> Router {
    Router::new().route("/health", get(|| async { Json(HealthResponse { status: "ok" }) }))
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let address = std::env::var("ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5620);
    let bind_addr = format!("{address}:{port}");

    let listener = TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind load target listener");
    info!("previa-load-target listening on http://{}", listener.local_addr().expect("local addr"));

    axum::serve(listener, app())
        .await
        .expect("failed to start load target");
}
```

- [ ] **Step 5: Verify the package exists and compiles**

Run:

```bash
cargo metadata --format-version=1 --no-deps | jq -r '.packages[].name' | rg '^previa-load-target$'
cargo check -p previa-load-target
```

Expected: both commands pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml load-target/Cargo.toml load-target/src/main.rs
git commit -m "Add local load target crate"
```

---

### Task 2: Add Deterministic Metrics and Load Endpoints

**Files:**
- Modify: `load-target/src/main.rs`

- [ ] **Step 1: Write failing tests for `/load/ok`, `/metrics`, and `/metrics/reset`**

Append this test module to `load-target/src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde_json::Value;
    use tower::ServiceExt;

    use super::app;

    async fn json_response(app: Router, uri: &str) -> (StatusCode, Value) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).expect("request"))
            .await
            .expect("response");
        let status = response.status();
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let value = serde_json::from_slice::<Value>(&body).expect("json");
        (status, value)
    }

    use axum::Router;

    #[tokio::test]
    async fn ok_endpoint_increments_metrics() {
        let app = app();

        let (status, body) = json_response(app.clone(), "/load/ok").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");

        let (status, metrics) = json_response(app, "/metrics").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(metrics["totalRequests"], 1);
        assert_eq!(metrics["totalOk"], 1);
        assert_eq!(metrics["totalErrors"], 0);
    }

    #[tokio::test]
    async fn reset_endpoint_clears_metrics() {
        let app = app();

        let _ = json_response(app.clone(), "/load/ok").await;
        let (status, reset) = json_response(app.clone(), "/metrics/reset").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(reset["status"], "reset");

        let (_status, metrics) = json_response(app, "/metrics").await;
        assert_eq!(metrics["totalRequests"], 0);
        assert_eq!(metrics["perSecond"].as_array().expect("per second").len(), 0);
    }
}
```

Run:

```bash
cargo test -p previa-load-target ok_endpoint_increments_metrics reset_endpoint_clears_metrics -- --nocapture
```

Expected: FAIL because `/load/ok`, `/metrics`, and `/metrics/reset` are missing.

- [ ] **Step 2: Implement shared state and metrics endpoints**

Replace `load-target/src/main.rs` with:

```rust
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::time::{Duration, sleep};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[derive(Debug, Default)]
struct Counters {
    started_at_ms: u128,
    total_requests: u64,
    total_ok: u64,
    total_errors: u64,
    total_latency_ms: u128,
    per_second: BTreeMap<u64, SecondBucket>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SecondBucket {
    second: u64,
    requests: u64,
    ok: u64,
    errors: u64,
    avg_latency_ms: f64,
}

#[derive(Debug, Clone)]
struct AppState {
    counters: Arc<Mutex<Counters>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadResponse {
    status: &'static str,
    total_requests: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetricsResponse {
    started_at_ms: u128,
    elapsed_ms: u128,
    total_requests: u64,
    total_ok: u64,
    total_errors: u64,
    avg_latency_ms: f64,
    current_rps: u64,
    per_second: Vec<SecondBucket>,
}

#[derive(Debug, Deserialize)]
struct SlowQuery {
    ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct FailQuery {
    rate: Option<u64>,
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before epoch")
        .as_millis()
}

fn app_state() -> AppState {
    AppState {
        counters: Arc::new(Mutex::new(Counters {
            started_at_ms: now_ms(),
            ..Counters::default()
        })),
    }
}

fn app() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/load/ok", get(load_ok))
        .route("/load/slow", get(load_slow))
        .route("/load/fail", get(load_fail))
        .route("/metrics", get(metrics))
        .route("/metrics/reset", get(reset_metrics))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(app_state())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn load_ok(State(state): State<AppState>) -> Json<LoadResponse> {
    let total_requests = record_request(&state, false, 0);
    Json(LoadResponse {
        status: "ok",
        total_requests,
    })
}

async fn load_slow(State(state): State<AppState>, Query(query): Query<SlowQuery>) -> Json<LoadResponse> {
    let delay_ms = query.ms.unwrap_or(50).min(30_000);
    sleep(Duration::from_millis(delay_ms)).await;
    let total_requests = record_request(&state, false, delay_ms);
    Json(LoadResponse {
        status: "ok",
        total_requests,
    })
}

async fn load_fail(
    State(state): State<AppState>,
    Query(query): Query<FailQuery>,
) -> (StatusCode, Json<LoadResponse>) {
    let rate = query.rate.unwrap_or(100).clamp(1, 100);
    let should_fail = {
        let counters = state.counters.lock().expect("metrics lock");
        ((counters.total_requests + 1) % 100) < rate
    };
    let total_requests = record_request(&state, should_fail, 0);
    let status = if should_fail {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    };
    let body_status = if should_fail { "error" } else { "ok" };
    (
        status,
        Json(LoadResponse {
            status: body_status,
            total_requests,
        }),
    )
}

async fn metrics(State(state): State<AppState>) -> Json<MetricsResponse> {
    Json(snapshot_metrics(&state))
}

async fn reset_metrics(State(state): State<AppState>) -> Json<HealthResponse> {
    let mut counters = state.counters.lock().expect("metrics lock");
    *counters = Counters {
        started_at_ms: now_ms(),
        ..Counters::default()
    };
    Json(HealthResponse { status: "reset" })
}

fn record_request(state: &AppState, failed: bool, latency_ms: u64) -> u64 {
    let now = now_ms();
    let mut counters = state.counters.lock().expect("metrics lock");
    let elapsed_ms = now.saturating_sub(counters.started_at_ms);
    let second = (elapsed_ms / 1000) as u64;
    counters.total_requests += 1;
    counters.total_latency_ms += latency_ms as u128;
    if failed {
        counters.total_errors += 1;
    } else {
        counters.total_ok += 1;
    }
    let total_requests = counters.total_requests;
    let bucket = counters.per_second.entry(second).or_insert_with(|| SecondBucket {
        second,
        ..SecondBucket::default()
    });
    bucket.requests += 1;
    if failed {
        bucket.errors += 1;
    } else {
        bucket.ok += 1;
    }
    bucket.avg_latency_ms = if bucket.requests > 0 {
        latency_ms as f64
    } else {
        0.0
    };
    total_requests
}

fn snapshot_metrics(state: &AppState) -> MetricsResponse {
    let now = now_ms();
    let counters = state.counters.lock().expect("metrics lock");
    let elapsed_ms = now.saturating_sub(counters.started_at_ms);
    let latest_second = counters.per_second.keys().next_back().copied();
    let current_rps = latest_second
        .and_then(|second| counters.per_second.get(&second).map(|bucket| bucket.requests))
        .unwrap_or(0);
    MetricsResponse {
        started_at_ms: counters.started_at_ms,
        elapsed_ms,
        total_requests: counters.total_requests,
        total_ok: counters.total_ok,
        total_errors: counters.total_errors,
        avg_latency_ms: if counters.total_requests == 0 {
            0.0
        } else {
            counters.total_latency_ms as f64 / counters.total_requests as f64
        },
        current_rps,
        per_second: counters.per_second.values().cloned().collect(),
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let address = std::env::var("ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5620);
    let bind_addr = format!("{address}:{port}");

    let listener = TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind load target listener");
    info!("previa-load-target listening on http://{}", listener.local_addr().expect("local addr"));

    axum::serve(listener, app())
        .await
        .expect("failed to start load target");
}
```

- [ ] **Step 3: Run tests**

Run:

```bash
cargo test -p previa-load-target -- --nocapture
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add load-target/src/main.rs
git commit -m "Add deterministic local load target endpoints"
```

---

### Task 3: Add the Reference Pipeline Fixture

**Files:**
- Create: `fixtures/local-load-target.previa.yaml`

- [ ] **Step 1: Write the fixture**

```yaml
id: local-load-target
name: Local Load Target - Open Loop Reference
description: Pipeline de referencia para medir wave load test contra uma API local deterministica.
steps:
  - id: local_target_ok
    name: Local target OK
    description: Endpoint rapido usado para validar a curva de envio sem depender de servico externo.
    method: GET
    url: "{{envs.local.api}}/load/ok"
    headers: {}
    asserts:
      - field: status
        operator: equals
        expected: "200"
      - field: body.status
        operator: equals
        expected: ok
```

- [ ] **Step 2: Validate the fixture parses as a pipeline**

Run:

```bash
cargo test -p previa pipeline_import::tests::imports_pipelines_into_new_project_and_preserves_ids -- --nocapture
python3 - <<'PY'
import yaml
with open("fixtures/local-load-target.previa.yaml", "r", encoding="utf-8") as f:
    data = yaml.safe_load(f)
assert data["name"] == "Local Load Target - Open Loop Reference"
assert data["steps"][0]["url"] == "{{envs.local.api}}/load/ok"
print("fixture ok")
PY
```

Expected: Rust test passes and Python prints `fixture ok`.

- [ ] **Step 3: Commit**

```bash
git add fixtures/local-load-target.previa.yaml
git commit -m "Add local load target pipeline fixture"
```

---

### Task 4: Add a Stack Script that Starts and Seeds the Reference Test

**Files:**
- Create: `scripts/start-local-load-target-stack.sh`

- [ ] **Step 1: Write the script**

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN_PORT="${MAIN_PORT:-5610}"
TARGET_PORT="${TARGET_PORT:-5620}"
DB_URL="${ORCHESTRATOR_DATABASE_URL:-sqlite:///private/tmp/previa-local-load-target.db}"
SCREEN_NAME="${SCREEN_NAME:-previa-local-load-target}"

cd "$ROOT"

npm --prefix app run build
cargo build --release

screen -S "$SCREEN_NAME" -X quit 2>/dev/null || true
for port in "$MAIN_PORT" 5611 5612 5613 "$TARGET_PORT"; do
  pid="$(lsof -ti "tcp:$port" 2>/dev/null || true)"
  if [ -n "$pid" ]; then
    kill "$pid" 2>/dev/null || true
  fi
done

screen -dmS "$SCREEN_NAME" zsh -lc "
  cd '$ROOT'
  RUST_LOG=info PORT=$TARGET_PORT target/release/previa-load-target > /tmp/previa-load-target-$TARGET_PORT.log 2>&1 &
  RUST_LOG=info PORT=5611 target/release/previa-runner > /tmp/previa-runner-5611.log 2>&1 &
  RUST_LOG=info PORT=5612 target/release/previa-runner > /tmp/previa-runner-5612.log 2>&1 &
  RUST_LOG=info PORT=5613 target/release/previa-runner > /tmp/previa-runner-5613.log 2>&1 &
  RUST_LOG=info PREVIA_APP_ENABLED=1 ORCHESTRATOR_DATABASE_URL='$DB_URL' PORT=$MAIN_PORT RUNNER_ENDPOINTS=http://127.0.0.1:5611,http://127.0.0.1:5612,http://127.0.0.1:5613 target/release/previa-main > /tmp/previa-main-$MAIN_PORT.log 2>&1
"

for url in "http://127.0.0.1:$TARGET_PORT/health" "http://127.0.0.1:$MAIN_PORT/info"; do
  for attempt in $(seq 1 40); do
    if curl -fsS "$url" >/dev/null; then
      break
    fi
    if [ "$attempt" -eq 40 ]; then
      echo "Service did not become ready: $url" >&2
      exit 1
    fi
    sleep 0.25
  done
done

PROJECT_ID="$(
  curl -fsS -X POST "http://127.0.0.1:$MAIN_PORT/api/v1/projects" \
    -H 'content-type: application/json' \
    -d '{"name":"Local Load Target Reference","description":"Projeto local para validar wave open-loop contra API deterministica.","pipelines":[]}' \
    | jq -r '.id'
)"

curl -fsS -X POST "http://127.0.0.1:$MAIN_PORT/api/v1/projects/$PROJECT_ID/env-groups" \
  -H 'content-type: application/json' \
  -d "{\"slug\":\"local\",\"name\":\"Local\",\"entries\":[{\"name\":\"api\",\"url\":\"http://127.0.0.1:$TARGET_PORT\",\"description\":\"Local deterministic load target\"}]}" >/dev/null

PIPELINE_PAYLOAD="$(
  python3 - <<'PY'
import json, yaml
with open("fixtures/local-load-target.previa.yaml", "r", encoding="utf-8") as f:
    data = yaml.safe_load(f)
data.pop("id", None)
print(json.dumps(data))
PY
)"

PIPELINE_ID="$(
  curl -fsS -X POST "http://127.0.0.1:$MAIN_PORT/api/v1/projects/$PROJECT_ID/pipelines" \
    -H 'content-type: application/json' \
    -d "$PIPELINE_PAYLOAD" \
    | jq -r '.id'
)"

curl -fsS "http://127.0.0.1:$TARGET_PORT/metrics/reset" >/dev/null

echo "Main:       http://127.0.0.1:$MAIN_PORT"
echo "Target:     http://127.0.0.1:$TARGET_PORT"
echo "Metrics:    http://127.0.0.1:$TARGET_PORT/metrics"
echo "Load test:  http://127.0.0.1:$MAIN_PORT/projects/$PROJECT_ID/pipeline/$PIPELINE_ID/load-test"
```

- [ ] **Step 2: Make it executable**

Run:

```bash
chmod +x scripts/start-local-load-target-stack.sh
```

- [ ] **Step 3: Run the script and verify services**

Run:

```bash
scripts/start-local-load-target-stack.sh
curl -fsS http://127.0.0.1:5620/metrics | jq '{totalRequests,totalOk,totalErrors,currentRps}'
curl -fsS http://127.0.0.1:5610/info | jq '{activeRunners,totalRunners}'
```

Expected:

```json
{
  "totalRequests": 0,
  "totalOk": 0,
  "totalErrors": 0,
  "currentRps": 0
}
```

and:

```json
{
  "activeRunners": 3,
  "totalRunners": 3
}
```

- [ ] **Step 4: Commit**

```bash
git add scripts/start-local-load-target-stack.sh
git commit -m "Add local load target stack script"
```

---

### Task 5: Add Documentation for the Reference Workflow

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add this section after the local usage section**

````markdown
### Local wave load target

For validating wave load-test behavior without an external API, start the deterministic local target stack:

```bash
scripts/start-local-load-target-stack.sh
```

The script starts:

- Previa main on `http://127.0.0.1:5610`
- three runners on `5611`, `5612`, and `5613`
- the deterministic load target on `http://127.0.0.1:5620`

It also creates a project named `Local Load Target Reference` and prints the load-test URL. During or after a run, inspect the target-side counters:

```bash
curl -fsS http://127.0.0.1:5620/metrics | jq
```

Use this target to compare the configured wave, runner HTTP RPS, and target-side received RPS without depending on DNS, gateway behavior, or a remote application.
````

- [ ] **Step 2: Verify markdown contains the command**

Run:

```bash
rg -n "Local wave load target|start-local-load-target-stack|5620/metrics" README.md
```

Expected: all three terms are found.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "Document local wave load target workflow"
```

---

### Task 6: Final Verification

**Files:**
- Verify all files changed in previous tasks.

- [ ] **Step 1: Run focused tests**

```bash
cargo test -p previa-load-target -- --nocapture
npm --prefix app run build
cargo build --release
```

Expected: all commands pass.

- [ ] **Step 2: Run the full local reference stack**

```bash
scripts/start-local-load-target-stack.sh
```

Expected: script prints `Main`, `Target`, `Metrics`, and `Load test` URLs.

- [ ] **Step 3: Prove target metrics receive traffic**

Run:

```bash
curl -fsS http://127.0.0.1:5620/load/ok >/dev/null
curl -fsS http://127.0.0.1:5620/load/ok >/dev/null
curl -fsS http://127.0.0.1:5620/metrics | jq '{totalRequests,totalOk,totalErrors}'
```

Expected:

```json
{
  "totalRequests": 2,
  "totalOk": 2,
  "totalErrors": 0
}
```

- [ ] **Step 4: Commit final verification updates if any**

If Task 6 required file changes, stage the exact files changed by the verification fix:

```bash
git add docs/superpowers/plans/2026-05-07-local-load-target.md
git commit -m "Polish local load target workflow"
```

If Task 6 did not require file changes, do not create an empty commit.

- [ ] **Step 5: Push**

```bash
git push origin codex/wave-load-test
```

---

## Self-Review

- Spec coverage: the plan creates an isolated API target, exposes deterministic success/slow/failure endpoints, exposes target-side metrics, seeds a Previa project/pipeline, and documents how to run it.
- Placeholder scan: the plan contains concrete files, commands, expected outputs, and implementation snippets.
- Type consistency: endpoint names, JSON field names, ports, crate name, and script paths are consistent across tasks.
