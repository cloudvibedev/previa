import { useState, useMemo, useCallback, useEffect, useRef } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { useTranslation } from "react-i18next";
import yaml from "js-yaml";
import type { OpenAPIRoute } from "@/types/pipeline";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

import { CheckCircle2, Plus, X, Server, Play } from "lucide-react";
import { checkForUpdate } from "@/lib/spec-sync";
import { validateSpec } from "@/lib/api-client";
import { getApiUrl } from "@/stores/useOrchestratorStore";
import { useEventStore } from "@/stores/useEventStore";
import { useSpecSyncStore } from "@/stores/useSpecSyncStore";
import { SplitPaneLayout } from "@/components/SplitPaneLayout";
import { PreviewLayout } from "@/components/PreviewLayout";
import { OpenAPIEditor } from "@/components/editors";
import { RouteItem } from "@/components/RouteItem";
import { TryItDrawer } from "@/components/TryItDrawer";
import { parseOpenAPISpec } from "@/lib/openapi-parser";
import { slugify } from "@/types/project";
import type { OpenAPISpec } from "@/types/pipeline";
import { useEditorFormatStore } from "@/stores/useEditorFormatStore";
import type { FormatType } from "@/lib/pipeline-schema";
import { UnsavedChangesDialog } from "@/components/UnsavedChangesDialog";

interface RouteEditorPageProps {
  spec?: OpenAPISpec;
  specId?: string;
  projectId?: string;
  initialSlug?: string;
  initialServers?: Record<string, string>;
  initialUrl?: string;
  initialSync?: boolean;
  initialSpecMd5?: string;
  onConfirm: (spec: OpenAPISpec, slug: string, servers: Record<string, string>, url?: string, sync?: boolean) => void;
  onCancel?: () => void;
  isDark?: boolean;
}

