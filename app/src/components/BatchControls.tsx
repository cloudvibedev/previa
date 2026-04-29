import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { Square, Loader2, CheckCircle2, XCircle, Clock } from "lucide-react";
import type { E2eQueuePipelineRecord } from "@/lib/api-client";
import { cn } from "@/lib/utils";

interface BatchControlsProps {
  batchState: "running" | "paused";
  progress: number;
  total: number;
  onPause: () => void;
  onResume: () => void;
  onCancel: () => void;
  /** Pipeline statuses from the server queue (optional — shows detailed status when available) */
  queuePipelines?: E2eQueuePipelineRecord[];
  /** Map pipeline IDs to names for display */
  pipelineNames?: Record<string, string>;
}

function PipelineStatusIcon({ status }: { status: string }) {
  switch (status) {
    case "running":
      return <Loader2 className="h-3 w-3 animate-spin text-primary" />;
    case "completed":
      return <CheckCircle2 className="h-3 w-3 text-emerald-500" />;
    case "failed":
      return <XCircle className="h-3 w-3 text-destructive" />;
    case "cancelled":
      return <Square className="h-3 w-3 text-muted-foreground" />;
    default:
      return <Clock className="h-3 w-3 text-muted-foreground" />;
  }
}

export function BatchControls({ batchState, progress, total, onCancel, queuePipelines, pipelineNames }: BatchControlsProps) {
  const { t } = useTranslation();
  const progressPercent = total > 0 ? (progress / total) * 100 : 0;

  return (
    <div className="mt-2 space-y-2">
      <div className="flex items-center gap-2">
        <Progress value={progressPercent} className="h-2 flex-1" />
        <span className="text-[10px] font-medium text-muted-foreground whitespace-nowrap">
          {progress}/{total}
        </span>
      </div>

      {queuePipelines && queuePipelines.length > 0 && (
        <div className="space-y-0.5 max-h-24 overflow-y-auto">
          {queuePipelines.map((p) => (
            <div
              key={p.id}
              className={cn(
                "flex items-center gap-1.5 text-[10px] px-1 py-0.5 rounded",
                p.status === "running" && "bg-primary/5",
                p.status === "failed" && "bg-destructive/5",
              )}
            >
              <PipelineStatusIcon status={p.status} />
              <span className="truncate flex-1 text-muted-foreground">
                {pipelineNames?.[p.id] || p.id.slice(0, 8)}
              </span>
            </div>
          ))}
        </div>
      )}

      <div className="flex gap-1">
        <Button variant="outline" size="sm" className="h-6 text-[10px] flex-1" onClick={onCancel}>
          <Square className="h-3 w-3" /> {t("common.cancel")}
        </Button>
      </div>
    </div>
  );
}
