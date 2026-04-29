import { create } from "zustand";
import { getTheme, setTheme as persistTheme, getPalette, setPalette as persistPalette, getGlassLevel, setGlassLevel as persistGlassLevel } from "@/lib/ui-preferences";
import type { PaletteId } from "@/lib/theme-palettes";
import { isComplexPalette, COMPLEX_PALETTES } from "@/lib/theme-palettes";

function applyGlassLevel(level: number) {
  if (level > 0) {
    document.documentElement.dataset.glassLevel = String(level);
  } else {
    delete document.documentElement.dataset.glassLevel;
  }
}

interface ThemeState {
  theme: "dark" | "light";
  palette: PaletteId;
  glassLevel: number;
  setTheme: (theme: "dark" | "light") => void;
  toggleTheme: () => void;
  isDark: () => boolean;
  setPalette: (palette: PaletteId) => void;
  isComplex: () => boolean;
  setGlassLevel: (level: number) => void;
}

export const useThemeStore = create<ThemeState>((set, get) => ({
  theme: getTheme(),
  palette: getPalette() as PaletteId,
  glassLevel: getGlassLevel(),

  setTheme: (theme) => {
    if (isComplexPalette(get().palette)) return;
    persistTheme(theme);
    document.documentElement.classList.toggle("dark", theme === "dark");
    set({ theme });
  },

  toggleTheme: () => {
    if (isComplexPalette(get().palette)) return;
    const next = get().theme === "dark" ? "light" : "dark";
    get().setTheme(next);
  },

  isDark: () => get().theme === "dark",
  isComplex: () => isComplexPalette(get().palette),

  setGlassLevel: (level) => {
    persistGlassLevel(level);
    applyGlassLevel(level);
    set({ glassLevel: level });
  },

  setPalette: (palette) => {
    persistPalette(palette);

    if (palette === "default") {
      delete document.documentElement.dataset.palette;
    } else {
      document.documentElement.dataset.palette = palette;
    }

    if (isComplexPalette(palette)) {
      const paletteInfo = COMPLEX_PALETTES.find((p) => p.id === palette);
      const forced = paletteInfo?.forcedTheme ?? "dark";
      document.documentElement.classList.toggle("dark", forced === "dark");
      set({ palette, theme: forced });
    } else {
      const savedTheme = getTheme();
      document.documentElement.classList.toggle("dark", savedTheme === "dark");
      set({ palette, theme: savedTheme });
    }
  },
}));