export default function RouteEditorPage({ spec, specId, projectId, initialSlug, initialServers, initialUrl, initialSync, initialSpecMd5, onConfirm, onCancel, isDark: isDarkProp }: RouteEditorPageProps) {
  const { t } = useTranslation();
  const isDark = isDarkProp ?? document.documentElement.classList.contains("dark");
  const navigate = useNavigate();
  const location = useLocation();
  const [editorContent, setEditorContent] = useState<string>(() => {
    if (spec?.raw) {
      try {
        return JSON.stringify(spec.raw, null, 2);
      } catch {
        return "";
      }
    }
    return "";
  });
  
  const { format, setFormat } = useEditorFormatStore();
  const [parseError, setParseError] = useState<string | null>(null);
  const [slug, setSlug] = useState(initialSlug ?? "");
  const [servers, setServers] = useState<{ key: string; value: string }[]>(() => {
    if (initialServers && Object.keys(initialServers).length > 0) {
      return Object.entries(initialServers).map(([key, value]) => ({ key, value }));
    }
    return [];
  });
  const [sourceUrl, setSourceUrl] = useState<string | null>(initialUrl ?? null);
  const [liveCheck, setLiveCheck] = useState(initialSync ?? false);
  const [tryItRoute, setTryItRoute] = useState<OpenAPIRoute | null>(null);
  const [lastTryItRoute, setLastTryItRoute] = useState<OpenAPIRoute | null>(null);
  const closingTryItRef = useRef(false);
  const [showUnsavedDialog, setShowUnsavedDialog] = useState(false);

  const savedContentRef = useRef<string>("");

  const normalizeContent = useCallback((content: string): string => {
    if (!content) return "";
    try {
      const trimmed = content.trim();
      if (!trimmed) return "";
      const parsed = trimmed.startsWith("{") || trimmed.startsWith("[")
        ? JSON.parse(trimmed)
        : yaml.load(trimmed);
      return JSON.stringify(parsed);
    } catch { return content.trim(); }
  }, []);

  // Initialize saved content ref
  useEffect(() => {
    if (spec?.raw) {
      try {
        savedContentRef.current = JSON.stringify(spec.raw);
      } catch { /* ignore */ }
    }
  }, []); // only on mount

  const isDirty = useMemo(() => {
    return normalizeContent(editorContent) !== savedContentRef.current;
  }, [editorContent, normalizeContent]);

  const handleFormatChange = useCallback((newFormat: FormatType) => {
    setFormat(newFormat);
  }, [setFormat]);

  const handleEditorChange = useCallback((newValue: string) => {
    setEditorContent(newValue);
  }, []);

  // Sync editor content when spec prop changes (e.g. after accepting a remote update)
  useEffect(() => {
    if (spec?.raw) {
      try {
        const newContent = JSON.stringify(spec.raw, null, 2);
        setEditorContent(newContent);
      } catch {
        // ignore
      }
    }
  }, [spec?.raw]);

  const parsedRoutes = useMemo<OpenAPIRoute[]>(() => {
    if (!editorContent?.trim()) return [];
    try {
      const parsed = parseOpenAPISpec(editorContent);
      setParseError(null);
      // Auto-generate slug from spec title if slug is empty
      if (!slug && parsed.title) {
        setSlug(slugify(parsed.title));
      }
      return parsed.routes;
    } catch (e) {
      setParseError((e as Error).message);
      return [];
    }
  }, [editorContent]);

  // Auto-open TryItDrawer if URL ends with /try-it
  useEffect(() => {
    if (!location.pathname.endsWith("/try-it")) {
      closingTryItRef.current = false;
      return;
    }
    if (closingTryItRef.current) return;
    if (!tryItRoute && parsedRoutes.length > 0) {
      setTryItRoute(lastTryItRoute || parsedRoutes[0]);
    }
  }, [location.pathname, parsedRoutes]);

  const serversRecord = useMemo(() => {
    const record: Record<string, string> = {};
    for (const s of servers) {
      if (s.key.trim() && s.value.trim()) {
        record[s.key.trim()] = s.value.trim();
      }
    }
    return record;
  }, [servers]);

  const slugError = useMemo(() => {
    if (!slug) return null;
    const hasDash = slug.includes("-");
    const hasUnderscore = slug.includes("_");
    if (hasDash && hasUnderscore) return "Slug não pode misturar - e _";
    if (!/^[a-z0-9]+([_-][a-z0-9]+)*$/.test(slug)) {
      return "Slug inválido (ex: auth-api ou auth_api)";
    }
    return null;
  }, [slug]);

  const handleLiveCheckChange = useCallback(async (enabled: boolean) => {
    setLiveCheck(enabled);
    if (!enabled || !sourceUrl) return;

    const baseUrl = getApiUrl();
    if (!baseUrl) return;

    try {
      let currentMd5 = initialSpecMd5;
      if (!currentMd5) {
        const currentValidation = await validateSpec(baseUrl, editorContent);
        currentMd5 = currentValidation.sourceMd5;
      }

      // Register in sync store so polling takes over
      if (specId && projectId) {
        useSpecSyncStore.getState().enableSync(projectId, specId, sourceUrl, currentMd5);
      }

      const result = await checkForUpdate(sourceUrl, currentMd5);
      if (result.changed && result.newContent) {
        useEventStore.getState().addEvent({
          uid: `spec-sync-${specId}`,
          type: "warning",
          title: t("specSync.outdated"),
          message: t("specSync.remoteContentDiffers"),
          actionUrl: specId && projectId ? `/projects/${projectId}/specs/${specId}/diff` : undefined,
          actionLabel: t("specSync.viewChanges"),
        });
      }
    } catch {
      // silently ignore validation errors
    }
  }, [sourceUrl, editorContent, specId, projectId, initialSpecMd5]);

  const handleConfirm = () => {
    if (!editorContent.trim() || !slug.trim() || slugError) return;
    try {
      const newSpec = parseOpenAPISpec(editorContent);
      onConfirm(newSpec, slug.trim(), serversRecord, sourceUrl || undefined, liveCheck);
    } catch {
      // Error already shown in parseError
    }
  };

  const handleImport = (content: string, importSourceUrl?: string, importLiveCheck?: boolean) => {
    try {
      const newSpec = parseOpenAPISpec(content);
      const formatted = format === "json" 
        ? JSON.stringify(newSpec.raw, null, 2) 
        : yaml.dump(newSpec.raw, { indent: 2, lineWidth: -1 });
      setEditorContent(formatted);
      setParseError(null);
      if (importSourceUrl) {
        setSourceUrl(importSourceUrl);
      }
      if (importLiveCheck !== undefined) {
        setLiveCheck(importLiveCheck);
      }
    } catch (e) {
      setParseError((e as Error).message);
    }
  };

  const specTitle = useMemo(() => {
    if (!editorContent?.trim()) return null;
    try {
      const parsed = parseOpenAPISpec(editorContent);
      return { title: parsed.title, version: parsed.version };
    } catch {
      return spec ? { title: spec.title, version: spec.version } : null;
    }
  }, [editorContent, spec]);

  const addServer = () => setServers((s) => [...s, { key: "", value: "" }]);
  const removeServer = (idx: number) => setServers((s) => s.filter((_, i) => i !== idx));
  const updateServer = (idx: number, field: "key" | "value", val: string) =>
    setServers((s) => s.map((item, i) => (i === idx ? { ...item, [field]: val } : item)));

  const leftPanel = (
    <OpenAPIEditor
      value={editorContent}
      onChange={handleEditorChange}
      format={format}
      onFormatChange={handleFormatChange}
      onImport={handleImport}
      isDark={isDark}
      buttonBack={() => {
        if (isDirty) {
          setShowUnsavedDialog(true);
        } else {
          onCancel?.();
        }
      }}
      liveCheck={liveCheck}
      onLiveCheckChange={handleLiveCheckChange}
      showLiveCheck={true}
    />
  );

  const rightPanel = (
    <PreviewLayout
      title={specTitle?.title || "Routes"}
      subtitle={specTitle ? `v${specTitle.version}` : "Import a spec to get started"}
      rightContent={
        <div className="flex items-center gap-2">
          <Badge variant="secondary">{parsedRoutes.length} routes</Badge>
          <Button
            variant="ghost"
            size="sm"
            disabled={parsedRoutes.length === 0}
            onClick={() => {
              const route = lastTryItRoute || parsedRoutes[0];
              setTryItRoute(route);
              const tryItPath = location.pathname.replace(/\/editor$/, "/try-it");
              if (tryItPath !== location.pathname) navigate(tryItPath, { replace: true });
            }}
          >
            <Play className="h-3.5 w-3.5" /> Try It
          </Button>
        </div>
      }
      buttonContent={<><CheckCircle2 className="h-4 w-4" /> {t("routeEditor.confirmButton")}</>}
      onButtonClick={handleConfirm}
      buttonDisabled={!!parseError || parsedRoutes.length === 0 || !slug.trim() || !!slugError}
    >
      {/* Slug & Servers section */}
      <div className="border-border/50 p-4 space-y-4">
        {/* Slug */}
        <div className="space-y-1.5">
          <Label htmlFor="spec-slug" className="text-xs font-medium">{t("routeEditor.slug.label")}</Label>
          <Input
            id="spec-slug"
            placeholder="ex: auth-api ou auth_api"
            value={slug}
            onChange={(e) => setSlug(e.target.value.toLowerCase().replace(/[^a-z0-9_-]/g, ""))}
            className="font-mono text-sm h-8"
          />
          {slugError && <p className="text-[10px] text-destructive">{slugError}</p>}
          {slug && !slugError && (
            <p className="text-[10px] text-muted-foreground">
              Variável na pipeline: <code className="text-primary font-mono">{`{{url.${slug}.<env>}}`}</code>
            </p>
          )}
        </div>

        {/* Servers */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label className="text-xs font-medium flex items-center gap-1.5">
              <Server className="h-3.5 w-3.5" />
              {t("routeEditor.servers.label")}
            </Label>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 gap-1 text-xs"
              onClick={addServer}
            >
              <Plus className="h-3 w-3" /> {t("routeEditor.servers.add")}
            </Button>
          </div>
          {servers.map((s, i) => (
            <div key={i} className="flex items-center gap-2">
              <Input
                placeholder="nome (ex: hml)"
                value={s.key}
                onChange={(e) => updateServer(i, "key", e.target.value.toLowerCase().replace(/[^a-z0-9-_]/g, ""))}
                className="w-24 h-8 text-xs font-mono"
              />
              <Input
                placeholder="http://localhost:3000"
                value={s.value}
                onChange={(e) => updateServer(i, "value", e.target.value)}
                className="flex-1 h-8 text-xs font-mono"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="h-7 w-7 shrink-0"
                onClick={() => removeServer(i)}
              >
                <X className="h-3 w-3" />
              </Button>
            </div>
          ))}
          {servers.length === 0 && (
            <p className="text-xs text-muted-foreground">{t("routeEditor.servers.none")}</p>
          )}
        </div>
      </div>

      {/* Routes list */}
      <div className="space-y-1 p-2">
        {parsedRoutes.map((route, i) => (
          <RouteItem key={`${route.method}-${route.path}-${i}`} route={route} onTryIt={setTryItRoute} />
        ))}
        {parsedRoutes.length === 0 && !parseError && (
          <div className="py-8 text-center text-sm text-muted-foreground">
            {t("routeEditor.noRoutes")}
          </div>
        )}
      </div>
    </PreviewLayout>
  );

  return (
    <>
    <div className="flex flex-col h-full w-full">
      {tryItRoute ? (
        <TryItDrawer
          route={tryItRoute}
          servers={serversRecord}
          onClose={() => {
            setLastTryItRoute(tryItRoute);
            setTryItRoute(null);
            closingTryItRef.current = true;
            const editorPath = location.pathname.replace(/\/try-it$/, "/editor");
            if (editorPath !== location.pathname) navigate(editorPath, { replace: true });
          }}
          allRoutes={parsedRoutes}
          onSelectRoute={setTryItRoute}
        />
      ) : (
        <div className="flex-1 min-h-0 flex flex-col">
          <SplitPaneLayout
            leftPanel={leftPanel}
            rightPanel={rightPanel}
            leftDefaultSize={35}
            rightDefaultSize={65}
            leftMinSize={35}
            rightMinSize={25}
            autoSaveId="split-spec"
            withPadding={false}
            withBorder={false}
          />
        </div>
      )}
    </div>
    <UnsavedChangesDialog
      open={showUnsavedDialog}
      onOpenChange={setShowUnsavedDialog}
      onSave={() => {
        handleConfirm();
      }}
      onDiscard={() => onCancel?.()}
    />
    </>
  );
}
