import { CheckCircle2, XCircle } from "lucide-react";
import { DotsLoader } from "@/components/DotsLoader";
import { formatDistanceToNow } from "date-fns";
import { ptBR, enUS } from "date-fns/locale";
import { useTranslation } from "react-i18next";
import type { ExecutionRun } from "@/lib/execution-store";

interface RunHistoryItemProps {
  run: ExecutionRun;
  isActive: boolean;
  onClick: () => void;
}

function parseRunDate(timestamp: string): Date | null {
  const parsed = new Date(timestamp);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

export function RunHistoryItem({ run, isActive, onClick }: RunHistoryItemProps) {
  const { t, i18n } = useTranslation();
  const dateLocale = i18n.language === "pt-BR" ? ptBR : enUS;
  const localeStr = i18n.language === "pt-BR" ? "pt-BR" : "en-US";
  const runDate = parseRunDate(run.timestamp);
  const relativeLabel = runDate
    ? formatDistanceToNow(runDate, { addSuffix: true, locale: dateLocale })
    : t("common.invalidDate");
  const absoluteLabel = runDate
    ? `${runDate.toLocaleDateString(localeStr)} ${t("runHistory.at")} ${runDate.toLocaleTimeString(localeStr, { hour: "2-digit", minute: "2-digit", second: "2-digit" })}`
    : t("common.invalidTimestamp");

  return (
    <button
      onClick={onClick}
      className={`flex flex-col gap-0.5 px-2.5 py-2 text-[11px] font-medium transition-all duration-150 w-full text-left active:scale-[0.98] border-border/20 ${
        isActive
          ? run.status === "running"
            ? "bg-primary/15 text-primary shadow-ring-primary "
            : run.status === "success"
              ? "bg-success/15 text-success shadow-ring-success "
              : "bg-destructive/15 text-destructive shadow-ring-error "
          : run.status === "running"
            ? "bg-primary/10 text-primary"
            : "text-muted-foreground hover:bg-accent/40"
      }`}
    >
      <div className="flex items-center gap-1.5">
        {run.status === "running" ? (
          <DotsLoader className="text-primary" />
        ) : run.status === "success" ? (
          <CheckCircle2 className="h-3 w-3 text-success shrink-0" />
        ) : (
          <XCircle className="h-3 w-3 text-destructive shrink-0" />
        )}
        <span className="truncate">{run.status === "running" ? t("common.inProgress") : relativeLabel}</span>
      </div>
      <div className="flex flex-col pl-[18px] text-[10px] text-muted-foreground">
        <span>{absoluteLabel}</span>
        <span>{run.status === "running" ? "—" : `${run.duration}ms`}</span>
      </div>
    </button>
  );
}
