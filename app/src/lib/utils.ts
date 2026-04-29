import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function toDisplayText(value: unknown): string {
  if (typeof value === "string") return value;
  if (value === null || value === undefined) return "";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (typeof value === "object" && value && "default" in (value as any)) {
    return toDisplayText((value as any).default);
  }
  try {
    return JSON.stringify(value);
  } catch {
    return "[Object]";
  }
}
