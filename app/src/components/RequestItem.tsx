import { useState, useCallback } from "react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Plus, Trash2 } from "lucide-react";
import { MonacoInput } from "@/components/MonacoInput";
import type { TemplateValidationContext } from "@/lib/template-validator";

interface RequestItemProps {
  name: string;
  value: string;
  onChange: (value: string) => void;
  onNameChange?: (name: string) => void;
  onRemove?: () => void;
  placeholder?: string;
  required?: boolean;
  type?: string;
  description?: string;
  enum?: string[];
  format?: string;
  pattern?: string;
  validationContext?: TemplateValidationContext;
}

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const URL_RE = /^https?:\/\/.+/;
const INTERPOLATION_RE = /\{\{.+?\}\}/;

function validate(
  value: string,
  format?: string,
  pattern?: string,
): string | null {
  if (!value || INTERPOLATION_RE.test(value)) return null; // skip empty or interpolated values
  if (format === "email" && !EMAIL_RE.test(value)) return "Email inválido";
  if ((format === "uri" || format === "url") && !URL_RE.test(value)) return "URL inválida (deve iniciar com http:// ou https://)";
  if (pattern) {
    try {
      if (!new RegExp(pattern).test(value)) return `Não corresponde ao padrão: ${pattern}`;
    } catch {
      // invalid regex in schema, skip
    }
  }
  return null;
}

function ValueInput({
  value,
  onChange,
  placeholder,
  type,
  enumValues,
  format,
  pattern,
  validationContext,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: string;
  enumValues?: string[];
  format?: string;
  pattern?: string;
  validationContext?: TemplateValidationContext;
}) {
  const [touched, setTouched] = useState(false);
  const error = touched ? validate(value, format, pattern) : null;

  const handleBlur = useCallback(() => setTouched(true), []);

  // Enum select
  if (enumValues && enumValues.length > 0) {
    return (
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger className="h-8 flex-1 font-mono text-xs">
          <SelectValue placeholder={placeholder ?? "Select..."} />
        </SelectTrigger>
        <SelectContent>
          {enumValues.map((opt) => (
            <SelectItem key={opt} value={opt}>
              {opt}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  // Boolean select
  if (type === "boolean") {
    return (
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger className="h-8 flex-1 font-mono text-xs">
          <SelectValue placeholder={placeholder ?? "true / false"} />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="true">true</SelectItem>
          <SelectItem value="false">false</SelectItem>
        </SelectContent>
      </Select>
    );
  }

  // Number input
  if (type === "integer" || type === "number") {
    return (
      <Input
        type="number"
        placeholder={placeholder ?? `Valor de ${type}`}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="h-8 flex-1 font-mono text-xs"
      />
    );
  }

  // Default text input with Monaco for template highlighting
  const ph =
    type === "object"
      ? "JSON object"
      : type === "array"
        ? "JSON array"
        : format === "email"
          ? "user@example.com"
          : format === "uri" || format === "url"
            ? "https://..."
            : placeholder;

  return (
    <div className="flex-1 space-y-0.5">
      <MonacoInput
        placeholder={ph}
        value={value}
        onChange={(v) => {
          onChange(v);
          if (!touched) setTouched(true);
        }}
        className={`h-8 ${error ? "border-destructive" : ""}`}
        validationContext={validationContext}
      />
      {error && (
        <p className="text-[10px] text-destructive">{error}</p>
      )}
    </div>
  );
}

export function RequestItem({
  name,
  value,
  onChange,
  onNameChange,
  onRemove,
  placeholder,
  required,
  type,
  description,
  enum: enumValues,
  format,
  pattern,
  validationContext,
}: RequestItemProps) {
  return (
    <div className="flex items-center gap-2">
      <Input
        placeholder="Key"
        value={name}
        onChange={onNameChange ? (e) => onNameChange(e.target.value) : undefined}
        readOnly={!onNameChange}
        className={`h-8 flex-1 font-mono text-xs ${!onNameChange ? "bg-muted cursor-default" : ""}`}
      />
      <ValueInput
        value={value}
        onChange={onChange}
        placeholder={placeholder ?? description ?? `Valor de ${name}`}
        type={type}
        enumValues={enumValues}
        format={format}
        pattern={pattern}
        validationContext={validationContext}
      />
      {onRemove ? (
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8 shrink-0"
          onClick={onRemove}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      ) : (
        <div className="h-8 w-8 shrink-0" />
      )}
    </div>
  );
}

interface RequestSectionProps {
  title: string;
  items: Array<{
    name: string;
    value: string;
    required?: boolean;
    type?: string;
    description?: string;
    enum?: string[];
    format?: string;
    pattern?: string;
  }>;
  onChange: (name: string, value: string, index?: number) => void;
  onNameChange?: (oldName: string, newName: string, index?: number) => void;
  onRemove?: (name: string, index?: number) => void;
  onAdd?: () => void;
  validationContext?: TemplateValidationContext;
}

export function RequestSection({
  title,
  items,
  onChange,
  onNameChange,
  onRemove,
  onAdd,
  validationContext,
}: RequestSectionProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <Label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {title}
        </Label>
        {onAdd && (
          <Button variant="ghost" size="sm" onClick={onAdd} className="h-7 gap-1 text-xs">
            <Plus className="h-3 w-3" /> Add
          </Button>
        )}
      </div>
      {items.length > 0 && (
        <div className="space-y-2 rounded-md border p-3">
          {items.map((item, i) => (
            <RequestItem
              key={onNameChange ? i : item.name}
              name={item.name}
              value={item.value}
              onChange={(v) => onChange(item.name, v, i)}
              onNameChange={onNameChange ? (newName) => onNameChange(item.name, newName, i) : undefined}
              onRemove={onRemove && !item.required ? () => onRemove(item.name, i) : undefined}
              required={item.required}
              type={item.type}
              description={item.description}
              enum={item.enum}
              format={item.format}
              pattern={item.pattern}
              validationContext={validationContext}
            />
          ))}
        </div>
      )}
    </div>
  );
}