import { useState, useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { BarChart3, CalendarIcon, X, ArrowLeft, Zap } from "lucide-react";
import {
  LineChart, Line, PieChart, Pie, Cell, BarChart, Bar, AreaChart, Area,
  RadarChart, Radar, PolarGrid, PolarAngleAxis, PolarRadiusAxis,
  ScatterChart, Scatter, ZAxis,
  XAxis, YAxis, CartesianGrid, ResponsiveContainer,
} from "recharts";
import { format, subDays, subHours, startOfDay, endOfDay, isWithinInterval } from "date-fns";
import { ptBR, enUS } from "date-fns/locale";
import i18n from "@/i18n";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ChartContainer, ChartTooltip, ChartTooltipContent } from "@/components/ui/chart";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/EmptyState";
import { cn } from "@/lib/utils";
import { useExecutionHistoryStore } from "@/stores/useExecutionHistoryStore";
import { useLoadTestHistoryStore } from "@/stores/useLoadTestHistoryStore";
import type { ExecutionRun } from "@/lib/execution-store";
import type { LoadTestRunRecord } from "@/lib/load-test-store";
import * as apiClient from "@/lib/api-client";
import type { Pipeline } from "@/types/pipeline";
import {
  buildDurationHistory, getPipelineNames,
  buildSuccessRate, buildStepDurations, buildAssertionStats,
  buildStatusCodeDistribution, buildExecutionTimeline, buildPipelineComparison,
} from "@/lib/dashboard-data";
import {
  buildLatencyHistory, buildRpsHistory, buildLoadTestSuccessRate,
  buildLatencyDistribution, buildConfigComparison, buildThroughputVsLatency,
  buildLoadTestTimeline,
} from "@/lib/load-test-dashboard-data";

const CHART_COLORS = [
  "hsl(var(--primary))",
  "hsl(var(--chart-2, 173 58% 39%))",
  "hsl(var(--chart-3, 197 37% 24%))",
  "hsl(var(--chart-4, 43 74% 66%))",
  "hsl(var(--chart-5, 27 87% 67%))",
  "hsl(var(--chart-1, 12 76% 61%))",
];

interface DashboardPageProps {
  projectId: string;
  pipelines: Pipeline[];
  onBack?: () => void;
  executionBackendUrl?: string;
  initialPipelineId?: string;
}

type DatePreset = "all" | "1h" | "24h" | "7d" | "30d" | "custom";

function getPresetRange(preset: DatePreset): { from: Date; to: Date } | null {
  if (preset === "all") return null;
  const now = new Date();
  if (preset === "1h") return { from: subHours(now, 1), to: now };
  if (preset === "24h") return { from: subDays(now, 1), to: now };
  if (preset === "7d") return { from: subDays(now, 7), to: now };
  if (preset === "30d") return { from: subDays(now, 30), to: now };
  return null;
}

