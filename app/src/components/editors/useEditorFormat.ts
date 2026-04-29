import { useState, useCallback } from "react";
import yaml from "js-yaml";
import type { FormatType } from "@/lib/pipeline-schema";

export interface UseEditorFormatOptions {
  initialContent?: string;
  initialFormat?: FormatType;
}

export interface UseEditorFormatReturn {
  content: string;
  setContent: (value: string) => void;
  format: FormatType;
  setFormat: (format: FormatType) => void;
  /** Import raw content and auto-detect format */
  importContent: (raw: string) => void;
  /** Convert current content to JSON string */
  toJson: () => string;
  /** Convert current content to YAML string */
  toYaml: () => string;
}

function detectFormat(content: string): FormatType {
  const trimmed = content.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    return "json";
  }
  return "yaml";
}

function parseAny(content: string): unknown {
  const trimmed = content.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    return JSON.parse(content);
  }
  return yaml.load(content);
}

export function useEditorFormat(options: UseEditorFormatOptions = {}): UseEditorFormatReturn {
  const { initialContent = "{}", initialFormat = "json" } = options;
  
  const [content, setContent] = useState<string>(initialContent);
  const [format, setFormat] = useState<FormatType>(initialFormat);

  const importContent = useCallback((raw: string) => {
    const detectedFormat = detectFormat(raw);
    setFormat(detectedFormat);
    setContent(raw);
  }, []);

  const toJson = useCallback((): string => {
    try {
      const parsed = parseAny(content);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return content;
    }
  }, [content]);

  const toYaml = useCallback((): string => {
    try {
      const parsed = parseAny(content);
      return yaml.dump(parsed, { indent: 2, lineWidth: -1 });
    } catch {
      return content;
    }
  }, [content]);

  return {
    content,
    setContent,
    format,
    setFormat,
    importContent,
    toJson,
    toYaml,
  };
}
