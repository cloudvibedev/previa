import type { MouseEvent, ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export interface SidebarItemAction {
  label: string;
  icon: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}

interface SidebarItemActionBarProps {
  label: string;
  actions: SidebarItemAction[];
  className?: string;
}

export function SidebarItemActionBar({ label, actions, className }: SidebarItemActionBarProps) {
  if (actions.length === 0) return null;

  const handleClick = (event: MouseEvent<HTMLDivElement>) => {
    event.stopPropagation();
  };

  return (
    <div
      aria-label={label}
      className={cn(
        "glass absolute right-2 top-1/2 z-10 flex -translate-y-1/2 items-center gap-1 rounded-md border-border/20 p-2 opacity-0 transition-opacity group-hover:opacity-100",
        className,
      )}
      onClick={handleClick}
    >
      {actions.map((action) => (
        <Button
          key={action.label}
          variant="ghost"
          size="icon"
          className="h-5 min-w-5 w-5 shrink-0"
          onClick={action.onClick}
          title={action.label}
          disabled={action.disabled}
        >
          {action.icon}
        </Button>
      ))}
    </div>
  );
}
