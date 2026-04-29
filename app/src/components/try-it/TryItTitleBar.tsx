import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ArrowLeft, X, Menu } from "lucide-react";
import { METHOD_COLORS } from "@/lib/constants";
import { useIsMobile } from "@/hooks/use-mobile";
import { statusColor } from "./helpers";
import type { OpenAPIRoute } from "@/types/pipeline";
import type { TryItResponse } from "./types";

interface TryItTitleBarProps {
  route: OpenAPIRoute;
  response: TryItResponse | null;
  hasRoutes: boolean;
  onClose: () => void;
  onOpenRoutes?: () => void;
}

export function TryItTitleBar({ route, response, hasRoutes, onClose, onOpenRoutes }: TryItTitleBarProps) {
  const { t } = useTranslation();
  const isMobile = useIsMobile();

  return (
    <div className="flex items-center justify-between px-4 py-2 border-border/50 shrink-0">
      <div className="flex items-center gap-2 min-w-0">
        {isMobile && hasRoutes && (
          <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0" onClick={onOpenRoutes}>
            <Menu className="h-4 w-4" />
          </Button>
        )}
        <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0" onClick={onClose}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <span className={`shrink-0 rounded-md px-2 py-0.5 text-xs font-bold ${METHOD_COLORS[route.method] || "bg-muted"}`}>
          {route.method}
        </span>
        <span className={`text-xs font-mono text-foreground/80 truncate ${isMobile ? "max-w-[120px]" : "max-w-[300px]"}`}>
          {route.path}
        </span>
        {!isMobile && route.summary && (
          <span className="text-xs text-muted-foreground truncate max-w-[200px]">
            — {route.summary}
          </span>
        )}
      </div>
      <div className="flex items-center gap-1">
        {response && (
          <Badge variant="outline" className={`text-[10px] ${statusColor(response.status)}`}>
            {response.status} · {response.duration}ms
          </Badge>
        )}
        <Button variant="ghost" size="icon" className="h-6 w-6" onClick={onClose}>
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}
