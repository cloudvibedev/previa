import type { ReactNode } from "react";
import { ArrowDown } from "lucide-react";

interface StepFlowListProps {
  items: { key: string; content: ReactNode }[];
}

export function StepFlowList({ items }: StepFlowListProps) {
  return (
    <div className="space-y-3">
      {items.map((item, i) => (
        <div key={item.key} className="animate-fade-in" style={{ animationDelay: `${i * 60}ms`, opacity: 0 }}>
          {item.content}
          {i < items.length - 1 && (
            <div className="flex justify-center py-1">
              <ArrowDown className="h-4 w-4 text-muted-foreground" />
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
