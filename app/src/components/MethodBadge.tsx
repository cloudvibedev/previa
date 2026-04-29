import { Badge } from "@/components/ui/badge";
import { METHOD_COLORS } from "@/lib/constants";

interface MethodBadgeProps {
  method: string;
  className?: string;
}

export function MethodBadge({ method, className = "" }: MethodBadgeProps) {
  return (
    <Badge className={`${METHOD_COLORS[method] || "bg-muted"} border-0 hover:bg-none pointer-events-none ${className}`}>
      {method}
    </Badge>
  );
}
