import { useState, useCallback, useRef, lazy, Suspense, useEffect, type ComponentType, type ReactNode } from "react";
import type { Monaco, OnMount } from "@monaco-editor/react";
import yaml from "js-yaml";
import { z } from "zod";
import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { ValidationFooter, type ValidationFooterError } from "@/components/ValidationFooter";
import { positionToLineCol, type MarkerInfo, type FormatType } from "@/lib/pipeline-schema";
import { setupMonacoTemplateLanguage, applyMonacoTheme, registerTemplateCompletions } from "@/lib/monaco-template-setup";
import { validateInterpolations, type TemplateValidationContext } from "@/lib/template-validator";

const MonacoEditorLazy = lazy(async (): Promise<{ default: ComponentType<any> }> => {
  const mod: any = await import("@monaco-editor/react");
  const resolved = mod?.default?.default ?? mod?.default ?? mod;
  return { default: resolved as ComponentType<any> };
});

// ============ Types ============

type PathPosition = { startLine: number; startCol: number; endLine: number; endCol: number };

interface IMarkerData {
  severity: number;
  message: string;
  startLineNumber: number;
  startColumn: number;
  endLineNumber: number;
  endColumn: number;
}

export interface ErrorWithPosition {
  message: string;
  line: number;
  column: number;
}

export interface ValidationResultSuccess<T> {
  success: true;
  data: T;
}

export interface ValidationResultError {
  success: false;
  errors: string[];
  markers: MarkerInfo[];
  syntaxError?: { line: number; column: number; message: string };
}

export type GenericValidationResult<T> = ValidationResultSuccess<T> | ValidationResultError;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export interface MonacoCodeEditorProps<T = any> {
  value: string;
  onChange?: (value: string) => void;
  format?: FormatType;
  onFormatChange?: (format: FormatType) => void;
  readOnly?: boolean;
  height?: string;
  isDark?: boolean;
  title?: string;
  headerActions?: ReactNode;
  buttonBack?: () => void;
  schema?: z.ZodSchema<T>;
  /** JSON Schema object for Monaco's built-in validation (e.g., OpenAPI 3.0 schema) */
  jsonSchema?: object;
  showValidation?: boolean;
  onValidation?: (result: GenericValidationResult<T>) => void;
  /** External warning markers (e.g., contract validation warnings) */
  warningMarkers?: MarkerInfo[];
  /** Validation context for template interpolation autocomplete */
  validationContext?: TemplateValidationContext;
  className?: string;
  showHeader?: boolean;
  showLineNumbers?: boolean;
  /** Ref to expose the Monaco editor instance to parent components */
  editorInstanceRef?: React.MutableRefObject<Parameters<OnMount>[0] | null>;
}

// ============ Utility Functions ============

function formatContent(value: string, format: FormatType): string {
  try {
    // Parse from any format (yaml.load handles both JSON and YAML)
    const parsed = yaml.load(value);
    if (format === "yaml") {
      return yaml.dump(parsed, { indent: 2, lineWidth: -1 });
    }
    return JSON.stringify(parsed, null, 2);
  } catch {
    return value;
  }
}

function jsonToYaml(jsonString: string): string {
  try {
    const obj = JSON.parse(jsonString);
    return yaml.dump(obj, { indent: 2, lineWidth: -1 });
  } catch {
    return jsonString;
  }
}

function yamlToJson(yamlString: string): string {
  try {
    const obj = yaml.load(yamlString);
    return JSON.stringify(obj, null, 2);
  } catch {
    return yamlString;
  }
}

