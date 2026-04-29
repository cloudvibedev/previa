import { useState } from "react";
import type { OnMount } from "@monaco-editor/react";
import MonacoCodeEditor, { type GenericValidationResult } from "@/components/MonacoCodeEditor";
import { ImportButton } from "@/components/ImportButton";
import { PipelineSchema, type FormatType } from "@/lib/pipeline-schema";
import type { MarkerInfo } from "@/lib/pipeline-schema";
import type { Pipeline } from "@/types/pipeline";
import type { TemplateValidationContext } from "@/lib/template-validator";

export interface PipelineEditorProps {
  value: string;
  onChange: (value: string) => void;
  onValidation?: (result: GenericValidationResult<Pipeline>) => void;
  format?: FormatType;
  onFormatChange?: (format: FormatType) => void;
  onImport?: (content: string) => void;
  importDialogTitle?: string;
  importDialogDescription?: string;
  isDark?: boolean;
  title?: string;
  className?: string;
  readOnly?: boolean;
  buttonBack?: () => void;
  warningMarkers?: MarkerInfo[];
  validationContext?: TemplateValidationContext;
  /** Ref to expose the Monaco editor instance */
  editorInstanceRef?: React.MutableRefObject<Parameters<OnMount>[0] | null>;
}

export function PipelineEditor({
  value,
  onChange,
  onValidation,
  format: controlledFormat,
  onFormatChange,
  onImport,
  importDialogTitle = "Import Pipeline",
  importDialogDescription = "Paste a URL or upload a pipeline JSON/YAML file",
  isDark,
  title = "Pipeline",
  className = "flex-1",
  readOnly = false,
  buttonBack,
  warningMarkers,
  validationContext,
  editorInstanceRef,
}: PipelineEditorProps) {
  const [internalFormat, setInternalFormat] = useState<FormatType>("json");
  
  const format = controlledFormat ?? internalFormat;
  const handleFormatChange = onFormatChange ?? setInternalFormat;

  const headerActions = onImport ? (
    <ImportButton
      onImport={onImport}
      dialogTitle={importDialogTitle}
      dialogDescription={importDialogDescription}
      variant="ghost"
      size="sm"
    />
  ) : undefined;

  return (
    <MonacoCodeEditor
      value={value}
      onChange={onChange}
      format={format}
      onFormatChange={handleFormatChange}
      isDark={isDark}
      title={title}
      headerActions={headerActions}
      schema={PipelineSchema}
      showValidation={true}
      onValidation={onValidation as (result: GenericValidationResult<unknown>) => void}
      className={className}
      readOnly={readOnly}
      buttonBack={buttonBack}
      warningMarkers={warningMarkers}
      validationContext={validationContext}
      editorInstanceRef={editorInstanceRef}
    />
  );
}
