import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { CheckCircle2, AlertCircle, ChevronUp, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";
import type { FormatType } from "@/lib/pipeline-schema";

export interface ValidationFooterError {
  message: string;
  line: number;
  column: number;
}

export interface ValidationFooterProps {
  isValid: boolean | null;
  errors: ValidationFooterError[];
  warnings?: ValidationFooterError[];
  format: FormatType;
  onErrorClick?: (error: ValidationFooterError) => void;
  validLabel?: string;
  pendingLabel?: string;
  className?: string;
}

export function ValidationFooter({
  isValid,
  errors,
  warnings = [],
  format,
  onErrorClick,
  validLabel,
  pendingLabel,
  className,
}: ValidationFooterProps) {
  const { t } = useTranslation();
  const resolvedValidLabel = validLabel ?? t("validation.valid");
  const [expanded, setExpanded] = useState(false);
  const [isClosing, setIsClosing] = useState(false);

  useEffect(() => {
    if (errors.length === 0 && warnings.length === 0) {
      setExpanded(false);
    }
  }, [errors.length, warnings.length]);

  const handleToggle = () => {
    if (expanded) {
      setIsClosing(true);
      setTimeout(() => {
        setExpanded(false);
        setIsClosing(false);
      }, 150);
    } else {
      setExpanded(true);
    }
  };

  const handleErrorClick = (error: ValidationFooterError) => {
    onErrorClick?.(error);
  };

  const defaultPendingLabel = t("validation.pending", { format: format.toUpperCase() });
  const pendingText = pendingLabel ?? defaultPendingLabel;

  const hasIssues = errors.length > 0 || warnings.length > 0;
  const showWarningStatus = isValid === true && warnings.length > 0;

  return (
    <div className={cn("relative", className)}>
      {expanded && hasIssues && (
        <div
          className={cn(
            "glass absolute bottom-full left-0 right-0 max-h-40 overflow-y-auto border-border/50 text-xs",
            errors.length > 0
              ? "bg-destructive/90 text-destructive-foreground"
              : "bg-warning/90 text-warning-foreground",
            isClosing ? "animate-slide-down" : "animate-slide-up"
          )}
        >
          {errors.map((error, i) => (
            <div
              key={`e-${i}`}
              className="flex items-start gap-2 px-3 py-1.5 hover:bg-background/20 cursor-pointer border-background/20 last:border-b-0"
              onClick={() => handleErrorClick(error)}
            >
              <span className="shrink-0 font-mono text-[10px] opacity-70">
                L{error.line}:{error.column}
              </span>
              <span className="break-words">{error.message}</span>
            </div>
          ))}
          {warnings.map((warning, i) => (
            <div
              key={`w-${i}`}
              className={cn(
                "flex items-start gap-2 px-3 py-1.5 hover:bg-background/20 cursor-pointer border-background/20 last:border-b-0",
                errors.length > 0 && "bg-warning/20"
              )}
              onClick={() => handleErrorClick(warning)}
            >
              <span className="shrink-0 font-mono text-[10px] opacity-70">
                L{warning.line}:{warning.column}
              </span>
              <span className="break-words">⚠ {warning.message}</span>
            </div>
          ))}
        </div>
      )}

      <div
        className={cn(
          "flex items-center justify-between px-3 py-1.5 text-xs border-border/50",
          isValid === true && warnings.length === 0 && "bg-success/10 text-success",
          showWarningStatus && "bg-warning/10 text-warning",
          isValid === false && "bg-destructive/10 text-destructive cursor-pointer",
          isValid === null && "bg-muted text-muted-foreground"
        )}
        onClick={isValid === false && hasIssues ? handleToggle : showWarningStatus ? handleToggle : undefined}
      >
        <div className="flex items-center gap-2">
          {isValid === true && warnings.length === 0 && <CheckCircle2 className="h-3.5 w-3.5" />}
          {showWarningStatus && <AlertCircle className="h-3.5 w-3.5" />}
          {isValid === false && <AlertCircle className="h-3.5 w-3.5" />}
          
          {isValid === true && warnings.length === 0 && <span>{resolvedValidLabel}</span>}
          {showWarningStatus && (
            <span>
              {resolvedValidLabel} · {warnings.length} {warnings.length === 1 ? "warning" : "warnings"}
            </span>
          )}
          {isValid === false && (
            <span>
              {t(errors.length === 1 ? "validation.errors" : "validation.errors_plural", { count: errors.length })}
              {warnings.length > 0 && ` · ${warnings.length} ${warnings.length === 1 ? "warning" : "warnings"}`}
            </span>
          )}
          {isValid === null && <span>{pendingText}</span>}
        </div>

        {hasIssues && (isValid === false || showWarningStatus) && (
          <button
            type="button"
            className="flex items-center gap-1 hover:underline"
            onClick={(e) => {
              e.stopPropagation();
              handleToggle();
            }}
          >
            {expanded ? (
              <>
                {t("common.hide")} <ChevronDown className="h-3 w-3" />
              </>
            ) : (
              <>
                {t("common.showDetails")} <ChevronUp className="h-3 w-3" />
              </>
            )}
          </button>
        )}
      </div>
    </div>
  );
}
