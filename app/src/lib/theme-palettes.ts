export type PaletteId =
  | "default" | "ocean" | "forest" | "sunset" | "rose"
  | "one-dark" | "dracula" | "monokai" | "material" | "night-owl" | "github" | "nord" | "catppuccin"
  | "high-contrast-dark" | "high-contrast-light" | "gruvbox" | "solarized" | "monochrome";

export interface PaletteInfo {
  id: PaletteId;
  label: string;
  primaryPreview: string;
  secondaryPreview: string;
  complex?: boolean;
  bgPreview?: string;
  forcedTheme?: "dark" | "light";
}

export const PALETTES: PaletteInfo[] = [
  { id: "default", label: "Default",  primaryPreview: "#006fee", secondaryPreview: "#7828c8", bgPreview: "#1a1a2e" },
  { id: "ocean",   label: "Ocean",    primaryPreview: "#0891b2", secondaryPreview: "#0d9488", bgPreview: "#0f1f2e" },
  { id: "forest",  label: "Forest",   primaryPreview: "#16a34a", secondaryPreview: "#65a30d", bgPreview: "#0f1f14" },
  { id: "sunset",  label: "Sunset",   primaryPreview: "#f97316", secondaryPreview: "#e11d48", bgPreview: "#1f150f" },
  { id: "rose",    label: "Rose",     primaryPreview: "#e11d55", secondaryPreview: "#8b5cf6", bgPreview: "#1f0f1a" },
];

export const COMPLEX_PALETTES: PaletteInfo[] = [
  { id: "one-dark",  label: "One Dark Pro",    primaryPreview: "#61afef", secondaryPreview: "#c678dd", complex: true, bgPreview: "#282c34" },
  { id: "dracula",   label: "Dracula",         primaryPreview: "#bd93f9", secondaryPreview: "#ff79c6", complex: true, bgPreview: "#282a36" },
  { id: "monokai",   label: "Monokai Pro",     primaryPreview: "#ffd866", secondaryPreview: "#ab9df2", complex: true, bgPreview: "#2d2a2e" },
  { id: "material",  label: "Material Theme",  primaryPreview: "#89ddff", secondaryPreview: "#c792ea", complex: true, bgPreview: "#263238" },
  { id: "night-owl", label: "Night Owl",       primaryPreview: "#82aaff", secondaryPreview: "#c792ea", complex: true, bgPreview: "#011627" },
  { id: "github",    label: "GitHub Dark",     primaryPreview: "#58a6ff", secondaryPreview: "#a371f7", complex: true, bgPreview: "#0d1117" },
  { id: "nord",      label: "Nord",            primaryPreview: "#88c0d0", secondaryPreview: "#81a1c1", complex: true, bgPreview: "#2e3440" },
  { id: "catppuccin", label: "Catppuccin Mocha", primaryPreview: "#89b4fa", secondaryPreview: "#cba6f7", complex: true, bgPreview: "#1e1e2e" },
  { id: "high-contrast-dark",  label: "Alto Contraste Dark",  primaryPreview: "#ffffff", secondaryPreview: "#ffd700", complex: true, bgPreview: "#000000" },
  { id: "high-contrast-light", label: "Alto Contraste Light", primaryPreview: "#0000cc", secondaryPreview: "#6600cc", complex: true, bgPreview: "#ffffff", forcedTheme: "light" as any },
  { id: "gruvbox",   label: "Gruvbox",         primaryPreview: "#fe8019", secondaryPreview: "#fabd2f", complex: true, bgPreview: "#282828" },
  { id: "solarized", label: "Solarized Dark",  primaryPreview: "#268bd2", secondaryPreview: "#2aa198", complex: true, bgPreview: "#002b36" },
  { id: "monochrome", label: "Monochrome",    primaryPreview: "#a0a0a0", secondaryPreview: "#d4d4d4", complex: true, bgPreview: "#141414" },
];

export const ALL_PALETTES = [...PALETTES, ...COMPLEX_PALETTES];

export function isComplexPalette(id: PaletteId): boolean {
  return COMPLEX_PALETTES.some((p) => p.id === id);
}
