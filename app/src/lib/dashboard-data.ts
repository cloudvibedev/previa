import type { ExecutionRun } from "@/lib/execution-store";
import type { Pipeline } from "@/types/pipeline";

// 1. Duration History — one line per pipeline over time
export interface DurationHistoryPoint {
  timestamp: string;
  [pipelineName: string]: string | number;
}

export function buildDurationHistory(runs: ExecutionRun[]): DurationHistoryPoint[] {
  const sorted = [...runs].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  );

  return sorted.map((run) => ({
    timestamp: new Date(run.timestamp).toLocaleString("pt-BR", {
      day: "2-digit",
      month: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    }),
    [run.pipelineName]: run.duration,
  }));
}

// Collect unique pipeline names from runs
export function getPipelineNames(runs: ExecutionRun[]): string[] {
  return [...new Set(runs.map((r) => r.pipelineName))];
}

// 2. Success Rate — success vs error count
export interface SuccessRateData {
  name: string;
  value: number;
  fill: string;
}

export function buildSuccessRate(runs: ExecutionRun[]): SuccessRateData[] {
  const success = runs.filter((r) => r.status === "success").length;
  const error = runs.filter((r) => r.status === "error").length;
  return [
    { name: "Sucesso", value: success, fill: "var(--color-success)" },
    { name: "Erro", value: error, fill: "var(--color-error)" },
  ];
}

// 3. Step Durations — average duration per step for a specific pipeline
export interface StepDurationData {
  step: string;
  avgDuration: number;
}

export function buildStepDurations(runs: ExecutionRun[], pipelineIndex: number): StepDurationData[] {
  const filtered = runs.filter((r) => r.pipelineIndex === pipelineIndex);
  if (!filtered.length) return [];

  const stepTotals: Record<string, { total: number; count: number }> = {};

  for (const run of filtered) {
    for (const [stepId, result] of Object.entries(run.results)) {
      if (result.duration !== undefined) {
        if (!stepTotals[stepId]) stepTotals[stepId] = { total: 0, count: 0 };
        stepTotals[stepId].total += result.duration;
        stepTotals[stepId].count += 1;
      }
    }
  }

  return Object.entries(stepTotals).map(([step, { total, count }]) => ({
    step,
    avgDuration: Math.round(total / count),
  }));
}

// 4. Assertion Stats — passed vs failed per step
export interface AssertionStatsData {
  step: string;
  passed: number;
  failed: number;
}

export function buildAssertionStats(runs: ExecutionRun[], pipelineIndex: number): AssertionStatsData[] {
  const filtered = runs.filter((r) => r.pipelineIndex === pipelineIndex);
  if (!filtered.length) return [];

  const stepStats: Record<string, { passed: number; failed: number }> = {};

  for (const run of filtered) {
    for (const [stepId, result] of Object.entries(run.results)) {
      if (result.assertResults?.length) {
        if (!stepStats[stepId]) stepStats[stepId] = { passed: 0, failed: 0 };
        for (const a of result.assertResults) {
          if (a.passed) stepStats[stepId].passed++;
          else stepStats[stepId].failed++;
        }
      }
    }
  }

  return Object.entries(stepStats).map(([step, stats]) => ({
    step,
    ...stats,
  }));
}

// 5. Status Codes — distribution of HTTP status codes
export interface StatusCodeData {
  code: string;
  count: number;
  fill: string;
}

export function buildStatusCodeDistribution(runs: ExecutionRun[]): StatusCodeData[] {
  const counts: Record<string, number> = {};

  for (const run of runs) {
    for (const result of Object.values(run.results)) {
      if (result.response?.status) {
        const category = `${Math.floor(result.response.status / 100)}xx`;
        counts[category] = (counts[category] || 0) + 1;
      }
    }
  }

  const colorMap: Record<string, string> = {
    "2xx": "var(--color-success)",
    "3xx": "var(--color-info)",
    "4xx": "var(--color-warning)",
    "5xx": "var(--color-error)",
  };

  return Object.entries(counts)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([code, count]) => ({
      code,
      count,
      fill: colorMap[code] || "var(--color-muted)",
    }));
}

// 6. Execution Timeline — runs per day
export interface TimelineData {
  date: string;
  executions: number;
}

export function buildExecutionTimeline(runs: ExecutionRun[]): TimelineData[] {
  const dayCounts: Record<string, number> = {};

  for (const run of runs) {
    const day = new Date(run.timestamp).toLocaleDateString("pt-BR");
    dayCounts[day] = (dayCounts[day] || 0) + 1;
  }

  return Object.entries(dayCounts)
    .sort(([a], [b]) => {
      const [da, ma, ya] = a.split("/").map(Number);
      const [db, mb, yb] = b.split("/").map(Number);
      return new Date(ya, ma - 1, da).getTime() - new Date(yb, mb - 1, db).getTime();
    })
    .map(([date, executions]) => ({ date, executions }));
}

// 7. Pipeline Comparison — radar chart data
export interface PipelineComparisonData {
  metric: string;
  [pipelineName: string]: string | number;
}

export function buildPipelineComparison(runs: ExecutionRun[], pipelines: Pipeline[]): PipelineComparisonData[] {
  const pipelineStats: Record<string, { totalDuration: number; successCount: number; totalRuns: number; steps: number }> = {};

  for (let i = 0; i < pipelines.length; i++) {
    const name = pipelines[i].name;
    const pipelineRuns = runs.filter((r) => r.pipelineIndex === i);
    pipelineStats[name] = {
      totalDuration: pipelineRuns.reduce((sum, r) => sum + r.duration, 0),
      successCount: pipelineRuns.filter((r) => r.status === "success").length,
      totalRuns: pipelineRuns.length,
      steps: pipelines[i].steps.length,
    };
  }

  // Normalize values 0-100 for radar
  const names = Object.keys(pipelineStats);
  if (!names.length) return [];

  const maxDuration = Math.max(...names.map((n) => pipelineStats[n].totalRuns > 0 ? pipelineStats[n].totalDuration / pipelineStats[n].totalRuns : 0), 1);
  const maxSteps = Math.max(...names.map((n) => pipelineStats[n].steps), 1);

  const metrics = ["Duração Média", "Taxa de Sucesso", "Nº de Steps"];

  return metrics.map((metric) => {
    const point: PipelineComparisonData = { metric };
    for (const name of names) {
      const s = pipelineStats[name];
      if (metric === "Duração Média") {
        const avg = s.totalRuns > 0 ? s.totalDuration / s.totalRuns : 0;
        point[name] = Math.round((1 - avg / maxDuration) * 100); // invert: lower is better
      } else if (metric === "Taxa de Sucesso") {
        point[name] = s.totalRuns > 0 ? Math.round((s.successCount / s.totalRuns) * 100) : 0;
      } else {
        point[name] = Math.round((s.steps / maxSteps) * 100);
      }
    }
    return point;
  });
}