function findJsonPathPosition(
  text: string,
  path: (string | number)[]
): PathPosition | null {
  if (path.length === 0) {
    return { startLine: 1, startCol: 1, endLine: 1, endCol: 2 };
  }

  let searchStart = 0;

  for (let i = 0; i < path.length; i++) {
    const segment = path[i];

    if (typeof segment === "string") {
      const keyPattern = new RegExp(`"${segment}"\\s*:`);
      const subText = text.slice(searchStart);
      const match = keyPattern.exec(subText);

      if (!match) return null;

      const absolutePos = searchStart + match.index;
      searchStart = absolutePos + match[0].length;

      if (i === path.length - 1) {
        const { line, column } = positionToLineCol(text, absolutePos);
        const keyLength = segment.length + 2;
        return {
          startLine: line,
          startCol: column,
          endLine: line,
          endCol: column + keyLength,
        };
      }
    } else if (typeof segment === "number") {
      const subText = text.slice(searchStart);
      const bracketIdx = subText.indexOf("[");
      if (bracketIdx === -1) return null;

      searchStart += bracketIdx + 1;
      let arrayDepth = 0;
      let objectDepth = 0;
      let elementCount = 0;
      let elementStart = searchStart;

      for (let j = searchStart; j < text.length; j++) {
        const char = text[j];

        if (char === "[") arrayDepth++;
        else if (char === "]") {
          if (arrayDepth === 0) break;
          arrayDepth--;
        } else if (char === "{") objectDepth++;
        else if (char === "}") objectDepth--;
        else if (char === "," && arrayDepth === 0 && objectDepth === 0) {
          if (elementCount === segment) {
            searchStart = elementStart;
            break;
          }
          elementCount++;
          elementStart = j + 1;
        }

        if (j === text.length - 1 || (text[j + 1] === "]" && arrayDepth === 0 && objectDepth === 0)) {
          if (elementCount === segment) {
            searchStart = elementStart;
          }
        }
      }

      if (i === path.length - 1) {
        const { line, column } = positionToLineCol(text, searchStart);
        return {
          startLine: line,
          startCol: column,
          endLine: line,
          endCol: column + 1,
        };
      }
    }
  }

  return null;
}

function findYamlPathPosition(
  text: string,
  path: (string | number)[]
): PathPosition | null {
  if (path.length === 0) {
    return { startLine: 1, startCol: 1, endLine: 1, endCol: 2 };
  }

  const lines = text.split("\n");
  let currentLineIdx = 0;
  let currentIndent = -1;

  for (let i = 0; i < path.length; i++) {
    const segment = path[i];

    if (typeof segment === "string") {
      let found = false;
      for (let lineIdx = currentLineIdx; lineIdx < lines.length; lineIdx++) {
        const line = lines[lineIdx];
        const lineIndent = line.search(/\S/);
        
        if (lineIndent === -1) continue;
        
        if (currentIndent >= 0 && lineIndent <= currentIndent && lineIdx > currentLineIdx) {
          break;
        }

        const keyMatch = line.match(new RegExp(`^(\\s*)${segment}\\s*:`));
        if (keyMatch) {
          const keyIndent = keyMatch[1].length;
          
          if (currentIndent < 0 || keyIndent > currentIndent) {
            currentLineIdx = lineIdx;
            currentIndent = keyIndent;
            found = true;

            if (i === path.length - 1) {
              const startCol = keyIndent + 1;
              return {
                startLine: lineIdx + 1,
                startCol,
                endLine: lineIdx + 1,
                endCol: startCol + segment.length,
              };
            }
            break;
          }
        }
      }
      if (!found) return null;
    } else if (typeof segment === "number") {
      let elementCount = 0;
      let found = false;

      for (let lineIdx = currentLineIdx + 1; lineIdx < lines.length; lineIdx++) {
        const line = lines[lineIdx];
        const lineIndent = line.search(/\S/);
        
        if (lineIndent === -1) continue;
        
        if (lineIndent <= currentIndent && currentIndent >= 0) {
          break;
        }

        const itemMatch = line.match(/^(\s*)-\s/);
        if (itemMatch) {
          const itemIndent = itemMatch[1].length;
          
          if (itemIndent > currentIndent || currentIndent < 0) {
            if (elementCount === segment) {
              currentLineIdx = lineIdx;
              currentIndent = itemIndent;
              found = true;

              if (i === path.length - 1) {
                return {
                  startLine: lineIdx + 1,
                  startCol: itemIndent + 1,
                  endLine: lineIdx + 1,
                  endCol: itemIndent + 2,
                };
              }
              break;
            }
            elementCount++;
          }
        }
      }
      if (!found) return null;
    }
  }

  return null;
}

