import { useState } from "react";
import MonacoCodeEditor from "@/components/MonacoCodeEditor";
import { ImportButton } from "@/components/ImportButton";
import { OPENAPI_3_SCHEMA } from "@/lib/openapi-schema";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Radio } from "lucide-react";
import type { FormatType } from "@/lib/pipeline-schema";

export interface OpenAPIEditorProps {
  value: string;
  onChange: (value: string) => void;
  /** Controlled format state */
  format?: FormatType;
  /** Callback when format changes */
  onFormatChange?: (format: FormatType) => void;
  /** Callback when import button is used */
  onImport?: (content: string, sourceUrl?: string, liveCheck?: boolean) => void;
  /** Title for import dialog */
  importDialogTitle?: string;
  /** Description for import dialog */
  importDialogDescription?: string;
  /** Editor title (default: "OpenAPI Schema") */
  title?: string;
  /** Editor height (default: "100%") */
  height?: string;
  /** Additional CSS classes */
  className?: string;
  /** Whether editor is read-only */
  readOnly?: boolean;
  /** Dark mode */
  isDark?: boolean;
  /** Back button callback */
  buttonBack?: () => void;
  /** Live check state */
  liveCheck?: boolean;
  /** Live check change handler */
  onLiveCheckChange?: (v: boolean) => void;
  /** Show live check toggle */
  showLiveCheck?: boolean;
}

export function OpenAPIEditor({
  value,
  onChange,
  format: controlledFormat,
  onFormatChange,
  onImport,
  importDialogTitle = "Import OpenAPI Spec",
  importDialogDescription = "Paste a URL or upload an OpenAPI JSON/YAML file",
  title = "OpenAPI Schema",
  height = "100%",
  className = "h-full",
  readOnly = false,
  isDark,
  buttonBack,
  liveCheck = false,
  onLiveCheckChange,
  showLiveCheck = false,
}: OpenAPIEditorProps) {
  const [internalFormat, setInternalFormat] = useState<FormatType>("json");
  
  const format = controlledFormat ?? internalFormat;
  const handleFormatChange = onFormatChange ?? setInternalFormat;

  const headerActions = (
    <div className="flex items-center gap-2">
      {showLiveCheck && onLiveCheckChange && (
        <div className="flex items-center gap-1.5">
          <Label htmlFor="header-live-check" className="text-[10px] font-medium flex items-center gap-1 cursor-pointer text-muted-foreground">
            <Radio className="h-3 w-3" />
            Live
          </Label>
          <Switch
            id="header-live-check"
            checked={liveCheck}
            onCheckedChange={onLiveCheckChange}
            className="h-4 w-8 [&>span]:h-3 [&>span]:w-3 data-[state=checked]:bg-primary"
          />
        </div>
      )}
      {onImport && (
        <ImportButton
          onImport={onImport}
          dialogTitle={importDialogTitle}
          dialogDescription={importDialogDescription}
          variant="ghost"
          size="sm"
        />
      )}
    </div>
  );

  return (
    <MonacoCodeEditor
      value={value}
      onChange={onChange}
      format={format}
      onFormatChange={handleFormatChange}
      height={height}
      title={title}
      showHeader={true}
      showValidation={true}
      className={className}
      jsonSchema={format === "json" ? OPENAPI_3_SCHEMA : undefined}
      headerActions={headerActions}
      readOnly={readOnly}
      isDark={isDark}
      buttonBack={buttonBack}
    />
  );
}
