# Wave Lifecycle Chart Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a frontend chart that explains where the wave load pipeline diverges by plotting planned dispatch, send start, HTTP start, HTTP send return, and response body completion over time.

**Architecture:** Keep the existing `HTTP RPS over time` chart focused on RPS shape and per-runner lines. Add a second aggregate chart below it, built from the same `metrics.rpsHistory`, that converts lifecycle counters into one-second deltas. Prefer direct dispatch buckets for `dispatchStarted` when present, and use cumulative-counter deltas for the other lifecycle phases.

**Tech Stack:** React, TypeScript, Recharts, Vitest, existing `LoadTestResultsPanel`, existing `LoadTestMetrics` and `RpsPoint` types.

---

## Context

The backend already exposes the lifecycle counters needed for this first UI pass:

```ts
scheduledStarts
dispatchSubmitted
slotEnqueued
requestPrepared
requestEnqueued
sendTaskSpawned
sendStarted
dispatchStarted
httpStarted
httpSendReturned
responseBodyCompleted
```

The RPS chart currently answers:

```text
How many requests started per second, and how close was that to target RPS?
```

The new lifecycle chart should answer:

```text
Where did starts stop moving at wave speed?
```

Expected diagnostic reading:

```text
planned ~= sendStarted ~= httpStarted
```

means the open-loop algorithm is healthy.

```text
httpStarted follows the wave, but httpSendReturned / responseBodyCompleted lag
```

means the target, network, or HTTP stack is saturated after request start.

```text
sendStarted or httpStarted no longer follows planned
```

means the runner/runtime/OS is saturated before actual HTTP start.

---

## File Structure

- Create `app/src/lib/load-lifecycle-chart.ts`
  - Owns all lifecycle chart data transformation.
  - Converts cumulative counters into per-second rates.
  - Preserves direct `dispatchBucket` for dispatch-started RPS when available.
  - Exposes series metadata for labels/colors.

- Modify `app/src/components/LoadTestResultsPanel.tsx`
  - Imports `buildLifecycleChartData`.
  - Renders a new chart directly below `HTTP RPS over time`.
  - Uses aggregate lifecycle lines only. Do not add per-runner lifecycle lines in this task, because the RPS chart already shows runner split and the lifecycle chart would become visually noisy.

- Modify `app/src/components/LoadTestResultsPanel.test.tsx`
  - Adds unit tests for the lifecycle transformer.
  - Adds a render test proving the lifecycle chart appears only when enough lifecycle history exists.

- Modify `app/src/i18n/locales/en.json`
  - Adds English labels for the chart and series.

- Modify `app/src/i18n/locales/pt-BR.json`
  - Adds Portuguese labels for the chart and series.

---

### Task 1: Build Lifecycle Chart Data

**Files:**
- Create: `app/src/lib/load-lifecycle-chart.ts`
- Test: `app/src/components/LoadTestResultsPanel.test.tsx`

- [ ] **Step 1: Write failing tests for lifecycle chart data**

Add this import to `app/src/components/LoadTestResultsPanel.test.tsx`:

```ts
import { buildLifecycleChartData } from "@/lib/load-lifecycle-chart";
```

Add this test inside `describe("LoadTestResultsPanel", () => { ... })`:

```ts
it("builds lifecycle chart rows from cumulative counters", () => {
  const metrics: LoadTestMetrics = {
    ...emptyMetrics,
    rpsHistory: [
      {
        timestamp: 1_000,
        elapsedMs: 0,
        rps: 0,
        scheduledStarts: 0,
        sendStarted: 0,
        httpStarted: 0,
        httpSendReturned: 0,
        responseBodyCompleted: 0,
      },
      {
        timestamp: 2_000,
        elapsedMs: 1_000,
        rps: 0,
        scheduledStarts: 100,
        sendStarted: 98,
        httpStarted: 97,
        httpSendReturned: 40,
        responseBodyCompleted: 10,
      },
      {
        timestamp: 3_000,
        elapsedMs: 2_000,
        rps: 0,
        scheduledStarts: 250,
        sendStarted: 245,
        httpStarted: 244,
        httpSendReturned: 90,
        responseBodyCompleted: 20,
      },
    ],
  };

  expect(buildLifecycleChartData(metrics)).toEqual({
    data: [
      {
        time: 1,
        planned: 100,
        sendStarted: 98,
        httpStarted: 97,
        httpSendReturned: 40,
        responseBodyCompleted: 10,
      },
      {
        time: 2,
        planned: 150,
        sendStarted: 147,
        httpStarted: 147,
        httpSendReturned: 50,
        responseBodyCompleted: 10,
      },
    ],
    series: [
      { key: "planned", labelKey: "loadTestResults.lifecyclePlanned", tone: "planned" },
      { key: "sendStarted", labelKey: "loadTestResults.lifecycleSendStarted", tone: "send" },
      { key: "httpStarted", labelKey: "loadTestResults.lifecycleHttpStarted", tone: "http" },
      { key: "httpSendReturned", labelKey: "loadTestResults.lifecycleHttpSendReturned", tone: "returned" },
      { key: "responseBodyCompleted", labelKey: "loadTestResults.lifecycleBodyCompleted", tone: "body" },
    ],
  });
});
```

