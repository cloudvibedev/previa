import { useState, useEffect, useMemo } from "react";
import { TrendingUp, CheckCircle2, XCircle } from "lucide-react";
import {
  AreaChart, Area, ResponsiveContainer,
  BarChart, Bar, Tooltip,
} from "recharts";
import { useExecutionHistoryStore } from "@/stores/useExecutionHistoryStore";
import type { ExecutionRun } from "@/lib/execution-store";

interface PipelineMiniChartProps {
  projectId: string;
  pipelineIndex: number;
  refreshKey?: number;
  executionBackendUrl?: string;
}

export function PipelineMiniChart({ projectId, pipelineIndex, refreshKey, executionBackendUrl }: PipelineMiniChartProps) {
  const [runs, setRuns] = useState<ExecutionRun[]>([]);
  const loadHistory = useExecutionHistoryStore((s) => s.loadHistory);
  const storeRuns = useExecutionHistoryStore((s) => s.runs);

  useEffect(() => {
    loadHistory(projectId, pipelineIndex, executionBackendUrl);
  }, [projectId, pipelineIndex, refreshKey, executionBackendUrl, loadHistory]);

  useEffect(() => {
    setRuns(storeRuns);
  }, [storeRuns]);

  const completedRuns = useMemo(() => runs.filter(r => r.status !== "running"), [runs]);

  const last10 = useMemo(() => {
    return [...completedRuns]
      .sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime())
      .slice(-10);
  }, [completedRuns]);

  const durationData = useMemo(() => last10.map((r, i) => ({
    i,
    d: r.duration,
    label: `${r.duration}ms`,
  })), [last10]);

  const rateData = useMemo(() => {
    return last10.map((r, i) => ({
      i,
      success: r.status === "success" ? 1 : 0,
      error: r.status === "error" ? 1 : 0,
    }));
  }, [last10]);

  const successCount = completedRuns.filter((r) => r.status === "success").length;
  const errorCount = completedRuns.filter((r) => r.status === "error").length;
  const total = successCount + errorCount;
  const successPct = total > 0 ? Math.round((successCount / total) * 100) : 0;
  const avgDuration = total > 0 ? Math.round(completedRuns.reduce((s, r) => s + r.duration, 0) / total) : 0;
  const lastDuration = last10.length > 0 ? last10[last10.length - 1].duration : 0;

  const hasData = runs.length > 0;

  return (
    <div className="glass grid grid-cols-[1fr_1fr] gap-3 border-border/50 p-3">
      {/* Duration chart */}
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <TrendingUp className="h-3 w-3 text-primary" />
            <span className="text-[11px] font-medium text-foreground">Duração</span>
          </div>
          <span className="text-[10px] text-muted-foreground">
            avg <span className="font-mono font-medium text-foreground">{avgDuration}ms</span>
          </span>
        </div>
        <div className="h-10 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={durationData} margin={{ top: 2, right: 2, bottom: 0, left: 2 }}>
              <defs>
                <linearGradient id="miniDurationGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="hsl(var(--primary))" stopOpacity={0.3} />
                  <stop offset="100%" stopColor="hsl(var(--primary))" stopOpacity={0.02} />
                </linearGradient>
              </defs>
              <Tooltip
                contentStyle={{ fontSize: 10, padding: "2px 6px", borderRadius: 6, background: "hsl(var(--popover))", border: "1px solid hsl(var(--border))", color: "hsl(var(--foreground))" }}
                labelFormatter={() => ""}
                formatter={(v: number) => [`${v}ms`, "Duração"]}
              />
              <Area
                type="monotone"
                dataKey="d"
                stroke="hsl(var(--primary))"
                fill="url(#miniDurationGrad)"
                strokeWidth={1.5}
                dot={false}
                activeDot={{ r: 2.5, strokeWidth: 0, fill: "hsl(var(--primary))" }}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
        <span className="text-[9px] text-muted-foreground">
          última: <span className="font-mono text-foreground">{lastDuration}ms</span>
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
            {successCount} ok
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-destructive" />
            {errorCount} erro
          </span>
        </div>
      </div>
    </div>
  );
}
