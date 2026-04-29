import type { ReactNode } from "react";

interface SectionHeaderProps {
  title: string;
  children?: ReactNode;
}

export function SectionHeader({ title, children }: SectionHeaderProps) {
  return (
    <div className="flex items-center justify-between">
      <h2 className="text-sm font-semibold">{title}</h2>
      {children && <div className="flex gap-0.5">{children}</div>}
    </div>
  );
}