function findPathPosition(
  text: string,
  path: (string | number)[],
  format: FormatType
): PathPosition | null {
  return format === "yaml" 
    ? findYamlPathPosition(text, path) 
    : findJsonPathPosition(text, path);
}

function createMonacoMarkersWithPositions(
  text: string,
  markers: MarkerInfo[],
  format: FormatType,
  syntaxError?: { line: number; column: number; message: string }
): { monacoMarkers: IMarkerData[]; errorsWithPositions: ErrorWithPosition[] } {
  const monacoMarkers: IMarkerData[] = [];
  const errorsWithPositions: ErrorWithPosition[] = [];

  if (syntaxError) {
    monacoMarkers.push({
      severity: 8,
      message: syntaxError.message,
      startLineNumber: syntaxError.line,
      startColumn: Math.max(1, syntaxError.column - 5),
      endLineNumber: syntaxError.line,
      endColumn: syntaxError.column + 10,
    });
    errorsWithPositions.push({
      message: syntaxError.message,
      line: syntaxError.line,
      column: syntaxError.column,
    });
    return { monacoMarkers, errorsWithPositions };
  }

  for (const marker of markers) {
    const pos = findPathPosition(text, marker.path, format);
    if (pos) {
      monacoMarkers.push({
        severity: 8,
        message: marker.message,
        startLineNumber: pos.startLine,
        startColumn: pos.startCol,
        endLineNumber: pos.endLine,
        endColumn: pos.endCol,
      });
      errorsWithPositions.push({
        message: `${marker.path.join(".")}: ${marker.message}`,
        line: pos.startLine,
        column: pos.startCol,
      });
    } else {
      monacoMarkers.push({
        severity: 8,
        message: `${marker.path.join(".")}: ${marker.message}`,
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: 1,
        endColumn: 2,
      });
      errorsWithPositions.push({
        message: `${marker.path.join(".")}: ${marker.message}`,
        line: 1,
        column: 1,
      });
    }
  }

  return { monacoMarkers, errorsWithPositions };
}

function parseSyntaxErrorPosition(error: Error): { line: number; column: number; message: string } | null {
  const msg = error.message;
  const posMatch = msg.match(/position\s+(\d+)/i);
  if (posMatch) {
    return { line: 1, column: parseInt(posMatch[1], 10), message: msg };
  }
  const lineColMatch = msg.match(/line\s+(\d+)\s+column\s+(\d+)/i);
  if (lineColMatch) {
    return {
      line: parseInt(lineColMatch[1], 10),
      column: parseInt(lineColMatch[2], 10),
      message: msg,
    };
  }
  return null;
}

function parseContent(content: string, format: FormatType): unknown {
  if (format === "yaml") {
    return yaml.load(content);
  }
  return JSON.parse(content);
}

