import { lazy, Suspense, useRef, useEffect, useCallback, useState, forwardRef, type ComponentType } from "react";
import type { OnMount } from "@monaco-editor/react";
import { cn } from "@/lib/utils";
import { setupMonacoTemplateLanguage, applyMonacoTheme, registerTemplateCompletions } from "@/lib/monaco-template-setup";
import { validateInterpolations, type TemplateValidationContext } from "@/lib/template-validator";

const MonacoEditorLazy = lazy(async (): Promise<{ default: ComponentType<any> }> => {
  const mod: any = await import("@monaco-editor/react");
  const resolved = mod?.default?.default ?? mod?.default ?? mod;
  const Wrapped = forwardRef<any, any>((props, _ref) => {
    const C = resolved as ComponentType<any>;
    return <C {...props} />;
  });
  Wrapped.displayName = "MonacoEditorWrapped";
  return { default: Wrapped };
});

interface MonacoInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  language?: string;
  readOnly?: boolean;
  /** Validation context for template interpolation checks */
  validationContext?: TemplateValidationContext;
}

export function MonacoInput({
  value,
  onChange,
  placeholder,
  className,
  language = "plaintext",
  readOnly = false,
  validationContext,
}: MonacoInputProps) {
  const [isDark, setIsDark] = useState(
    () => document.documentElement.classList.contains("dark")
  );
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const monacoRef = useRef<any>(null);
  const completionDisposableRef = useRef<{ dispose: () => void } | null>(null);
  const validationContextRef = useRef(validationContext);
  const [isEmpty, setIsEmpty] = useState(!value);

  useEffect(() => {
    const el = document.documentElement;
    const observer = new MutationObserver(() => {
      setIsDark(el.classList.contains("dark"));
    });
    observer.observe(el, { attributes: true, attributeFilter: ["class"] });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    setIsEmpty(!value);
  }, [value]);

  // Keep ref in sync
  useEffect(() => {
    validationContextRef.current = validationContext;
  }, [validationContext]);

  // Run template validation and set markers
  const runValidation = useCallback((text: string) => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    if (!editor || !monaco) return;

    const model = editor.getModel();
    if (!model) return;

    const diagnostics = validateInterpolations(text, validationContext);

    const markers = diagnostics.map((d) => {
      // Convert offset to line/col
      const before = text.substring(0, d.offset);
      const line = (before.match(/\n/g) || []).length + 1;
      const lastNewline = before.lastIndexOf("\n");
      const col = d.offset - lastNewline; // 1-based

      const endBefore = text.substring(0, d.offset + d.length);
      const endLine = (endBefore.match(/\n/g) || []).length + 1;
      const endLastNewline = endBefore.lastIndexOf("\n");
      const endCol = d.offset + d.length - endLastNewline;

      return {
        severity: d.severity === "error" ? 8 : 4, // Error=8, Warning=4
        message: d.message,
        startLineNumber: line,
        startColumn: col,
        endLineNumber: endLine,
        endColumn: endCol,
      };
    });

    monaco.editor.setModelMarkers(model, "template-validator", markers);
  }, [validationContext]);

  // Re-validate when value or context changes
  useEffect(() => {
    runValidation(value);
  }, [value, validationContext, runValidation]);

  const handleMount: OnMount = useCallback((editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;

    // Prevent Enter from adding newlines — keep single-line
    editor.addCommand(monaco.KeyCode.Enter, () => {});

    // Use centralized setup
    setupMonacoTemplateLanguage(monaco, isDark);

    // Register completions
    completionDisposableRef.current?.dispose();
    completionDisposableRef.current = registerTemplateCompletions(
      monaco,
      () => validationContextRef.current
    );

    // Run initial validation
    runValidation(value);
  }, [isDark, runValidation, value]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      completionDisposableRef.current?.dispose();
    };
  }, []);

  // Update theme when it changes
  useEffect(() => {
    if (monacoRef.current) {
      applyMonacoTheme(monacoRef.current, isDark);
    }
  }, [isDark]);

  return (
    <div
      className={cn(
        "relative rounded-md border border-input bg-background transition-colors",
        "focus-within:ring-1 focus-within:ring-ring focus-within:border-ring",
        className,
      )}
    >
      {isEmpty && placeholder && (
        <span className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 z-10 text-xs text-muted-foreground font-mono truncate">
          {placeholder}
        </span>
      )}
      <Suspense fallback={<div className="h-full w-full" />}>
        <MonacoEditorLazy
          height="100%"
          language={language === "plaintext" ? "template-input" : language}
          value={value}
          onChange={(v: string | undefined) => {
            const clean = (v ?? "").replace(/\n/g, "");
            onChange(clean);
          }}
          theme={isDark ? "transparent-dark" : "transparent-light"}
          onMount={handleMount}
          options={{
            lineNumbers: "off",
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            overviewRulerLanes: 0,
            folding: false,
            glyphMargin: false,
            wordWrap: "off",
            renderLineHighlight: "none",
            lineDecorationsWidth: 8,
            lineNumbersMinChars: 0,
            scrollbar: { horizontal: "hidden", vertical: "hidden", handleMouseWheel: false },
            readOnly,
            fontSize: 12,
            fontFamily: "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace",
            padding: { top: 6, bottom: 6 },
            contextmenu: false,
            quickSuggestions: true,
            suggestOnTriggerCharacters: true,
            parameterHints: { enabled: false },
            tabCompletion: "off",
            wordBasedSuggestions: "off",
          }}
        />
      </Suspense>
    </div>
  );
}
