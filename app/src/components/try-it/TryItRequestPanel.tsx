import { lazy, Suspense } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from "@/components/ui/resizable";
import { useIsMobile } from "@/hooks/use-mobile";
import { Play } from "lucide-react";
import { DotsLoader } from "@/components/DotsLoader";
import { KeyValueEditor } from "./KeyValueEditor";
import type { KeyValue } from "./types";

const MonacoCodeEditor = lazy(() => import("@/components/MonacoCodeEditor"));

interface TryItRequestPanelProps {
  serverKeys: string[];
  servers: Record<string, string>;
  selectedServer: string;
  onSelectedServerChange: (v: string) => void;
  customBaseUrl: string;
  onCustomBaseUrlChange: (v: string) => void;
  resolvedUrl: string;
  loading: boolean;
  baseUrl: string;
  onSend: () => void;
  pathParams: KeyValue[];
  onPathParamsChange: (v: KeyValue[]) => void;
  queryParams: KeyValue[];
  onQueryParamsChange: (v: KeyValue[]) => void;
  headers: KeyValue[];
  onHeadersChange: (v: KeyValue[]) => void;
  hasBody: boolean;
  body: string;
  onBodyChange: (v: string) => void;
  isDark: boolean;
}

export function TryItRequestPanel({
  serverKeys, servers, selectedServer, onSelectedServerChange,
  customBaseUrl, onCustomBaseUrlChange, resolvedUrl,
  loading, baseUrl, onSend,
  pathParams, onPathParamsChange,
  queryParams, onQueryParamsChange,
  headers, onHeadersChange,
  hasBody, body, onBodyChange, isDark,
}: TryItRequestPanelProps) {
  const { t } = useTranslation();
  const isMobile = useIsMobile();

  const paramsSection = (
    <>
      {pathParams.length > 0 && (
        <KeyValueEditor
          label={t("tryIt.pathParams")}
          items={pathParams}
          onChange={onPathParamsChange}
          keyReadOnly
        />
      )}
      {queryParams.length > 0 && (
        <KeyValueEditor
          label={t("tryIt.query")}
          items={queryParams}
          onChange={onQueryParamsChange}
          showAdd
          showDelete
        />
      )}
      <KeyValueEditor
        label={t("tryIt.headers")}
        items={headers}
        onChange={onHeadersChange}
        keyPlaceholder="Header"
        valuePlaceholder="Value"
        keyWidth={isMobile ? "w-24" : "w-36"}
        showAdd
        showDelete
      />
    </>
  );

  return (
    <>
      {/* URL bar */}
      <div className={`flex items-center gap-2 px-3 py-2 border-border/30 ${isMobile ? "flex-wrap" : ""}`}>
        {serverKeys.length > 0 ? (
          <select
            value={selectedServer}
            onChange={(e) => onSelectedServerChange(e.target.value)}
            className={`h-7 rounded-md border border-border bg-background px-2 text-xs font-mono ${isMobile ? "w-full" : ""}`}
          >
            {serverKeys.map((k) => (
              <option key={k} value={k}>{k}: {servers[k]}</option>
            ))}
          </select>
        ) : (
          <Input
            placeholder="http://localhost:3000"
            value={customBaseUrl}
            onChange={(e) => onCustomBaseUrlChange(e.target.value)}
            className={`h-7 text-xs font-mono ${isMobile ? "w-full" : "w-48"}`}
          />
        )}
        <Input value={resolvedUrl} readOnly className="flex-1 h-7 text-xs font-mono bg-muted/30" />
        <Button size="sm" className="h-7 gap-1.5 px-3" onClick={onSend} disabled={loading || !baseUrl}>
          {loading ? <DotsLoader /> : <Play className="h-3.5 w-3.5" />}
          {t("tryIt.send")}
        </Button>
      </div>

      {hasBody ? (
        <ResizablePanelGroup direction="vertical" className="flex-1 min-h-0" autoSaveId="tryit-request">
          <ResizablePanel defaultSize={40} minSize={15}>
            <div className="flex flex-col h-full min-h-0 overflow-auto">
              {paramsSection}
            </div>
          </ResizablePanel>
          <ResizableHandle />
          <ResizablePanel defaultSize={60} minSize={20}>
            <div className="flex flex-col h-full min-h-0 px-3 py-2">
              <span className="text-[10px] font-semibold uppercase text-muted-foreground tracking-wider mb-1.5">{t("tryIt.body")}</span>
              <div className="flex-1 min-h-0 rounded-md overflow-hidden border">
                <Suspense fallback={<pre className="w-full h-full rounded-md border border-border p-2 font-mono text-xs">{body}</pre>}>
                  <MonacoCodeEditor
                    className="h-full"
                    value={body}
                    onChange={onBodyChange}
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
      ) : (
        <div className="flex-1 flex flex-col min-h-0 overflow-auto">
          {paramsSection}
        </div>
      )}
    </>
  );
}
