import { useState } from "react";
import { cn } from "@/lib/utils";
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from "@/components/ui/resizable";
import { useIsMobile } from "@/hooks/use-mobile";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

interface SplitPaneLayoutProps {
  leftPanel: React.ReactNode;
  rightPanel: React.ReactNode;
  leftDefaultSize?: number;
  rightDefaultSize?: number;
  leftMinSize?: number;
  rightMinSize?: number;
  className?: string;
  withPadding?: boolean;
  withBorder?: boolean;
  mobileLeftLabel?: string;
  mobileRightLabel?: string;
  autoSaveId?: string;
}

export function SplitPaneLayout({
  leftPanel,
  rightPanel,
  leftDefaultSize = 50,
  rightDefaultSize = 50,
  leftMinSize = 20,
  rightMinSize = 20,
  className,
  withPadding = true,
  withBorder = true,
  mobileLeftLabel = "Editor",
  mobileRightLabel = "Preview",
  autoSaveId,
}: SplitPaneLayoutProps) {
  const isMobile = useIsMobile();

  if (isMobile) {
    return (
      <Tabs defaultValue="left" className={cn("flex flex-1 flex-col overflow-hidden", className)}>
        <TabsContent value="left" className="flex-1 overflow-hidden m-0">
          {leftPanel}
        </TabsContent>
        <TabsContent value="right" className="flex-1 overflow-hidden m-0">
          {rightPanel}
        </TabsContent>
        <div className="">
          <TabsList className="mx-2 my-2 grid w-auto grid-cols-2">
            <TabsTrigger value="left">{mobileLeftLabel}</TabsTrigger>
            <TabsTrigger value="right">{mobileRightLabel}</TabsTrigger>
          </TabsList>
        </div>
      </Tabs>
    );
  }

  return (
    <div className={cn(
      "flex h-full min-h-0 flex-1 overflow-hidden",
      withPadding && "p-6",
      className
    )}>
      <ResizablePanelGroup
        direction="horizontal"
        autoSaveId={autoSaveId}
        className={cn("h-full min-h-0 flex-1", withBorder && "rounded-lg border")}
      >
        <ResizablePanel className="min-h-0 min-w-0 overflow-hidden" defaultSize={leftDefaultSize} minSize={leftMinSize}>
          {leftPanel}
        </ResizablePanel>

        <ResizableHandle />

        <ResizablePanel className="min-h-0 min-w-0 overflow-hidden" defaultSize={rightDefaultSize} minSize={rightMinSize}>
          {rightPanel}
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
