import { cn } from "@/lib/utils";

interface DotsLoaderProps {
  className?: string;
}

export function DotsLoader({ className }: DotsLoaderProps) {
  return (
    <span className={cn("dots-loader text-muted-foreground", className)}>
      <span />
      <span />
      <span />
    </span>
  );
}
