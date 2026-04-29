import { useEffect, useRef, useState, useCallback, useMemo } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { useAppHeader } from "@/components/AppShell";
import { MonacoDiffEditor } from "@/components/MonacoDiffEditor";
import { Plus, Minus, PenLine, CheckCircle2, AlertTriangle, ArrowLeft } from "lucide-react";
import { DotsLoader } from "@/components/DotsLoader";
import { useProjectStore } from "@/stores/useProjectStore";
import { useSpecSyncStore } from "@/stores/useSpecSyncStore";
import { useEventStore } from "@/stores/useEventStore";
import { useOrchestratorStore } from "@/stores/useOrchestratorStore";
import { validateSpec, ensureApiPrefix } from "@/lib/api-client";
import { parseOpenAPISpec } from "@/lib/openapi-parser";
import yaml from "js-yaml";
import i18n from "@/i18n";

// ─── types ────────────────────────────────────────────────────────────────────

interface LineChange {
  originalStartLineNumber: number;
  originalEndLineNumber: number;
  modifiedStartLineNumber: number;
  modifiedEndLineNumber: number;
}

type ChangeType = "added" | "removed" | "modified";
type PageState = "loading" | "ready" | "up-to-date" | "error" | "no-sync";

// ─── helpers ──────────────────────────────────────────────────────────────────

function formatContent(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    try {
      return JSON.stringify(JSON.parse(trimmed), null, 2);
    } catch {
      return raw;
    }
  }
  try {
    return yaml.dump(yaml.load(trimmed) as Record<string, unknown>, { indent: 2, lineWidth: -1 });
  } catch {
    return raw;
  }
}

function detectLanguage(content: string): string {
  const trimmed = content.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) return "json";
  return "yaml";
}

function classifyChange(change: LineChange): ChangeType {
  if (change.originalStartLineNumber > change.originalEndLineNumber) return "added";
  if (change.modifiedStartLineNumber > change.modifiedEndLineNumber) return "removed";
  return "modified";
}

function changeLabel(type: ChangeType, change: LineChange): string {
  switch (type) {
    case "added": {
      const s = change.modifiedStartLineNumber;
      const e = change.modifiedEndLineNumber;
      return s === e
        ? i18n.t("specDiff.lineAdded", { line: s })
        : i18n.t("specDiff.linesAdded", { start: s, end: e });
    }
    case "removed": {
      const s = change.originalStartLineNumber;
      const e = change.originalEndLineNumber;
      return s === e
        ? i18n.t("specDiff.lineRemoved", { line: s })
        : i18n.t("specDiff.linesRemoved", { start: s, end: e });
    }
    case "modified": {
      const s = change.modifiedStartLineNumber;
      const e = change.modifiedEndLineNumber;
      return s === e
        ? i18n.t("specDiff.lineModified", { line: s })
        : i18n.t("specDiff.linesModified", { start: s, end: e });
    }
  }
}

const changeIcon: Record<ChangeType, React.ReactNode> = {
  added: <Plus className="h-3.5 w-3.5 text-success" />,
  removed: <Minus className="h-3.5 w-3.5 text-destructive" />,
  modified: <PenLine className="h-3.5 w-3.5 text-warning" />,
};

// ─── component ────────────────────────────────────────────────────────────────

