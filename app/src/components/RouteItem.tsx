import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Button } from "@/components/ui/button";
import { ChevronRight, Play } from "lucide-react";
import { METHOD_COLORS, PARAM_TYPE_COLORS } from "@/lib/constants";
import type { OpenAPIRoute } from "@/types/pipeline";

interface RouteItemProps {
  route: OpenAPIRoute;
  onTryIt?: (route: OpenAPIRoute) => void;
}

export function RouteItem({ route, onTryIt }: RouteItemProps) {
  const [isOpen, setIsOpen] = useState(false);

  const hasDetails = route.parameters?.length || route.requestBody || route.responses?.length;

  return (
    <Collapsible open={isOpen} onOpenChange={setIsOpen}>
      <CollapsibleTrigger asChild>
        <div className="flex items-start gap-3 rounded-md px-3 py-2 text-sm hover:bg-accent cursor-pointer group">
          <ChevronRight
            className={`mt-0.5 h-4 w-4 shrink-0 text-muted-foreground transition-transform ${isOpen ? "rotate-90" : ""} ${!hasDetails ? "invisible" : ""}`}
          />
          <span className={`shrink-0 rounded-md px-2 py-0.5 text-xs font-bold ${METHOD_COLORS[route.method] || "bg-muted"}`}>
            {route.method}
          </span>
          <div className="min-w-0 flex-1">
            <span className="block truncate font-mono text-xs">{route.path}</span>
            {(route.summary || route.description) && (
              <p className="mt-0.5 truncate text-xs text-muted-foreground">
                {route.summary || route.description}
              </p>
            )}
          </div>
          {onTryIt && (
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-2 gap-1 text-xs opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
              onClick={(e) => { e.stopPropagation(); onTryIt(route); }}
            >
              <Play className="h-3 w-3" /> Try it
            </Button>
          )}
        </div>
      </CollapsibleTrigger>

      {hasDetails && (
        <CollapsibleContent>
          <div className="ml-10 mr-3 mb-2 space-y-3 rounded-md border p-3 text-xs">
            {route.parameters && route.parameters.length > 0 && (
              <div>
                <h4 className="font-semibold text-muted-foreground mb-1.5">Parameters</h4>
                <div className="space-y-1">
                  {route.parameters.map((param, i) => (
                    <div key={i} className="flex items-center gap-2">
                      <Badge variant="outline" className={`text-[10px] px-1.5 py-0 ${PARAM_TYPE_COLORS[param.in] || ""}`}>
                        {param.in}
                      </Badge>
                      <code className="font-mono text-foreground">{param.name}</code>
                      {param.required && <span className="text-destructive">*</span>}
                      {param.schema?.type && (
                        <span className="text-muted-foreground">({String(param.schema.type)})</span>
                      )}
                      {param.description && (
                        <span className="text-muted-foreground truncate">— {param.description}</span>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {route.requestBody && (
              <div>
                <h4 className="font-semibold text-muted-foreground mb-1.5">
                  Request Body
                  {route.requestBody.required && <span className="text-destructive ml-1">*</span>}
                </h4>
                {route.requestBody.content && (
                  <div className="space-y-1">
                    {Object.entries(route.requestBody.content).map(([contentType, content]) => (
                      <div key={contentType}>
                        <code className="text-muted-foreground">{contentType}</code>
                        {content.schema && (
                          <pre className="mt-1 p-2 rounded-md bg-background text-[10px] overflow-auto max-h-24">
                            {JSON.stringify(content.schema, null, 2)}
                          </pre>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {route.responses && route.responses.length > 0 && (
              <div>
                <h4 className="font-semibold text-muted-foreground mb-1.5">Responses</h4>
                <div className="space-y-1">
                  {route.responses.map((response, i) => (
                    <div key={i} className="flex items-center gap-2">
                      <Badge
                        variant="outline"
                        className={`text-[10px] px-1.5 py-0 ${
                          response.statusCode.startsWith("2")
                            ? "bg-success/15 text-success"
                            : response.statusCode.startsWith("4")
                              ? "bg-warning/15 text-warning"
                              : response.statusCode.startsWith("5")
                                ? "bg-destructive/15 text-destructive"
                                : ""
                        }`}
                      >
                        {response.statusCode}
                      </Badge>
                      {response.description && (
                        <span className="text-muted-foreground">{response.description}</span>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </CollapsibleContent>
      )}
    </Collapsible>
  );
}