Add a second test proving direct dispatch bucket wins for `httpStarted`/actual dispatch line:

```ts
it("uses direct dispatch buckets for the HTTP started lifecycle line when available", () => {
  const metrics: LoadTestMetrics = {
    ...emptyMetrics,
    rpsHistory: [
      {
        timestamp: 1_000,
        elapsedMs: 0,
        rps: 0,
        scheduledStarts: 100,
        dispatchBucket: 99,
        sendStarted: 100,
        httpStarted: 10_000,
      },
      {
        timestamp: 2_000,
        elapsedMs: 1_000,
        rps: 0,
        scheduledStarts: 200,
        dispatchBucket: 125,
        sendStarted: 200,
        httpStarted: 20_000,
      },
    ],
  };

  expect(buildLifecycleChartData(metrics).data).toEqual([
    {
      time: 0,
      planned: 100,
      sendStarted: 100,
      httpStarted: 99,
      httpSendReturned: 0,
      responseBodyCompleted: 0,
    },
    {
      time: 1,
      planned: 100,
      sendStarted: 100,
      httpStarted: 125,
      httpSendReturned: 0,
      responseBodyCompleted: 0,
    },
  ]);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
```

Expected:

```text
Failed to resolve import "@/lib/load-lifecycle-chart"
```

- [ ] **Step 3: Implement transformer**

Create `app/src/lib/load-lifecycle-chart.ts`:

