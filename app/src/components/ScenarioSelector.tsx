import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Wand2 } from "lucide-react";

export interface Scenario {
  id: string;
  category: string;
  title: string;
  description: string;
}

interface ScenarioSelectorProps {
  scenarios: Scenario[];
  onGenerate: (selected: Scenario[]) => void;
  disabled?: boolean;
}

export function ScenarioSelector({ scenarios, onGenerate, disabled }: ScenarioSelectorProps) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const grouped = useMemo(() => {
    const map = new Map<string, Scenario[]>();
    for (const s of scenarios) {
      const list = map.get(s.category) || [];
      list.push(s);
      map.set(s.category, list);
    }
    return map;
  }, [scenarios]);

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleCategory = (category: string) => {
    const ids = grouped.get(category)?.map((s) => s.id) || [];
    setSelected((prev) => {
      const next = new Set(prev);
      const allSelected = ids.every((id) => next.has(id));
      for (const id of ids) {
        if (allSelected) next.delete(id);
        else next.add(id);
      }
      return next;
    });
  };

  const handleGenerate = () => {
    const chosen = scenarios.filter((s) => selected.has(s.id));
    if (chosen.length > 0) onGenerate(chosen);
  };

  return (
    <div className="space-y-3 w-full">
      {Array.from(grouped.entries()).map(([category, items]) => {
        const allSelected = items.every((s) => selected.has(s.id));
        const someSelected = items.some((s) => selected.has(s.id));

        return (
          <div key={category} className="space-y-1.5">
            <div className="flex items-center gap-2 px-1">
              <Checkbox
                checked={allSelected}
                ref={undefined}
                onCheckedChange={() => toggleCategory(category)}
                className="h-3.5 w-3.5"
                data-state={someSelected && !allSelected ? "indeterminate" : undefined}
              />
              <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                {category}
              </span>
            </div>
            <div className="grid gap-1.5">
              {items.map((scenario) => {
                const isSelected = selected.has(scenario.id);
                return (
                  <button
                    key={scenario.id}
                    type="button"
                    onClick={() => toggle(scenario.id)}
                    className={`flex items-start gap-2.5 rounded-lg border px-3 py-2.5 text-left transition-all duration-150 cursor-pointer ${
                      isSelected
                        ? "border-primary/50 bg-primary/5"
                        : "border-border/40 bg-card/40 hover:border-border/60 hover:bg-card/60"
                    }`}
                  >
                    <Checkbox
                      checked={isSelected}
                      onCheckedChange={() => toggle(scenario.id)}
                      className="mt-0.5 h-3.5 w-3.5 shrink-0"
                      onClick={(e) => e.stopPropagation()}
                    />
                    <div className="min-w-0">
                      <p className="text-sm font-medium leading-tight">{scenario.title}</p>
                      <p className="text-xs text-muted-foreground mt-0.5 leading-snug">
                        {scenario.description}
                      </p>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        );
      })}

      {selected.size > 0 && (
        <Button
          size="sm"
          className="w-full gap-1.5 mt-2"
          onClick={handleGenerate}
          disabled={disabled}
        >
          <Wand2 className="h-3.5 w-3.5" />
          {t("scenarioSelector.generate", { count: selected.size })}
        </Button>
      )}
    </div>
  );
}
