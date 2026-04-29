import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Plus, Globe } from "lucide-react";

export interface SpecSuggestion {
  id: string;
  name: string;
  description: string;
  routes_preview: string;
}

interface SpecSuggestionSelectorProps {
  suggestions: SpecSuggestion[];
  onGenerate: (selected: SpecSuggestion[]) => void;
  disabled?: boolean;
}

export function SpecSuggestionSelector({ suggestions, onGenerate, disabled }: SpecSuggestionSelectorProps) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleGenerate = () => {
    const chosen = suggestions.filter((s) => selected.has(s.id));
    if (chosen.length > 0) onGenerate(chosen);
  };

  return (
    <div className="space-y-2 w-full">
      <div className="grid gap-1.5">
        {suggestions.map((spec) => {
          const isSelected = selected.has(spec.id);
          return (
            <button
              key={spec.id}
              type="button"
              onClick={() => toggle(spec.id)}
              className={`flex items-start gap-2.5 rounded-lg border px-3 py-2.5 text-left transition-all duration-150 cursor-pointer ${
                isSelected
                  ? "border-primary/50 bg-primary/5"
                  : "border-border/40 bg-card/40 hover:border-border/60 hover:bg-card/60"
              }`}
            >
              <Checkbox
                checked={isSelected}
                onCheckedChange={() => toggle(spec.id)}
                className="mt-0.5 h-3.5 w-3.5 shrink-0"
                onClick={(e) => e.stopPropagation()}
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <Globe className="h-3.5 w-3.5 text-primary shrink-0" />
                  <p className="text-sm font-medium leading-tight">{spec.name}</p>
                </div>
                <p className="text-xs text-muted-foreground mt-0.5 leading-snug">
                  {spec.description}
                </p>
                <p className="text-[11px] text-muted-foreground/70 mt-1 font-mono leading-snug">
                  {spec.routes_preview}
                </p>
              </div>
            </button>
          );
        })}
      </div>

      {selected.size > 0 && (
        <Button
          size="sm"
          className="w-full gap-1.5 mt-2"
          onClick={handleGenerate}
          disabled={disabled}
        >
          <Plus className="h-3.5 w-3.5" />
          {t("specSuggestion.create", { count: selected.size })}
        </Button>
      )}
    </div>
  );
}