export default function SpecDiffPage() {
  const { t } = useTranslation();
  const { id: projectId, specId } = useParams<{ id: string; specId: string }>();
  const navigate = useNavigate();

  const { currentProject, loadProject, updateSpec } = useProjectStore();
  const { syncs, acceptUpdate, dismissUpdate, hydrate } = useSpecSyncStore();
  const orchUrl = useOrchestratorStore((s) => s.url);

  const [pageState, setPageState] = useState<PageState>("loading");
  const [errorMsg, setErrorMsg] = useState("");
  const [currentContent, setCurrentContent] = useState("");
  const [remoteContent, setRemoteContent] = useState("");
  const [remoteHash, setRemoteHash] = useState("");
  const [changes, setChanges] = useState<LineChange[]>([]);
  const [theme, setTheme] = useState<string>(
    document.documentElement.classList.contains("dark") ? "vs-dark" : "vs"
  );
  const diffEditorRef = useRef<any>(null);

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setTheme(document.documentElement.classList.contains("dark") ? "vs-dark" : "vs");
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!projectId || !specId) {
      setErrorMsg(t("specDiffPage.invalidParams"));
      setPageState("error");
      return;
    }

    let cancelled = false;

    async function load() {
      setPageState("loading");

      let project = useProjectStore.getState().currentProject;
      if (!project || project.id !== projectId) {
        project = await loadProject(projectId!);
      }

      if (cancelled) return;

      if (!project) {
        setErrorMsg(t("specDiffPage.projectNotFound"));
        setPageState("error");
        return;
      }

      hydrate(project.specs);

      const spec = project.specs.find((s) => s.id === specId);
      if (!spec) {
        setErrorMsg(t("specDiffPage.specNotFound"));
        setPageState("error");
        return;
      }

      const specObj = spec.spec;
      const rawData = (specObj as any)?.raw ?? specObj;
      const current = rawData ? formatContent(JSON.stringify(rawData, null, 2)) : "";
      setCurrentContent(current);

      const syncEntries = useSpecSyncStore.getState().syncs;
      const syncEntry = syncEntries[specId!] ?? null;
      const syncUrl = syncEntry?.url ?? spec.url;

      if (!syncUrl) {
        setErrorMsg(t("specDiffPage.noSync"));
        setPageState("no-sync");
        return;
      }

      let source: string;
      try {
        const res = await fetch(syncUrl);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        source = await res.text();
      } catch (e: any) {
        if (cancelled) return;
        setErrorMsg(`${t("specDiffPage.errorTitle")}: ${e.message}`);
        setPageState("error");
        return;
      }

      if (cancelled) return;

      let newHash = "";
      const apiBase = orchUrl ? ensureApiPrefix(orchUrl) : null;

      if (apiBase) {
        try {
          const result = await validateSpec(apiBase, source);
          newHash = result.sourceMd5;
        } catch {
          // proceed without md5 comparison
        }
      }

      if (cancelled) return;

      const storedHash = syncEntry?.hash ?? (spec as any).specMd5 ?? "";
      if (newHash && storedHash && newHash === storedHash) {
        setPageState("up-to-date");
        return;
      }

      setRemoteContent(formatContent(source));
      setRemoteHash(newHash);
      setPageState("ready");
    }

    load();
    return () => { cancelled = true; };
  }, [projectId, specId, orchUrl]);

  const handleEditorMount = useCallback((editor: any) => {
    diffEditorRef.current = editor;
    const updateChanges = () => {
      const lineChanges = editor.getLineChanges();
      if (lineChanges) setChanges(lineChanges);
    };
    editor.onDidUpdateDiff(updateChanges);
    setTimeout(updateChanges, 300);
  }, []);

  const handleOccurrenceClick = (change: LineChange) => {
    const editor = diffEditorRef.current;
    if (!editor) return;
    const type = classifyChange(change);
    const line = type === "removed"
      ? change.originalStartLineNumber
      : change.modifiedStartLineNumber;
    if (type === "removed") {
      editor.getOriginalEditor().revealLineInCenter(line);
    } else {
      editor.getModifiedEditor().revealLineInCenter(line);
    }
  };

  const goBack = useCallback(() => {
    navigate(`/projects/${projectId}/specs/${specId}/editor`);
  }, [navigate, projectId, specId]);

  const headerBack = useCallback(() => {
    navigate("/");
  }, [navigate]);

  const headerActions = useMemo(() => undefined, []);

  useAppHeader({
    projectName: currentProject?.name,
    onBackToProjects: headerBack,
    headerActions,
  });

  const handleAccept = async () => {
    if (!projectId || !specId) return;
    const project = useProjectStore.getState().currentProject;
    const spec = project?.specs.find((s) => s.id === specId);

    const storeResult = acceptUpdate(projectId, specId);
    const contentToApply = storeResult?.newContent ?? remoteContent;

    if (contentToApply) {
      try {
        const parsed = parseOpenAPISpec(contentToApply);
        await updateSpec(
          projectId,
          specId,
          parsed,
          spec?.url,
          true,
          spec?.slug,
          spec?.servers,
        );
      } catch (e) {
        console.error("Failed to apply spec update:", e);
      }
    }

    if (specId) useEventStore.getState().dismissByUid(`spec-sync-${specId}`);
    goBack();
  };

  const handleDismiss = () => {
    if (specId && projectId) dismissUpdate(projectId, specId);
    if (specId) useEventStore.getState().dismissByUid(`spec-sync-${specId}`);
    goBack();
  };

  const specName = currentProject?.specs.find((s) => s.id === specId)?.name ?? specId ?? "Spec";
  const language = detectLanguage(currentContent || remoteContent);

  return (
    <div className="flex flex-1 flex-col bg-background">
      {pageState === "loading" && (
        <div className="flex flex-1 items-center justify-center gap-3 text-muted-foreground">
          <DotsLoader />
          <span className="text-sm">{t("specDiffPage.loading")}</span>
        </div>
      )}

      {pageState === "up-to-date" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <CheckCircle2 className="h-12 w-12 text-success" />
          <div className="text-center">
            <p className="text-lg font-semibold">{t("specDiffPage.upToDate")}</p>
            <p className="text-sm text-muted-foreground mt-1">
              {t("specDiffPage.upToDateDesc")}
            </p>
          </div>
          <Button variant="outline" onClick={goBack}>{t("common.backToProject")}</Button>
        </div>
      )}

      {pageState === "no-sync" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <AlertTriangle className="h-12 w-12 text-warning" />
          <div className="text-center">
            <p className="text-lg font-semibold">{t("specDiffPage.noSync")}</p>
            <p className="text-sm text-muted-foreground mt-1">{errorMsg}</p>
          </div>
          <Button variant="outline" onClick={goBack}>{t("common.backToProject")}</Button>
        </div>
      )}

      {pageState === "error" && (
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <AlertTriangle className="h-12 w-12 text-destructive" />
          <div className="text-center">
            <p className="text-lg font-semibold">{t("specDiffPage.errorTitle")}</p>
            <p className="text-sm text-muted-foreground mt-1">{errorMsg}</p>
          </div>
          <Button variant="outline" onClick={goBack}>{t("common.backToProject")}</Button>
        </div>
      )}

      {pageState === "ready" && (
        <>
          <div className="flex items-center justify-between border-border/50 px-6 py-3 bg-muted/20">
            <div className="flex items-center gap-3">
              <Button variant="ghost" size="sm" onClick={goBack} className="gap-1.5 -ml-2 h-7 px-2 text-muted-foreground hover:text-foreground">
                <ArrowLeft className="h-3.5 w-3.5" />
                {t("common.backToEditor")}
              </Button>
              <div className="w-px h-4 bg-border" />
              <div>
                <p className="text-sm font-semibold">{specName}</p>
                <p className="text-xs text-muted-foreground mt-0.5">
                  {t("specDiffPage.leftCurrent")}
                </p>
              </div>
            </div>
          </div>

          <div className="flex flex-1 min-h-0">
            <div className="w-60 shrink-0 border-r border-border/50 flex flex-col bg-muted/20">
              <div className="px-3 py-2.5 border-border/50">
                <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                  {t("specDiff.occurrences", { count: changes.length })}
                </span>
              </div>
              <ScrollArea className="flex-1">
                <div className="p-1.5 space-y-0.5">
                  {changes.length === 0 && (
                    <p className="text-xs text-muted-foreground px-2 py-3 text-center">
                      {t("specDiffPage.calculatingDiffs")}
                    </p>
                  )}
                  {changes.map((change, i) => {
                    const type = classifyChange(change);
                    return (
                      <button
                        key={i}
                        onClick={() => handleOccurrenceClick(change)}
                        className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-md text-left text-xs hover:bg-accent/50 transition-colors"
                      >
                        {changeIcon[type]}
                        <span className="truncate text-foreground/80">
                          {changeLabel(type, change)}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </ScrollArea>
            </div>

            <div className="flex-1 min-w-0">
              <MonacoDiffEditor
                original={currentContent}
                modified={remoteContent}
                language={language}
                theme={theme}
                onMount={handleEditorMount}
                options={{
                  readOnly: true,
                  renderSideBySide: true,
                  minimap: { enabled: false },
                  scrollBeyondLastLine: false,
                  fontSize: 13,
                  wordWrap: "on",
                }}
                height="100%"
              />
            </div>
          </div>

          <div className="flex items-center justify-end gap-3 px-6 py-4 border-border/50 bg-background">
            <Button variant="outline" onClick={handleDismiss}>
              {t("specDiff.keepCurrent")}
            </Button>
            <Button onClick={handleAccept}>
              {t("specDiff.updateSpec")}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