function validateWithSchema<T>(
  content: string,
  format: FormatType,
  schema: z.ZodSchema<T>
): GenericValidationResult<T> {
  let parsed: unknown;
  try {
    parsed = parseContent(content, format);
  } catch (e) {
    let syntaxError: { line: number; column: number; message: string } | undefined;
    
    if (format === "yaml") {
      const yamlError = e as any;
      if (yamlError.mark && typeof yamlError.mark.line === "number") {
        syntaxError = {
          line: yamlError.mark.line + 1,
          column: (yamlError.mark.column || 0) + 1,
          message: yamlError.message || "YAML inválido: erro de sintaxe",
        };
      } else {
        syntaxError = { line: 1, column: 1, message: yamlError.message || "YAML inválido: erro de sintaxe" };
      }
    } else {
      const syntaxInfo = parseSyntaxErrorPosition(e as Error);
      if (syntaxInfo) {
        if (syntaxInfo.line === 1 && syntaxInfo.column > 1) {
          const { line, column } = positionToLineCol(content, syntaxInfo.column);
          syntaxError = { line, column, message: syntaxInfo.message };
        } else {
          syntaxError = syntaxInfo;
        }
      } else {
        syntaxError = { line: 1, column: 1, message: (e as Error).message || "JSON inválido: erro de sintaxe" };
      }
    }
    
    return {
      success: false,
      errors: [format === "yaml" ? "YAML inválido: erro de sintaxe" : "JSON inválido: erro de sintaxe"],
      markers: [],
      syntaxError,
    };
  }

  const result = schema.safeParse(parsed);

  if (result.success) {
    return { success: true, data: result.data };
  }

  const markers: MarkerInfo[] = result.error.issues.map((issue) => ({
    path: issue.path,
    message: issue.message,
  }));

  const errors = result.error.issues.map((issue) => {
    const path = issue.path.join(".");
    return path ? `${path}: ${issue.message}` : issue.message;
  });

  return { success: false, errors, markers };
}

// ============ Component ============

