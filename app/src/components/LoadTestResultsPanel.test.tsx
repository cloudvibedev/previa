import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LoadTestResultsPanel } from "@/components/LoadTestResultsPanel";
import type { LoadTestMetrics, WaveLoadConfig } from "@/types/load-test";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, number>) => {
      if (key === "loadTestResults.elapsedLabel") return "TIME";
      return key;
    },
  }),
}));

describe("LoadTestResultsPanel", () => {
  const emptyMetrics: LoadTestMetrics = {
    totalSent: 0,
    totalSuccess: 0,
    totalError: 0,
    avgLatency: 0,
    p95: 0,
    p99: 0,
    rps: 0,
    latencyHistory: [],
    rpsHistory: [],
    runnerResourceHistory: [],
    startTime: 1_000,
    elapsedMs: 0,
  };

  it("shows runner resource charts when a single runtime sample exists", () => {
    const metrics: LoadTestMetrics = {
      totalSent: 1,
      totalSuccess: 1,
      totalError: 0,
      avgLatency: 0,
      p95: 0,
      p99: 0,
      rps: 1,
      latencyHistory: [],
      rpsHistory: [],
      runnerResourceHistory: [
        {
          node: "runner-a",
          timestamp: 1_000,
          elapsedMs: 250,
          cpuUsagePercent: 12.5,
          memoryBytes: 104_857_600,
          memoryMb: 100,
          networkTxBytes: 2_048,
          networkRxBytes: 4_096,
          networkTotalBytes: 6_144,
          networkTotalKb: 6,
        },
      ],
      startTime: 750,
      elapsedMs: 250,
    };

    render(<LoadTestResultsPanel metrics={metrics} state="running" totalRequests={10} />);

    expect(screen.getByText("Runner CPU")).toBeInTheDocument();
    expect(screen.getByText("Runner memory")).toBeInTheDocument();
    expect(screen.getByText("Runner network")).toBeInTheDocument();
    expect(screen.getAllByText("runner-a").length).toBeGreaterThan(0);
  });

  it("shows elapsed time as a metric card instead of loose footer text", () => {
    const metrics: LoadTestMetrics = {
      totalSent: 10,
      totalSuccess: 10,
      totalError: 0,
      avgLatency: 100,
      p95: 150,
      p99: 200,
      rps: 2,
      latencyHistory: [],
      rpsHistory: [],
      runnerResourceHistory: [],
      startTime: 1_000,
      elapsedMs: 1_500,
    };

    render(<LoadTestResultsPanel metrics={metrics} state="completed" totalRequests={10} />);

    expect(screen.getByText("2s")).toBeInTheDocument();
    expect(screen.getByText("TIME")).toBeInTheDocument();
    expect(screen.queryByText(/elapsed/i)).not.toBeInTheDocument();
  });

  it("shows the configured wave profile on wave load results", () => {
    const config: WaveLoadConfig = {
      points: [
        { atMs: 0, intensity: 10 },
        { atMs: 30_000, intensity: 80 },
        { atMs: 60_000, intensity: 25 },
      ],
      interpolation: "smooth",
      maxInFlight: 200,
      gracePeriodMs: 30_000,
    };

    render(
      <LoadTestResultsPanel
        metrics={emptyMetrics}
        state="completed"
        totalRequests={0}
        config={config}
      />,
    );

    expect(screen.getByText("loadTestResults.configuredWave")).toBeInTheDocument();
    expect(screen.getByTestId("configured-wave-chart")).toBeInTheDocument();
    expect(screen.getByText("10%")).toBeInTheDocument();
    expect(screen.getByText("80%")).toBeInTheDocument();
    expect(screen.getByText("25%")).toBeInTheDocument();
  });
});
