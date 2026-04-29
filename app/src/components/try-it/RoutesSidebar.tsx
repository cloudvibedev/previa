import { ScrollArea } from "@/components/ui/scroll-area";
import { METHOD_COLORS } from "@/lib/constants";
import type { OpenAPIRoute } from "@/types/pipeline";

interface RoutesSidebarProps {
  allRoutes: OpenAPIRoute[];
  route: OpenAPIRoute;
  onSelectRoute?: (route: OpenAPIRoute) => void;
}

export function RoutesSidebar({ allRoutes, route, onSelectRoute }: RoutesSidebarProps) {
  return (
    <ScrollArea className="flex-1">
      <div className="p-1 space-y-0.5">
        {allRoutes.map((r, i) => {
          const isActive = r.method === route.method && r.path === route.path;
          return (
            <button
              key={`${r.method}-${r.path}-${i}`}
              onClick={() => onSelectRoute?.(r)}
              className={`w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-left transition-colors hover:bg-accent/50 ${isActive ? "bg-accent" : ""}`}
            >
              <span className={`shrink-0 rounded px-1.5 py-0.5 text-[10px] font-bold leading-none ${METHOD_COLORS[r.method] || "bg-muted"}`}>
                {r.method}
              </span>
              <span className="text-xs font-mono text-foreground/80 truncate">{r.path}</span>
            </button>
          );
        })}
      </div>
    </ScrollArea>
  );
}