```ts
import type { LoadTestMetrics, RpsPoint } from "@/types/load-test";

export type LifecycleSeriesKey =
  | "planned"
  | "sendStarted"
  | "httpStarted"
  | "httpSendReturned"
  | "responseBodyCompleted";

export type LifecycleSeriesTone = "planned" | "send" | "http" | "returned" | "body";

export interface LifecycleSeries {
  key: LifecycleSeriesKey;
  labelKey: string;
  tone: LifecycleSeriesTone;
}

export interface LifecycleChartRow {
  time: number;
  planned: number;
  sendStarted: number;
  httpStarted: number;
  httpSendReturned: number;
  responseBodyCompleted: number;
}

export interface LifecycleChartData {
  data: LifecycleChartRow[];
  series: LifecycleSeries[];
}

const SERIES: LifecycleSeries[] = [
  { key: "planned", labelKey: "loadTestResults.lifecyclePlanned", tone: "planned" },
  { key: "sendStarted", labelKey: "loadTestResults.lifecycleSendStarted", tone: "send" },
  { key: "httpStarted", labelKey: "loadTestResults.lifecycleHttpStarted", tone: "http" },
  { key: "httpSendReturned", labelKey: "loadTestResults.lifecycleHttpSendReturned", tone: "returned" },
  { key: "responseBodyCompleted", labelKey: "loadTestResults.lifecycleBodyCompleted", tone: "body" },
];

function elapsedMsForPoint(point: RpsPoint, metrics: LoadTestMetrics) {
  return typeof point.elapsedMs === "number"
    ? point.elapsedMs
    : point.timestamp - metrics.startTime;
}

function bucketSecond(point: RpsPoint, metrics: LoadTestMetrics) {
  return Math.max(0, Math.floor(elapsedMsForPoint(point, metrics) / 1000));
}

function cumulativeDelta(current: number | undefined, previous: number | undefined) {
  if (typeof current !== "number") return 0;
  if (typeof previous !== "number") return Math.max(0, current);
  return Math.max(0, current - previous);
}

function ensureRow(rows: Map<number, LifecycleChartRow>, time: number): LifecycleChartRow {
  const existing = rows.get(time);
  if (existing) return existing;

  const row: LifecycleChartRow = {
    time,
    planned: 0,
    sendStarted: 0,
    httpStarted: 0,
    httpSendReturned: 0,
    responseBodyCompleted: 0,
  };
  rows.set(time, row);
  return row;
}

function roundOne(value: number) {
  return Math.round(value * 10) / 10;
}

export function buildLifecycleChartData(metrics: LoadTestMetrics): LifecycleChartData {
  const history = metrics.rpsHistory ?? [];
  if (history.length === 0) return { data: [], series: SERIES };

  const rows = new Map<number, LifecycleChartRow>();

  for (let index = 0; index < history.length; index += 1) {
    const point = history[index];
    const previous = history[index - 1];
    const time = bucketSecond(point, metrics);
    const row = ensureRow(rows, time);

    row.planned += cumulativeDelta(point.scheduledStarts, previous?.scheduledStarts);
    row.sendStarted += cumulativeDelta(point.sendStarted, previous?.sendStarted);
    row.httpStarted += typeof point.dispatchBucket === "number"
      ? point.dispatchBucket
      : cumulativeDelta(point.httpStarted ?? point.dispatchStarted, previous?.httpStarted ?? previous?.dispatchStarted);
    row.httpSendReturned += cumulativeDelta(point.httpSendReturned, previous?.httpSendReturned);
    row.responseBodyCompleted += cumulativeDelta(point.responseBodyCompleted, previous?.responseBodyCompleted);
  }

  const data = Array.from(rows.values())
    .sort((a, b) => a.time - b.time)
    .map((row) => ({
      ...row,
      planned: roundOne(row.planned),
      sendStarted: roundOne(row.sendStarted),
      httpStarted: roundOne(row.httpStarted),
      httpSendReturned: roundOne(row.httpSendReturned),
      responseBodyCompleted: roundOne(row.responseBodyCompleted),
    }))
    .filter((row) =>
      row.planned > 0 ||
      row.sendStarted > 0 ||
      row.httpStarted > 0 ||
      row.httpSendReturned > 0 ||
      row.responseBodyCompleted > 0
    );

  return { data, series: SERIES };
}
```

- [ ] **Step 4: Run transformer tests**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
```

Expected:

```text
LoadTestResultsPanel.test.tsx passes
```

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/load-lifecycle-chart.ts app/src/components/LoadTestResultsPanel.test.tsx
git commit -m "test: add wave lifecycle chart data"
```

---

### Task 2: Render Lifecycle Chart In Results Panel

**Files:**
- Modify: `app/src/components/LoadTestResultsPanel.tsx`
- Test: `app/src/components/LoadTestResultsPanel.test.tsx`

- [ ] **Step 1: Add render test**

Add this test to `LoadTestResultsPanel.test.tsx`:

```ts
it("renders wave lifecycle chart when lifecycle history exists", () => {
  const metrics: LoadTestMetrics = {
    ...emptyMetrics,
    rpsHistory: [
      {
        timestamp: 1_000,
        elapsedMs: 0,
        rps: 0,
        scheduledStarts: 0,
        sendStarted: 0,
        httpStarted: 0,
        httpSendReturned: 0,
        responseBodyCompleted: 0,
      },
      {
        timestamp: 2_000,
        elapsedMs: 1_000,
        rps: 0,
        scheduledStarts: 100,
        sendStarted: 99,
        httpStarted: 99,
        httpSendReturned: 30,
        responseBodyCompleted: 5,
      },
    ],
  };

  render(<LoadTestResultsPanel metrics={metrics} state="running" totalRequests={0} />);

  expect(screen.getByTestId("wave-lifecycle-chart")).toBeInTheDocument();
  expect(screen.getByText("loadTestResults.waveLifecycle")).toBeInTheDocument();
  expect(screen.getByText("loadTestResults.lifecyclePlanned")).toBeInTheDocument();
  expect(screen.getByText("loadTestResults.lifecycleHttpStarted")).toBeInTheDocument();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
```

Expected:

```text
Unable to find an element by: [data-testid="wave-lifecycle-chart"]
```

- [ ] **Step 3: Import lifecycle chart builder**

