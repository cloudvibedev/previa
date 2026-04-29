import type { LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ReactNode } from "react";

interface EmptyStateAction {
  label: string;
  icon?: ReactNode;
  onClick: () => void;
}

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description: string;
  action?: EmptyStateAction;
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 text-center px-6 animate-fade-in">
      <div className="rounded-2xl bg-gradient-to-br from-primary/10 to-primary/5 p-5 border border-border/30 shadow-primary-glow animate-scale-in">
        <Icon className="h-10 w-10 text-primary/70" />
      </div>
      <div>
        <h3 className="text-lg font-semibold">{title}</h3>
        <p className="mt-1 text-sm text-muted-foreground">{description}</p>
      </div>
      {action && (
        <Button onClick={action.onClick} size="lg" className="mt-2 shadow-primary-button">
          {action.icon}
          {action.label}
        </Button>
      )}
    </div>
  );
}
