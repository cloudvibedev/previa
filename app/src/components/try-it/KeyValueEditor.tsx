import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Plus, Trash2 } from "lucide-react";
import type { KeyValue } from "./types";

interface KeyValueEditorProps {
  label: string;
  items: KeyValue[];
  onChange: (items: KeyValue[]) => void;
  keyReadOnly?: boolean;
  keyPlaceholder?: string;
  valuePlaceholder?: string;
  showAdd?: boolean;
  showDelete?: boolean;
  keyWidth?: string;
}

export function KeyValueEditor({
  label,
  items,
  onChange,
  keyReadOnly = false,
  keyPlaceholder = "key",
  valuePlaceholder = "value",
  showAdd = false,
  showDelete = false,
  keyWidth = "w-28",
}: KeyValueEditorProps) {
  const { t } = useTranslation();

  const updateItem = (index: number, field: "key" | "value", val: string) => {
    const updated = [...items];
    updated[index] = { ...items[index], [field]: val };
    onChange(updated);
  };

  const removeItem = (index: number) => onChange(items.filter((_, i) => i !== index));
  const addItem = () => onChange([...items, { key: "", value: "" }]);

  return (
    <div className="px-3 py-2 border-border/20">
      <span className="text-[10px] font-semibold uppercase text-muted-foreground tracking-wider">{label}</span>
      <div className="mt-1.5 space-y-1">
        {items.map((item, i) => (
          <div key={i} className="flex items-center gap-2">
            <Input
              placeholder={keyPlaceholder}
              value={item.key}
              readOnly={keyReadOnly}
              onChange={(e) => updateItem(i, "key", e.target.value)}
              className={`${keyWidth} h-7 text-xs font-mono ${keyReadOnly ? "bg-muted/30" : ""}`}
            />
            <Input
              placeholder={valuePlaceholder}
              value={item.value}
              onChange={(e) => updateItem(i, "value", e.target.value)}
              className="flex-1 h-7 text-xs font-mono"
            />
            {showDelete && (
              <Button variant="ghost" size="icon" className="h-6 w-6 shrink-0" onClick={() => removeItem(i)}>
                <Trash2 className="h-3 w-3" />
              </Button>
            )}
          </div>
        ))}
        {showAdd && (
          <Button variant="ghost" size="sm" className="h-6 text-xs gap-1" onClick={addItem}>
            <Plus className="h-3 w-3" /> {t("common.add")}
          </Button>
        )}
      </div>
    </div>
  );
}