In `LoadTestResultsPanel.tsx`, add:

```ts
import { buildLifecycleChartData, type LifecycleSeriesTone } from "@/lib/load-lifecycle-chart";
```

Add color helper near `RUNNER_RESOURCE_COLORS`:

```ts
const LIFECYCLE_COLORS: Record<LifecycleSeriesTone, string> = {
  planned: "hsl(var(--primary))",
  send: "hsl(var(--status-running))",
  http: "hsl(var(--status-success))",
  returned: "hsl(var(--status-warning))",
  body: "hsl(var(--muted-foreground))",
};
```

Inside `LoadTestResultsPanel`, after `const rpsChartData = rpsChart.data;`, add:

```ts
const lifecycleChart = buildLifecycleChartData(metrics);
const lifecycleChartData = lifecycleChart.data;
```

- [ ] **Step 4: Render chart below RPS chart**

In `LoadTestResultsPanel.tsx`, immediately after the existing RPS chart block, add:

```tsx
      {lifecycleChartData.length > 1 && (
        <div data-testid="wave-lifecycle-chart" className="glass rounded-lg p-3 space-y-2">
          <div className="flex items-center justify-between gap-2">
            <p className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider">
              {t("loadTestResults.waveLifecycle")}
            </p>
            <div className="flex items-center gap-2 text-[9px] text-muted-foreground flex-wrap justify-end">
              {lifecycleChart.series.map((series) => (
                <span key={series.key} className="inline-flex items-center gap-1">
                  <span
                    className="h-0 w-3 border-t"
                    style={{ borderColor: LIFECYCLE_COLORS[series.tone] }}
                  />
                  {t(series.labelKey)}
                </span>
              ))}
            </div>
          </div>
          <ResponsiveContainer width="100%" height={120}>
            <LineChart data={lifecycleChartData}>
              <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
              <XAxis dataKey="time" tick={{ fontSize: 9 }} stroke="hsl(var(--muted-foreground))" tickFormatter={(v) => `${v}s`} />
              <YAxis tick={{ fontSize: 9 }} stroke="hsl(var(--muted-foreground))" />
              <RechartsTooltip
                contentStyle={{
                  background: "hsl(var(--popover))",
                  border: "1px solid hsl(var(--border))",
                  borderRadius: "var(--radius)",
                  fontSize: 11,
                }}
                formatter={(v: number, name: string) => [
                  typeof v === "number" ? v.toFixed(1) : v,
                  t(lifecycleChart.series.find((series) => series.key === name)?.labelKey ?? name),
                ]}
                labelFormatter={(v) => `${v}s`}
              />
              {lifecycleChart.series.map((series) => (
                <Line
                  key={series.key}
                  type="monotone"
                  dataKey={series.key}
                  stroke={LIFECYCLE_COLORS[series.tone]}
                  strokeWidth={series.key === "planned" || series.key === "httpStarted" ? 1.75 : 1.4}
                  strokeDasharray={series.key === "planned" ? "2 4" : undefined}
                  dot={false}
                  connectNulls
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </div>
      )}
```

- [ ] **Step 5: Run panel tests**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
```

Expected:

```text
18+ tests pass, including "renders wave lifecycle chart when lifecycle history exists"
```

- [ ] **Step 6: Commit**

```bash
git add app/src/components/LoadTestResultsPanel.tsx app/src/components/LoadTestResultsPanel.test.tsx
git commit -m "feat: render wave lifecycle chart"
```

---

### Task 3: Add Translations

**Files:**
- Modify: `app/src/i18n/locales/en.json`
- Modify: `app/src/i18n/locales/pt-BR.json`
- Test: `app/src/components/LoadTestResultsPanel.test.tsx`

- [ ] **Step 1: Add English keys**

In `app/src/i18n/locales/en.json`, add these entries near existing `loadTestResults` keys:

```json
"loadTestResults.waveLifecycle": "Wave lifecycle",
"loadTestResults.lifecyclePlanned": "Planned",
"loadTestResults.lifecycleSendStarted": "Send started",
"loadTestResults.lifecycleHttpStarted": "HTTP started",
"loadTestResults.lifecycleHttpSendReturned": "Send returned",
"loadTestResults.lifecycleBodyCompleted": "Body completed"
```

- [ ] **Step 2: Add Portuguese keys**

In `app/src/i18n/locales/pt-BR.json`, add:

```json
"loadTestResults.waveLifecycle": "Ciclo da wave",
"loadTestResults.lifecyclePlanned": "Planejado",
"loadTestResults.lifecycleSendStarted": "Envio iniciado",
"loadTestResults.lifecycleHttpStarted": "HTTP iniciado",
"loadTestResults.lifecycleHttpSendReturned": "Send retornado",
"loadTestResults.lifecycleBodyCompleted": "Body concluído"
```

- [ ] **Step 3: Run JSON/build checks**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
npm --prefix app run build
```

