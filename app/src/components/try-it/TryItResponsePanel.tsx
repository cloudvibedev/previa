import { lazy, Suspense, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from "@/components/ui/resizable";
import { Copy, Check } from "lucide-react";
import { DotsLoader } from "@/components/DotsLoader";
import { statusColor, formatBody } from "./helpers";
import type { TryItResponse } from "./types";

const MonacoCodeEditor = lazy(() => import("@/components/MonacoCodeEditor"));

interface TryItResponsePanelProps {
  response: TryItResponse | null;
  loading: boolean;
  error: string | null;
  isDark: boolean;
}

export function TryItResponsePanel({ response, loading, error, isDark }: TryItResponsePanelProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);

  const copyResponse = useCallback(() => {
    if (!response) return;
    navigator.clipboard.writeText(formatBody(response.body));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [response]);

  return (
    <>
      <div className="flex items-center justify-between px-3 py-2 border-border/30">
        <span className="text-xs font-medium text-muted-foreground">{t("tryIt.response")}</span>
        {response && (
          <div className="flex items-center gap-2">
            <Badge variant="outline" className={`text-[10px] ${statusColor(response.status)}`}>
              {response.status} {response.statusText}
            </Badge>
            <span className="text-[10px] text-muted-foreground">{response.duration}ms</span>
            <Button variant="ghost" size="icon" className="h-6 w-6" onClick={copyResponse}>
              {copied ? <Check className="h-3 w-3 text-success" /> : <Copy className="h-3 w-3" />}
            </Button>
          </div>
        )}
      </div>

      {!response && !error && !loading && (
        <div className="flex-1 flex items-center justify-center">
          <p className="text-xs text-muted-foreground">{t("tryIt.sendHint")}</p>
        </div>
      )}

      {loading && (
        <div className="flex-1 flex items-center justify-center gap-2">
          <DotsLoader />
          <span className="text-xs text-muted-foreground">{t("tryIt.sending")}</span>
        </div>
      )}

      {error && (
        <div className="flex-1 flex items-center justify-center p-4">
          <p className="text-xs text-destructive">{error}</p>
        </div>
      )}

      {response && (
        <ResizablePanelGroup direction="vertical" className="flex-1 min-h-0" autoSaveId="tryit-response">
          <ResizablePanel defaultSize={30} minSize={10}>
            <ScrollArea className="h-full">
              <div className="px-3 py-2">
                <span className="text-[10px] font-semibold uppercase text-muted-foreground tracking-wider">{t("tryIt.headers")}</span>
                <div className="mt-1.5 space-y-1">
                  {Object.entries(response.headers).map(([k, v]) => (
                    <div key={k} className="flex gap-2 text-xs">
                      <span className="font-mono font-medium text-foreground/80">{k}:</span>
                      <span className="font-mono text-muted-foreground break-all">{v}</span>
                    </div>
                  ))}
                </div>
              </div>
            </ScrollArea>
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel defaultSize={70} minSize={20}>
            <div className="flex flex-col h-full min-h-0 px-3 pb-2">
              <span className="text-[10px] font-semibold uppercase text-muted-foreground tracking-wider mb-1.5">{t("tryIt.body")}</span>
              <div className="flex-1 min-h-0 rounded-md overflow-hidden border">
                <Suspense fallback={<pre className="text-xs font-mono p-2 rounded-md whitespace-pre-wrap break-all">{formatBody(response.body)}</pre>}>
                  <MonacoCodeEditor
                    className="h-full"
                    value={formatBody(response.body)}
                    readOnly
                    height="100%"
                    showHeader={false}
                    showValidation={false}
                    showLineNumbers={true}
                    isDark={isDark}
                  />
                </Suspense>
              </div>
            </div>
          </ResizablePanel>
        </ResizablePanelGroup>
      )}
    </>
  );
}