export default function DashboardPage({ projectId, pipelines, onBack, executionBackendUrl, initialPipelineId }: DashboardPageProps) {
  const { t } = useTranslation();
  const dateFnsLocale = i18n.language === "pt-BR" ? ptBR : enUS;

  const DATE_PRESETS: { value: DatePreset; label: string }[] = [
    { value: "all", label: t("dashboard.datePresets.all") },
    { value: "1h", label: "1h" },
    { value: "24h", label: "24h" },
    { value: "7d", label: "7d" },
    { value: "30d", label: "30d" },
    { value: "custom", label: "Custom" },
  ];

  const executionHistoryStore = useExecutionHistoryStore();
  const loadTestHistoryStore = useLoadTestHistoryStore();
  const [runs, setRuns] = useState<ExecutionRun[]>([]);
  const [ltRuns, setLtRuns] = useState<LoadTestRunRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedPipeline, setSelectedPipeline] = useState<string>("0");
  const [datePreset, setDatePreset] = useState<DatePreset>("all");
  const [customFrom, setCustomFrom] = useState<Date | undefined>();
  const [customTo, setCustomTo] = useState<Date | undefined>();

  useEffect(() => {
    const loadData = async () => {
      try {
        if (executionBackendUrl) {
          const [apiIntegration, apiLoad] = await Promise.all([
            apiClient.listIntegrationHistory(executionBackendUrl, projectId, { limit: 500 }),
            apiClient.listLoadHistory(executionBackendUrl, projectId, { limit: 500 }),
          ]);
          setRuns(apiIntegration.map(apiClient.integrationRecordToRun));
          setLtRuns(apiLoad.map(apiClient.loadRecordToRun));
        } else {
          const [r, lt] = await Promise.all([
            executionHistoryStore.loadLocalAllForProject(projectId),
            loadTestHistoryStore.loadLocalAllForProject(projectId),
          ]);
          setRuns(r);
          setLtRuns(lt);
        }
      } catch (err) {
        console.error("Failed to load dashboard data:", err);
        if (executionBackendUrl) {
          setRuns([]);
          setLtRuns([]);
        } else {
          const [r, lt] = await Promise.all([
            executionHistoryStore.loadLocalAllForProject(projectId),
            loadTestHistoryStore.loadLocalAllForProject(projectId),
          ]);
          setRuns(r);
          setLtRuns(lt);
        }
      } finally {
        setLoading(false);
      }
    };
    loadData();
  }, [projectId, executionBackendUrl]);

  useEffect(() => {
    if (!pipelines.length) {
      setSelectedPipeline("0");
      return;
    }

    if (!initialPipelineId) {
      return;
    }

    const pipelineIndex = pipelines.findIndex((pipeline) => pipeline.id === initialPipelineId);
    if (pipelineIndex >= 0) {
      setSelectedPipeline(String(pipelineIndex));
    }
  }, [initialPipelineId, pipelines]);

  const filteredRuns = useMemo(() => {
    if (datePreset === "all") return runs;
    let range: { from: Date; to: Date } | null = null;
    if (datePreset === "custom") {
      if (customFrom && customTo) range = { from: startOfDay(customFrom), to: endOfDay(customTo) };
      else if (customFrom) range = { from: startOfDay(customFrom), to: new Date() };
      else return runs;
    } else {
      range = getPresetRange(datePreset);
    }
    if (!range) return runs;
    return runs.filter((r) => {
      const d = new Date(r.timestamp);
      return isWithinInterval(d, { start: range!.from, end: range!.to });
    });
  }, [runs, datePreset, customFrom, customTo]);

  const filteredLtRuns = useMemo(() => {
    if (datePreset === "all") return ltRuns;
    let range: { from: Date; to: Date } | null = null;
    if (datePreset === "custom") {
      if (customFrom && customTo) range = { from: startOfDay(customFrom), to: endOfDay(customTo) };
      else if (customFrom) range = { from: startOfDay(customFrom), to: new Date() };
      else return ltRuns;
    } else {
      range = getPresetRange(datePreset);
    }
    if (!range) return ltRuns;
    return ltRuns.filter((r) => {
      const d = new Date(r.timestamp);
      return isWithinInterval(d, { start: range!.from, end: range!.to });
    });
  }, [ltRuns, datePreset, customFrom, customTo]);

  const pipelineNames = useMemo(() => getPipelineNames(filteredRuns), [filteredRuns]);
  const durationHistory = useMemo(() => buildDurationHistory(filteredRuns), [filteredRuns]);
  const successRate = useMemo(() => buildSuccessRate(filteredRuns), [filteredRuns]);
  const stepDurations = useMemo(() => buildStepDurations(filteredRuns, Number(selectedPipeline)), [filteredRuns, selectedPipeline]);
  const assertionStats = useMemo(() => buildAssertionStats(filteredRuns, Number(selectedPipeline)), [filteredRuns, selectedPipeline]);
  const statusCodes = useMemo(() => buildStatusCodeDistribution(filteredRuns), [filteredRuns]);
  const timeline = useMemo(() => buildExecutionTimeline(filteredRuns), [filteredRuns]);
  const comparison = useMemo(() => buildPipelineComparison(filteredRuns, pipelines), [filteredRuns, pipelines]);

  const ltLatencyHistory = useMemo(() => buildLatencyHistory(filteredLtRuns), [filteredLtRuns]);
  const ltRpsHistory = useMemo(() => buildRpsHistory(filteredLtRuns), [filteredLtRuns]);
  const ltSuccessRate = useMemo(() => buildLoadTestSuccessRate(filteredLtRuns), [filteredLtRuns]);
  const ltLatencyDist = useMemo(() => buildLatencyDistribution(filteredLtRuns), [filteredLtRuns]);
  const ltConfigComp = useMemo(() => buildConfigComparison(filteredLtRuns), [filteredLtRuns]);
  const ltThroughputLatency = useMemo(() => buildThroughputVsLatency(filteredLtRuns), [filteredLtRuns]);
  const ltTimeline = useMemo(() => buildLoadTestTimeline(filteredLtRuns), [filteredLtRuns]);

  if (loading) return null;

  if (!runs.length) {
    return (
      <EmptyState
        icon={BarChart3}
        title={t("dashboard.noData.title")}
        description={t("dashboard.noData.description")}
      />
    );
  }

  const successConfig = {
    success: { label: t("dashboard.success"), color: "hsl(142 71% 45%)" },
    error: { label: t("dashboard.error"), color: "hsl(0 84% 60%)" },
  };

  const durationConfig = Object.fromEntries(
    pipelineNames.map((name, i) => [name, { label: name, color: CHART_COLORS[i % CHART_COLORS.length] }])
  );

  const stepConfig = { avgDuration: { label: t("dashboard.avgDuration"), color: "hsl(var(--primary))" } };
  const assertConfig = { passed: { label: t("dashboard.passed"), color: "hsl(142 71% 45%)" }, failed: { label: t("dashboard.failed"), color: "hsl(0 84% 60%)" } };
  const statusConfig = { count: { label: t("dashboard.requests"), color: "hsl(var(--primary))" } };
  const timelineConfig = { executions: { label: t("dashboard.executions"), color: "hsl(var(--primary))" } };
  const radarConfig = Object.fromEntries(
    pipelines.map((p, i) => [p.name, { label: p.name, color: CHART_COLORS[i % CHART_COLORS.length] }])
  );

  return (
    <div className="h-full min-h-0 overflow-auto p-4 sm:p-6">
      <div className="mb-4 flex flex-col gap-3">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <h2 className="text-lg sm:text-xl font-bold flex items-center gap-2">
            {onBack && (
              <Button variant="ghost" size="icon" className="h-8 w-8" onClick={onBack}>
                <ArrowLeft className="h-4 w-4" />
              </Button>
            )}
            <BarChart3 className="h-5 w-5 text-primary" />
            {t("dashboard.title")}
          </h2>
          <Select value={selectedPipeline} onValueChange={setSelectedPipeline}>
            <SelectTrigger className="w-full sm:w-[220px]">
              <SelectValue placeholder={t("dashboard.selectPipeline")} />
            </SelectTrigger>
            <SelectContent>
              {pipelines.map((p, i) => (
                <SelectItem key={i} value={String(i)}>{p.name}</SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="flex items-center gap-1.5 sm:gap-2 flex-wrap">
          {DATE_PRESETS.map((p) => (
            <Button
              key={p.value}
              size="sm"
              variant={datePreset === p.value ? "default" : "outline"}
              onClick={() => setDatePreset(p.value)}
              className="h-7 text-xs rounded-full px-2.5 sm:px-3"
            >
              {p.label}
            </Button>
          ))}

          {datePreset === "custom" && (
            <div className="flex items-center gap-1.5 ml-1 sm:ml-2">
              <Popover>
                <PopoverTrigger asChild>
                  <Button variant="outline" size="sm" className={cn("h-7 text-xs gap-1", !customFrom && "text-muted-foreground")}>
                    <CalendarIcon className="h-3 w-3" />
                    {customFrom ? format(customFrom, "dd/MM/yy") : t("dashboard.from")}
                  </Button>
                </PopoverTrigger>
                <PopoverContent className="w-auto p-0" align="start">
                  <Calendar mode="single" selected={customFrom} onSelect={setCustomFrom} locale={dateFnsLocale} initialFocus className="p-3 pointer-events-auto" />
                </PopoverContent>
              </Popover>
              <span className="text-xs text-muted-foreground">→</span>
              <Popover>
                <PopoverTrigger asChild>
                  <Button variant="outline" size="sm" className={cn("h-7 text-xs gap-1", !customTo && "text-muted-foreground")}>
                    <CalendarIcon className="h-3 w-3" />
                    {customTo ? format(customTo, "dd/MM/yy") : t("dashboard.to")}
                  </Button>
                </PopoverTrigger>
                <PopoverContent className="w-auto p-0" align="start">
                  <Calendar mode="single" selected={customTo} onSelect={setCustomTo} locale={dateFnsLocale} initialFocus className="p-3 pointer-events-auto" />
                </PopoverContent>
              </Popover>
            </div>
          )}

          {datePreset !== "all" && (
            <Badge variant="secondary" className="ml-auto text-xs gap-1">
              {filteredRuns.length}/{runs.length}
              <X className="h-3 w-3 cursor-pointer" onClick={() => setDatePreset("all")} />
            </Badge>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        {/* 1. Duration History */}
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.durationHistory")}</CardTitle></CardHeader>
          <CardContent>
            <ChartContainer config={durationConfig} className="h-[200px] sm:h-[250px] w-full">
              <LineChart data={durationHistory}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="timestamp" fontSize={10} />
                <YAxis fontSize={10} />
                <ChartTooltip content={<ChartTooltipContent />} />
                {pipelineNames.map((name, i) => (
                  <Line key={name} type="monotone" dataKey={name} stroke={CHART_COLORS[i % CHART_COLORS.length]} strokeWidth={2} dot={{ r: 3 }} connectNulls />
                ))}
              </LineChart>
            </ChartContainer>
          </CardContent>
        </Card>

        {/* 2. Success Rate */}
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.successRate")}</CardTitle></CardHeader>
          <CardContent>
            <ChartContainer config={successConfig} className="h-[200px] sm:h-[250px] w-full">
              <PieChart>
                <Pie data={successRate} cx="50%" cy="50%" innerRadius={50} outerRadius={75} dataKey="value" nameKey="name" label={({ name, value }) => `${name}: ${value}`}>
                  <Cell fill="hsl(142 71% 45%)" /><Cell fill="hsl(0 84% 60%)" />
                </Pie>
                <ChartTooltip content={<ChartTooltipContent />} />
              </PieChart>
            </ChartContainer>
          </CardContent>
        </Card>

        {/* 3. Step Durations */}
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.stepDuration")}</CardTitle></CardHeader>
          <CardContent>
            {stepDurations.length ? (
              <ChartContainer config={stepConfig} className="h-[200px] sm:h-[250px] w-full">
                <BarChart data={stepDurations} layout="vertical">
                  <CartesianGrid strokeDasharray="3 3" />
                  <XAxis type="number" fontSize={10} />
                  <YAxis dataKey="step" type="category" fontSize={10} width={80} />
                  <ChartTooltip content={<ChartTooltipContent />} />
                  <Bar dataKey="avgDuration" fill="hsl(var(--primary))" radius={[0, 4, 4, 0]} />
                </BarChart>
              </ChartContainer>
            ) : (
              <p className="text-sm text-muted-foreground text-center py-8">{t("common.noData")}</p>
            )}
          </CardContent>
        </Card>

        {/* 4. Assertions */}
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.assertionsPerStep")}</CardTitle></CardHeader>
          <CardContent>
            {assertionStats.length ? (
              <ChartContainer config={assertConfig} className="h-[200px] sm:h-[250px] w-full">
                <BarChart data={assertionStats}>
                  <CartesianGrid strokeDasharray="3 3" />
                  <XAxis dataKey="step" fontSize={10} />
                  <YAxis fontSize={10} />
                  <ChartTooltip content={<ChartTooltipContent />} />
                  <Bar dataKey="passed" stackId="a" fill="hsl(142 71% 45%)" radius={[0, 0, 0, 0]} />
                  <Bar dataKey="failed" stackId="a" fill="hsl(0 84% 60%)" radius={[4, 4, 0, 0]} />
                </BarChart>
              </ChartContainer>
            ) : (
              <p className="text-sm text-muted-foreground text-center py-8">{t("dashboard.noAssertions")}</p>
            )}
          </CardContent>
        </Card>

        {/* 5. Status Codes */}
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.statusCodes")}</CardTitle></CardHeader>
          <CardContent>
            <ChartContainer config={statusConfig} className="h-[200px] sm:h-[250px] w-full">
              <BarChart data={statusCodes}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="code" fontSize={10} />
                <YAxis fontSize={10} />
                <ChartTooltip content={<ChartTooltipContent />} />
                <Bar dataKey="count" radius={[4, 4, 0, 0]}>
                  {statusCodes.map((entry, i) => (<Cell key={i} fill={entry.fill} />))}
                </Bar>
              </BarChart>
            </ChartContainer>
          </CardContent>
        </Card>

        {/* 6. Timeline */}
        <Card>
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.executionTimeline")}</CardTitle></CardHeader>
          <CardContent>
            <ChartContainer config={timelineConfig} className="h-[200px] sm:h-[250px] w-full">
              <AreaChart data={timeline}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="date" fontSize={10} />
                <YAxis fontSize={10} />
                <ChartTooltip content={<ChartTooltipContent />} />
                <Area type="monotone" dataKey="executions" stroke="hsl(var(--primary))" fill="hsl(var(--primary))" fillOpacity={0.2} />
              </AreaChart>
            </ChartContainer>
          </CardContent>
        </Card>

        {/* 7. Pipeline Comparison */}
        <Card className="md:col-span-2">
          <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.pipelineComparison")}</CardTitle></CardHeader>
          <CardContent>
            {comparison.length ? (
              <ChartContainer config={radarConfig} className="h-[250px] sm:h-[300px] w-full">
                <RadarChart data={comparison} cx="50%" cy="50%" outerRadius="70%">
                  <PolarGrid />
                  <PolarAngleAxis dataKey="metric" fontSize={11} />
                  <PolarRadiusAxis domain={[0, 100]} tick={false} />
                  <ChartTooltip content={<ChartTooltipContent />} />
                  {pipelines.map((p, i) => (
                    <Radar key={p.name} name={p.name} dataKey={p.name} stroke={CHART_COLORS[i % CHART_COLORS.length]} fill={CHART_COLORS[i % CHART_COLORS.length]} fillOpacity={0.15} />
                  ))}
                </RadarChart>
              </ChartContainer>
            ) : (
              <p className="text-sm text-muted-foreground text-center py-8">{t("dashboard.addPipelines")}</p>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Load Test Section */}
      {filteredLtRuns.length > 0 && (
        <>
          <h3 className="text-base font-semibold flex items-center gap-2 mt-8 mb-4">
            <Zap className="h-4 w-4 text-primary" />
            Load Test
            <Badge variant="secondary" className="text-xs">{filteredLtRuns.length} runs</Badge>
          </h3>
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.latencyOverTime")}</CardTitle></CardHeader>
              <CardContent>
                <ChartContainer config={{ avg: { label: "Avg", color: CHART_COLORS[0] }, p95: { label: "P95", color: CHART_COLORS[1] }, p99: { label: "P99", color: CHART_COLORS[4] } }} className="h-[200px] sm:h-[250px] w-full">
                  <LineChart data={ltLatencyHistory}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="timestamp" fontSize={10} />
                    <YAxis fontSize={10} unit="ms" />
                    <ChartTooltip content={<ChartTooltipContent />} />
                    <Line type="monotone" dataKey="avg" stroke={CHART_COLORS[0]} strokeWidth={2} dot={{ r: 3 }} />
                    <Line type="monotone" dataKey="p95" stroke={CHART_COLORS[1]} strokeWidth={2} dot={{ r: 3 }} />
                    <Line type="monotone" dataKey="p99" stroke={CHART_COLORS[4]} strokeWidth={2} dot={{ r: 3 }} />
                  </LineChart>
                </ChartContainer>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">RPS (Requests/s)</CardTitle></CardHeader>
              <CardContent>
                <ChartContainer config={{ rps: { label: "RPS", color: CHART_COLORS[0] } }} className="h-[200px] sm:h-[250px] w-full">
                  <AreaChart data={ltRpsHistory}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="timestamp" fontSize={10} />
                    <YAxis fontSize={10} />
                    <ChartTooltip content={<ChartTooltipContent />} />
                    <Area type="monotone" dataKey="rps" stroke={CHART_COLORS[0]} fill={CHART_COLORS[0]} fillOpacity={0.2} />
                  </AreaChart>
                </ChartContainer>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.successRateLoadTest")}</CardTitle></CardHeader>
              <CardContent>
                <ChartContainer config={{ success: { label: t("dashboard.success"), color: "hsl(142 71% 45%)" }, error: { label: t("dashboard.error"), color: "hsl(0 84% 60%)" } }} className="h-[200px] sm:h-[250px] w-full">
                  <PieChart>
                    <Pie data={ltSuccessRate} cx="50%" cy="50%" innerRadius={50} outerRadius={75} dataKey="value" nameKey="name" label={({ name, value }) => `${name}: ${value}`}>
                      <Cell fill="hsl(142 71% 45%)" /><Cell fill="hsl(0 84% 60%)" />
                    </Pie>
                    <ChartTooltip content={<ChartTooltipContent />} />
                  </PieChart>
                </ChartContainer>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.latencyDistribution")}</CardTitle></CardHeader>
              <CardContent>
                <ChartContainer config={{ count: { label: t("dashboard.requests"), color: CHART_COLORS[3] } }} className="h-[200px] sm:h-[250px] w-full">
                  <BarChart data={ltLatencyDist}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="bucket" fontSize={10} />
                    <YAxis fontSize={10} />
                    <ChartTooltip content={<ChartTooltipContent />} />
                    <Bar dataKey="count" fill={CHART_COLORS[3]} radius={[4, 4, 0, 0]} />
                  </BarChart>
                </ChartContainer>
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.configComparison")}</CardTitle></CardHeader>
              <CardContent>
                {ltConfigComp.length ? (
                  <ChartContainer config={{ avgLatency: { label: "Avg Latency", color: CHART_COLORS[0] }, p95: { label: "P95", color: CHART_COLORS[1] }, rps: { label: "RPS", color: CHART_COLORS[2] } }} className="h-[200px] sm:h-[250px] w-full">
                    <BarChart data={ltConfigComp}>
                      <CartesianGrid strokeDasharray="3 3" />
                      <XAxis dataKey="config" fontSize={10} />
                      <YAxis fontSize={10} />
                      <ChartTooltip content={<ChartTooltipContent />} />
                      <Bar dataKey="avgLatency" fill={CHART_COLORS[0]} radius={[4, 4, 0, 0]} />
                      <Bar dataKey="p95" fill={CHART_COLORS[1]} radius={[4, 4, 0, 0]} />
                      <Bar dataKey="rps" fill={CHART_COLORS[2]} radius={[4, 4, 0, 0]} />
                    </BarChart>
                  </ChartContainer>
                ) : (
                  <p className="text-sm text-muted-foreground text-center py-8">{t("dashboard.noConfigData")}</p>
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.throughputVsLatency")}</CardTitle></CardHeader>
              <CardContent>
                <ChartContainer config={{ scatter: { label: "Runs", color: CHART_COLORS[0] } }} className="h-[200px] sm:h-[250px] w-full">
                  <ScatterChart>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="rps" name="RPS" fontSize={10} unit=" req/s" />
                    <YAxis dataKey="avgLatency" name="Latency" fontSize={10} unit="ms" />
                    <ZAxis range={[40, 200]} />
                    <ChartTooltip content={<ChartTooltipContent />} />
                    <Scatter data={ltThroughputLatency} fill={CHART_COLORS[0]} />
                  </ScatterChart>
                </ChartContainer>
              </CardContent>
            </Card>

            <Card className="md:col-span-2">
              <CardHeader className="pb-2"><CardTitle className="text-sm font-medium">{t("dashboard.loadTestTimeline")}</CardTitle></CardHeader>
              <CardContent>
                <ChartContainer config={{ tests: { label: t("dashboard.tests"), color: CHART_COLORS[2] } }} className="h-[200px] sm:h-[250px] w-full">
                  <AreaChart data={ltTimeline}>
                    <CartesianGrid strokeDasharray="3 3" />
                    <XAxis dataKey="date" fontSize={10} />
                    <YAxis fontSize={10} />
                    <ChartTooltip content={<ChartTooltipContent />} />
                    <Area type="monotone" dataKey="tests" stroke={CHART_COLORS[2]} fill={CHART_COLORS[2]} fillOpacity={0.2} />
                  </AreaChart>
                </ChartContainer>
              </CardContent>
            </Card>
          </div>
        </>
      )}
    </div>
  );
}