export default function MonacoCodeEditor<T = unknown>({
  value,
  onChange,
  format: controlledFormat,
  onFormatChange,
  readOnly = false,
  height = "100%",
  isDark = false,
  title,
  headerActions,
  buttonBack,
  schema,
  jsonSchema,
  showValidation = false,
  onValidation,
  warningMarkers,
  validationContext,
  className = "",
  showHeader = true,
  showLineNumbers,
  editorInstanceRef,
}: MonacoCodeEditorProps<T>) {
  const [internalFormat, setInternalFormat] = useState<FormatType>(controlledFormat || "json");
  const format = controlledFormat ?? internalFormat;
  
  const [errorsWithPositions, setErrorsWithPositions] = useState<ErrorWithPosition[]>([]);
  const [warningsWithPositions, setWarningsWithPositions] = useState<ErrorWithPosition[]>([]);
  const [isValid, setIsValid] = useState<boolean | null>(null);
  const [editorReady, setEditorReady] = useState(false);
  
  const monacoRef = useRef<Monaco | null>(null);
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const completionDisposableRef = useRef<{ dispose: () => void } | null>(null);
  const validationContextRef = useRef(validationContext);
  const formatRef = useRef(format);

  // Keep validation context ref in sync
  useEffect(() => {
    validationContextRef.current = validationContext;
  }, [validationContext]);

  const prevFormatRef = useRef(format);

  useEffect(() => {
    formatRef.current = format;

    if (prevFormatRef.current === format) return;
    const prevFormat = prevFormatRef.current;
    prevFormatRef.current = format;

    const editor = editorRef.current;
    const mon = monacoRef.current;
    if (!editor || !mon) return;

    const current = editor.getValue();
    if (!current.trim()) return;

    const converted = prevFormat === "json"
      ? jsonToYaml(current)
      : yamlToJson(current);

    const model = editor.getModel();
    if (model) {
      mon.editor.setModelLanguage(model, format === "yaml" ? "yaml" : "json");
    }
    editor.setValue(converted);
    onChange?.(converted);
    validate(converted, format);
  }, [format]);

  // Cleanup completions on unmount
  useEffect(() => {
    return () => {
      completionDisposableRef.current?.dispose();
    };
  }, []);

  const goToErrorLine = (error: ErrorWithPosition) => {
    const editor = editorRef.current;
    if (!editor) return;
    
    editor.revealLineInCenter(error.line);
    editor.setPosition({ lineNumber: error.line, column: error.column });
    editor.focus();
  };

  const validate = useCallback((val: string, fmt: FormatType = format) => {
    if (!schema) {
      setIsValid(null);
      setErrorsWithPositions([]);
      return;
    }

    const result = validateWithSchema(val, fmt, schema);
    
    if (result.success) {
      setErrorsWithPositions([]);
      setIsValid(true);
      onValidation?.(result);
      
      const monaco = monacoRef.current;
      const editor = editorRef.current;
      if (monaco && editor) {
        const model = editor.getModel();
        if (model) {
          monaco.editor.setModelMarkers(model, "schema-validator", []);
        }
      }
    } else {
      const markers = 'markers' in result ? result.markers : [];
      const syntaxError = 'syntaxError' in result ? result.syntaxError : undefined;
      const { monacoMarkers, errorsWithPositions: newErrors } = createMonacoMarkersWithPositions(
        val, markers, fmt, syntaxError
      );
      setErrorsWithPositions(newErrors);
      setIsValid(false);
      onValidation?.(result);
      
      const monaco = monacoRef.current;
      const editor = editorRef.current;
      if (monaco && editor) {
        const model = editor.getModel();
        if (model) {
          monaco.editor.setModelMarkers(model, "schema-validator", monacoMarkers);
        }
      }
    }

    // Template interpolation validation (non-blocking warnings)
    runTemplateValidation(val, fmt);
  }, [format, schema, onValidation]);

  /** Scan string values in parsed pipeline for {{...}} and set warning markers */
  const runTemplateValidation = useCallback((val: string, fmt: FormatType) => {
    const monaco = monacoRef.current;
    const editor = editorRef.current;
    if (!monaco || !editor) return;
    const model = editor.getModel();
    if (!model) return;

    // Extract all {{...}} from the raw text
    const diagnostics = validateInterpolations(val);
    if (diagnostics.length === 0) {
      monaco.editor.setModelMarkers(model, "template-validator", []);
      return;
    }

    const templateMarkers = diagnostics.map((d) => {
      const before = val.substring(0, d.offset);
      const line = (before.match(/\n/g) || []).length + 1;
      const lastNl = before.lastIndexOf("\n");
      const col = d.offset - lastNl;

      const endBefore = val.substring(0, d.offset + d.length);
      const endLine = (endBefore.match(/\n/g) || []).length + 1;
      const endLastNl = endBefore.lastIndexOf("\n");
      const endCol = d.offset + d.length - endLastNl;

      return {
        severity: d.severity === "error" ? 8 : 4,
        message: d.message,
        startLineNumber: line,
        startColumn: col,
        endLineNumber: endLine,
        endColumn: endCol,
      };
    });

    monaco.editor.setModelMarkers(model, "template-validator", templateMarkers);
  }, []);

  const handleEditorMount: OnMount = useCallback((editor, monaco) => {
    monacoRef.current = monaco;
    editorRef.current = editor;
    if (editorInstanceRef) editorInstanceRef.current = editor;
    
    // Configure JSON Schema validation if provided
    if (jsonSchema && format === "json") {
      const model = editor.getModel();
      if (model) {
        monaco.languages.json.jsonDefaults.setDiagnosticsOptions({
          validate: true,
          schemaValidation: "error",
          schemaRequest: "warning",
          schemas: [
            {
              uri: "http://lovable/schema.json",
              fileMatch: [model.uri.toString()],
              schema: jsonSchema,
            },
          ],
        });
      }
    }
    
    // Use centralized template language & theme setup
    setupMonacoTemplateLanguage(monaco, isDark);

    // Register template completions for JSON/YAML
    completionDisposableRef.current?.dispose();
    completionDisposableRef.current = registerTemplateCompletions(
      monaco,
      () => validationContextRef.current,
      ["json", "yaml"]
    );
    
    if (!readOnly) {
      // Auto-format on paste
      editor.onDidPaste(() => {
        const current = editor.getValue();
        const formatted = formatContent(current, formatRef.current);
        if (formatted !== current) {
          editor.setValue(formatted);
          editor.setPosition({ lineNumber: 1, column: 1 });
          onChange?.(formatted);
          validate(formatted);
        }
      });
      
      // Auto-format on blur
      editor.onDidBlurEditorText(() => {
        const current = editor.getValue();
        const formatted = formatContent(current, formatRef.current);
        if (formatted !== current) {
          editor.setValue(formatted);
          onChange?.(formatted);
          validate(formatted);
        }
      });
    }
    
    // Convert content to match current format on mount
    const currentValue = editor.getValue();
    if (currentValue && currentValue.trim()) {
      const looksLikeJson = currentValue.trimStart().startsWith("{") || currentValue.trimStart().startsWith("[");
      const needsConversion = (format === "yaml" && looksLikeJson) || (format === "json" && !looksLikeJson);
      
      if (needsConversion) {
        const converted = format === "yaml" ? jsonToYaml(currentValue) : yamlToJson(currentValue);
        if (converted !== currentValue) {
          const model = editor.getModel();
          if (model) {
            monaco.editor.setModelLanguage(model, format === "yaml" ? "yaml" : "json");
          }
          editor.setValue(converted);
          onChange?.(converted);
        }
      }
    }

    // Run validation after mount
    if (schema) {
      validate(editor.getValue());
    }
    
    // Mark editor as ready for marker-based validation
    setEditorReady(true);
  }, [value, format, validate, readOnly, onChange, schema, jsonSchema, isDark]);

  const handleFormatChange = useCallback((newFormat: FormatType) => {
    if (newFormat === format) return;
    
    const converted = newFormat === "yaml" ? jsonToYaml(value) : yamlToJson(value);
    
    if (onFormatChange) {
      onFormatChange(newFormat);
    } else {
      setInternalFormat(newFormat);
    }
    
    onChange?.(converted);
    
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    if (editor && monaco) {
      const model = editor.getModel();
      if (model) {
        monaco.editor.setModelLanguage(model, newFormat === "yaml" ? "yaml" : "json");
      }
      editor.setValue(converted);
    }
    
    validate(converted, newFormat);
  }, [value, format, validate, onChange, onFormatChange]);

  const handleEditorChange = (val: string | undefined) => {
    const v = val || "";
    onChange?.(v);
    validate(v);
  };

  // Update Monaco theme when isDark changes
  useEffect(() => {
    const monaco = monacoRef.current;
    if (monaco && editorReady) {
      applyMonacoTheme(monaco, isDark);
    }
  }, [isDark, editorReady]);

  // Validate on value/format change (Zod schema)
  useEffect(() => {
    if (schema && monacoRef.current) {
      validate(value, format);
    }
  }, [value, format, schema, validate]);

  // Validation via Monaco markers (when using jsonSchema without Zod schema)
  useEffect(() => {
    if (!showValidation || schema || !jsonSchema || !editorReady) return;
    
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    if (!editor || !monaco) return;
    
    const model = editor.getModel();
    if (!model) return;
    
    const updateFromMarkers = () => {
      const markers = monaco.editor.getModelMarkers({ resource: model.uri });
      const errors = markers
        .filter(m => m.severity === monaco.MarkerSeverity.Error)
        .map(m => ({
          message: m.message,
          line: m.startLineNumber,
          column: m.startColumn,
        }));
      
      setErrorsWithPositions(errors);
      setIsValid(errors.length === 0);
    };
    
    // Listen for marker changes
    const disposable = monaco.editor.onDidChangeMarkers((uris) => {
      const modelUri = model.uri.toString();
      if (!uris.some(uri => uri.toString() === modelUri)) return;
      updateFromMarkers();
    });
    
    // Initial check (with small delay for Monaco to process)
    const timeoutId = setTimeout(updateFromMarkers, 100);
    
    return () => {
      disposable.dispose();
      clearTimeout(timeoutId);
    };
  }, [showValidation, schema, jsonSchema, value, editorReady]);

  // Warning markers effect
  useEffect(() => {
    const monaco = monacoRef.current;
    const editor = editorRef.current;
    if (!monaco || !editor || !editorReady) {
      setWarningsWithPositions([]);
      return;
    }

    if (!warningMarkers || warningMarkers.length === 0) {
      const model = editor.getModel();
      if (model) {
        monaco.editor.setModelMarkers(model, "contract-warnings", []);
      }
      setWarningsWithPositions([]);
      return;
    }

    const model = editor.getModel();
    if (!model) return;

    const warnMonacoMarkers: IMarkerData[] = [];
    const warnPositions: ErrorWithPosition[] = [];

    for (const marker of warningMarkers) {
      const pos = findPathPosition(value, marker.path, format);
      if (pos) {
        warnMonacoMarkers.push({
          severity: 4, // Warning
          message: marker.message,
          startLineNumber: pos.startLine,
          startColumn: pos.startCol,
          endLineNumber: pos.endLine,
          endColumn: pos.endCol,
        });
        warnPositions.push({
          message: marker.message,
          line: pos.startLine,
          column: pos.startCol,
        });
      } else {
        warnMonacoMarkers.push({
          severity: 4,
          message: marker.message,
          startLineNumber: 1,
          startColumn: 1,
          endLineNumber: 1,
          endColumn: 2,
        });
        warnPositions.push({
          message: marker.message,
          line: 1,
          column: 1,
        });
      }
    }

    monaco.editor.setModelMarkers(model, "contract-warnings", warnMonacoMarkers);
    setWarningsWithPositions(warnPositions);
  }, [warningMarkers, value, format, editorReady]);

  const showHeaderSection = showHeader && (title || headerActions || buttonBack || !readOnly);

  return (
    <div className={`flex flex-col overflow-hidden text-card-foreground ${className}`}>
      {/* Header */}
      {showHeaderSection && (
        <div className="flex items-center justify-between px-4 py-2 ">
          <div className="flex items-center gap-4">
            {buttonBack && (
              <Button variant="ghost" size="icon" onClick={buttonBack} className="h-7 w-7">
                <ArrowLeft className="h-4 w-4" />
              </Button>
            )}
            {title && <h2 className="text-sm font-semibold">{title}</h2>}
            
          </div>
          {headerActions && <div>{headerActions}</div>}
        </div>
      )}

      {/* Monaco Editor */}
      <div className="flex-1 min-h-0" style={{ height: showHeaderSection || showValidation ? undefined : height }}>
        <Suspense fallback={<div className="flex h-full items-center justify-center text-muted-foreground text-sm">Carregando editor...</div>}>
          <MonacoEditorLazy
            height={height}
            language={format}
            value={value}
            onChange={readOnly ? undefined : handleEditorChange}
            onMount={handleEditorMount}
            theme={isDark ? "transparent-dark" : "transparent-light"}
            options={{
              readOnly,
              minimap: { enabled: false },
              fontSize: 13,
              lineNumbers: showLineNumbers ?? (readOnly ? "off" : "on"),
              scrollBeyondLastLine: false,
              automaticLayout: true,
              tabSize: 2,
              wordWrap: "on",
              folding: !readOnly,
              lineDecorationsWidth: (readOnly && !showLineNumbers) ? 0 : undefined,
              lineNumbersMinChars: (readOnly && !showLineNumbers) ? 0 : undefined,
              glyphMargin: !readOnly,
              padding: readOnly ? { top: 8, bottom: 8 } : undefined,
              quickSuggestions: {
                strings: true,
                comments: false,
                other: true,
              },
              suggestOnTriggerCharacters: true,
            }}
          />
        </Suspense>
      </div>

      {/* Validation Footer */}
      {showValidation && (
        <ValidationFooter
          isValid={errorsWithPositions.length > 0 ? false : isValid}
          errors={errorsWithPositions}
          warnings={warningsWithPositions}
          format={format}
          onErrorClick={goToErrorLine}
        />
      )}
    </div>
  );
}