Expected:

```text
LoadTestResultsPanel tests pass
Vite build succeeds
```

- [ ] **Step 4: Commit**

```bash
git add app/src/i18n/locales/en.json app/src/i18n/locales/pt-BR.json
git commit -m "chore: add wave lifecycle chart translations"
```

---

### Task 4: Verify In Browser Against Current Test Data

**Files:**
- No code files expected.

- [ ] **Step 1: Run TypeScript check**

Run:

```bash
cd app && ./node_modules/.bin/tsc --noEmit
```

Expected:

```text
No TypeScript errors
```

- [ ] **Step 2: Run full focused verification**

Run:

```bash
npm --prefix app test -- LoadTestResultsPanel
npm --prefix app run build
cargo build --release
```

Expected:

```text
All pass
```

- [ ] **Step 3: Restart main with app enabled and existing DB**

Run:

```bash
for port in 5610 5611 5612 5613; do
  lsof -ti tcp:$port | while read pid; do
    [ -n "$pid" ] && kill "$pid" 2>/dev/null || true
  done
done
sleep 1
screen -S previa-wave -X quit >/dev/null 2>&1 || true
screen -dmS previa-wave zsh -lc '
  cd /Users/assis/projects/previa
  RUST_LOG=info PORT=5611 target/release/previa-runner > /tmp/previa-runner-5611.log 2>&1 &
  RUST_LOG=info PORT=5612 target/release/previa-runner > /tmp/previa-runner-5612.log 2>&1 &
  RUST_LOG=info PORT=5613 target/release/previa-runner > /tmp/previa-runner-5613.log 2>&1 &
  RUST_LOG=info PREVIA_APP_ENABLED=1 ORCHESTRATOR_DATABASE_URL=sqlite:///private/tmp/previa-verify-5610.db PORT=5610 RUNNER_ENDPOINTS=http://127.0.0.1:5611,http://127.0.0.1:5612,http://127.0.0.1:5613 target/release/previa-main > /tmp/previa-main-5610.log 2>&1
'
sleep 2
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

- [ ] **Step 4: Browser QA**

Open:

```text
http://127.0.0.1:5610/projects/019de1a7-4dfd-7662-8b53-a305e5714ca5/pipeline/019de1a7-4dfd-7662-8b53-a317b9bdbe23/load-test
```

Verify:

```text
HTTP RPS over time remains visible.
Wave lifecycle appears below HTTP RPS over time.
Legend shows Planned, Send started, HTTP started, Send returned, Body completed.
Chart does not overlap adjacent cards.
Mobile/narrow layout remains scrollable.
```

- [ ] **Step 5: Commit if verification required small fixes**

If browser QA required layout fixes:

```bash
git add app/src/components/LoadTestResultsPanel.tsx app/src/components/LoadTestResultsPanel.test.tsx
git commit -m "fix: polish wave lifecycle chart layout"
```

---

## Success Criteria

- A new lifecycle chart appears below `HTTP RPS over time` when `metrics.rpsHistory` contains lifecycle counters.
- The chart shows aggregate per-second lines for:
  - planned
  - send started
  - HTTP started
  - send returned
  - body completed
- Direct `dispatchBucket` is used for the HTTP-start/dispatch-start line when available.
- The chart makes target-vs-runner-vs-response bottlenecks visually obvious without adding another dense per-runner view.
- `npm --prefix app test -- LoadTestResultsPanel`, `npm --prefix app run build`, and `cargo build --release` pass.

## Self-Review

- Spec coverage: covers frontend graph creation, lifecycle interpretation, chart placement, translations, tests, and local QA.
- Placeholder scan: no TBD/TODO placeholders.
- Type consistency: all new types are introduced in `load-lifecycle-chart.ts`; tests import the same exported builder.
