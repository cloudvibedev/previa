import { lazy, Suspense, type ComponentType } from "react";
import { DotsLoader } from "@/components/DotsLoader";

const MonacoDiffEditorLazy = lazy(async (): Promise<{ default: ComponentType<any> }> => {
  const mod: any = await import("@monaco-editor/react");
  const resolved = mod?.DiffEditor ?? mod?.default?.DiffEditor;

  if (!resolved) {
    throw new Error("Failed to load Monaco DiffEditor");
  }

  return { default: resolved as ComponentType<any> };
});

interface MonacoDiffEditorProps {
  original: string;
  modified: string;
  language: string;
  theme: string;
  height?: string;
  onMount?: (editor: any) => void;
  options?: Record<string, unknown>;
}

export function MonacoDiffEditor({
  original,
  modified,
  language,
  theme,
  height = "100%",
  onMount,
  options,
}: MonacoDiffEditorProps) {
  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center">
          <DotsLoader />
        </div>
      }
    >
      <MonacoDiffEditorLazy
        original={original}
        modified={modified}
        language={language}
        theme={theme}
        onMount={onMount}
        options={options}
        height={height}
      />
    </Suspense>
  );
}
