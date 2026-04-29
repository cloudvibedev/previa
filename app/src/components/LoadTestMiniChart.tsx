import { useState, useEffect, useMemo } from "react";
import { TrendingUp, CheckCircle2, XCircle } from "lucide-react";
import {
  AreaChart, Area, ResponsiveContainer,
  BarChart, Bar, Tooltip,
} from "recharts";
import { useLoadTestHistoryStore } from "@/stores/useLoadTestHistoryStore";
import type { LoadTestRunRecord } from "@/lib/load-test-store";

interface LoadTestMiniChartProps {
  projectId: string;
  pipelineIndex: number;
  refreshKey?: number;
  executionBackendUrl?: string;
}

export function LoadTestMiniChart({ projectId, pipelineIndex, refreshKey, executionBackendUrl }: LoadTestMiniChartProps) {
  const [runs, setRuns] = useState<LoadTestRunRecord[]>([]);
  const loadHistory = useLoadTestHistoryStore((s) => s.loadHistory);
  const storeRuns = useLoadTestHistoryStore((s) => s.runs);

  useEffect(() => {
    loadHistory(projectId, pipelineIndex, executionBackendUrl, false);
  }, [projectId, pipelineIndex, refreshKey, executionBackendUrl, loadHistory]);

  useEffect(() => {
    setRuns(storeRuns);
  }, [storeRuns]);

  const last10 = useMemo(() => {
    return [...runs]
      .sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime())
      .slice(-10);
  }, [runs]);

  const hasLatencyData = last10.some(r => r.metrics.avgLatency > 0);

  const latencyData = useMemo(() => last10.map((r, i) => ({
    i,
    d: Math.round(r.metrics.avgLatency),
    label: `${Math.round(r.metrics.avgLatency)}ms`,
  })), [last10]);

  const rateData = useMemo(() => {
    return last10.map((r, i) => ({
      i,
      success: r.metrics.totalSuccess,
      error: r.metrics.totalError,
    }));
  }, [last10]);

  const totalSuccess = runs.reduce((s, r) => s + r.metrics.totalSuccess, 0);
  const totalError = runs.reduce((s, r) => s + r.metrics.totalError, 0);
  const total = totalSuccess + totalError;
  const successPct = total > 0 ? Math.round((totalSuccess / total) * 100) : 0;
  const runsWithLatency = runs.filter(r => r.metrics.avgLatency > 0);
  const avgLatency = runsWithLatency.length > 0 ? Math.round(runsWithLatency.reduce((s, r) => s + r.metrics.avgLatency, 0) / runsWithLatency.length) : 0;
  const lastLatency = last10.length > 0 ? Math.round(last10[last10.length - 1].metrics.avgLatency) : 0;

  const hasData = runs.length > 0;

  return (
    <div className="glass grid grid-cols-[1fr_1fr] gap-3 border-border/50 p-3">
      {/* Latency chart */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <TrendingUp className="h-3 w-3 text-primary" />
            <span className="text-[11px] font-medium text-foreground">Latência</span>
          </div>
          <span className="text-[10px] text-muted-foreground">
            avg <span className="font-mono font-medium text-foreground">{hasLatencyData ? `${avgLatency}ms` : '-'}</span>
          </span>
        </div>
        <div className="h-10 w-full">
          {hasLatencyData ? (
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={latencyData} margin={{ top: 2, right: 2, bottom: 0, left: 2 }}>
                <defs>
                  <linearGradient id="miniLatencyGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="hsl(var(--primary))" stopOpacity={0.3} />
                    <stop offset="100%" stopColor="hsl(var(--primary))" stopOpacity={0.02} />
                  </linearGradient>
                </defs>
                <Tooltip
                  contentStyle={{ fontSize: 10, padding: "2px 6px", borderRadius: 6, background: "hsl(var(--popover))", border: "1px solid hsl(var(--border))", color: "hsl(var(--foreground))" }}
                  labelFormatter={() => ""}
                  formatter={(v: number) => [`${v}ms`, "Latência"]}
                />
                <Area
                  type="monotone"
                  dataKey="d"
                  stroke="hsl(var(--primary))"
                  fill="url(#miniLatencyGrad)"
                  strokeWidth={1.5}
                  dot={false}
                  activeDot={{ r: 2.5, strokeWidth: 0, fill: "hsl(var(--primary))" }}
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-full flex items-center justify-center text-[10px] text-muted-foreground">Sem dados de latência</div>
          )}
        </div>
        <span className="text-[9px] text-muted-foreground">
          última: <span className="font-mono text-foreground">{hasLatencyData ? `${lastLatency}ms` : '-'}</span>
        </span>
      </div>

      {/* Success/Error chart */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            {successPct >= 50 ? (
              <CheckCircle2 className="h-3 w-3 text-success" />
            ) : (
              <XCircle className="h-3 w-3 text-destructive" />
            )}
            <span className="text-[11px] font-medium text-foreground">Taxa</span>
          </div>
          <span className="text-[10px] text-muted-foreground">
            <span className="font-mono font-medium text-success">{successPct}%</span>
            {" "}de {total}
          </span>
        </div>
        <div className="h-10 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={rateData} margin={{ top: 2, right: 2, bottom: 0, left: 2 }}>
              <Bar dataKey="success" stackId="a" fill="hsl(var(--success))" radius={[1, 1, 0, 0]} />
              <Bar dataKey="error" stackId="a" fill="hsl(var(--destructive))" radius={[1, 1, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
        <div className="flex items-center gap-2 text-[9px] text-muted-foreground">
          <span className="flex items-center gap-1">
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-success" />
            {totalSuccess} ok
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-destructive" />
            {totalError} erro
          </span>
        </div>
      </div>
    </div>
  );
}
