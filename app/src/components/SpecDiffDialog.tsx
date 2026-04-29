import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { MonacoDiffEditor } from "@/components/MonacoDiffEditor";
import { useEffect, useRef, useState } from "react";
import { Plus, Minus, PenLine } from "lucide-react";
import i18n from "@/i18n";
import { useTranslation } from "react-i18next";

interface LineChange {
  originalStartLineNumber: number;
  originalEndLineNumber: number;
  modifiedStartLineNumber: number;
  modifiedEndLineNumber: number;
}

type ChangeType = "added" | "removed" | "modified";

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

interface SpecDiffDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  specName: string;
  currentContent: string;
  newContent: string;
  onAccept: () => void;
  onDismiss: () => void;
}

function detectLanguage(content: string): string {
  const trimmed = content.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) return "json";
  return "yaml";
}

export function SpecDiffDialog({
  open,
  onOpenChange,
  specName,
  currentContent,
  newContent,
  onAccept,
  onDismiss,
}: SpecDiffDialogProps) {
  const { t } = useTranslation();
  const [theme, setTheme] = useState<string>(
    document.documentElement.classList.contains("dark") ? "vs-dark" : "vs"
  );
  const [changes, setChanges] = useState<LineChange[]>([]);
  const diffEditorRef = useRef<any>(null);

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setTheme(document.documentElement.classList.contains("dark") ? "vs-dark" : "vs");
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!open) {
      setChanges([]);
      diffEditorRef.current = null;
    }
  }, [open]);

  const language = detectLanguage(currentContent || newContent);

  const handleEditorMount = (editor: any) => {
    diffEditorRef.current = editor;

    const updateChanges = () => {
      const lineChanges = editor.getLineChanges();
      if (lineChanges) setChanges(lineChanges);
    };

    editor.onDidUpdateDiff(updateChanges);
    setTimeout(updateChanges, 300);
  };

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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-[95vw] w-[95vw] h-[85vh] flex flex-col p-0 gap-0">
        <DialogHeader className="px-6 py-4 border-border/50">
          <DialogTitle>{t("specDiff.title", { name: specName })}</DialogTitle>
          <DialogDescription>
            {t("specDiff.description")}
          </DialogDescription>
        </DialogHeader>

        <div className="flex-1 min-h-0 flex flex-row">
          <div className="w-64 border-r border-border/50 flex flex-col bg-muted/30">
            <div className="px-3 py-2.5 border-border/50">
              <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                {t("specDiff.occurrences", { count: changes.length })}
              </span>
            </div>
            <ScrollArea className="flex-1">
              <div className="p-1.5 space-y-0.5">
                {changes.length === 0 && (
                  <p className="text-xs text-muted-foreground px-2 py-3 text-center">
                    {t("specDiff.noDifferences")}
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
            {open && (
              <MonacoDiffEditor
                original={currentContent}
                modified={newContent}
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
            )}
          </div>
        </div>

        <div className="flex items-center justify-end gap-3 px-6 py-4 border-border/50">
          <Button variant="outline" onClick={onDismiss}>
            {t("specDiff.keepCurrent")}
          </Button>
          <Button onClick={onAccept}>
            {t("specDiff.updateSpec")}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
