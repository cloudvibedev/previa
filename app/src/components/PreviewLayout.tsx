import type { ReactNode } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";

interface PreviewLayoutProps {
  title: string;
  subtitle?: string;
  leftContent?: ReactNode;
  rightContent?: ReactNode;
  children: ReactNode;
  buttonContent?: ReactNode;
  onButtonClick?: () => void;
  buttonDisabled?: boolean;
}

export function PreviewLayout({
  title,
  subtitle,
  leftContent,
  rightContent,
  children,
  buttonContent,
  onButtonClick,
  buttonDisabled,
}: PreviewLayoutProps) {
  const showFooter = buttonContent || onButtonClick;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-center justify-between border-border/50 px-4 py-3">
        <div className="flex items-center gap-3">
          {leftContent}
          <div>
            <h2 className="text-lg font-semibold">{title}</h2>
            {subtitle && <span className="text-sm text-muted-foreground">{subtitle}</span>}
          </div>
        </div>
        {rightContent}
      </div>

      <ScrollArea className="flex-1 min-h-0 min-w-0">{children}</ScrollArea>

      {showFooter && (
        <div className="border-border/50 p-4">
          <Button onClick={onButtonClick} className="w-full" disabled={buttonDisabled}>
            {buttonContent}
          </Button>
        </div>
      )}
    </div>
  );
}
