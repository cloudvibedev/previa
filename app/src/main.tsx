import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import "./index.css";
import "./i18n";
import { getGlassLevel, getPalette, getTheme } from "@/lib/ui-preferences";

// Initialize browser-local UI preferences before React mounts.
const palette = getPalette();
if (palette !== "default") {
  document.documentElement.dataset.palette = palette;
}

// Initialize theme — complex palettes force specific mode
const COMPLEX_DARK_IDS = ["one-dark", "dracula", "monokai", "material", "night-owl", "github", "nord", "catppuccin", "high-contrast-dark", "gruvbox", "solarized", "monochrome"];
const COMPLEX_LIGHT_IDS = ["high-contrast-light"];
const isDark = COMPLEX_DARK_IDS.includes(palette)
  ? true
  : COMPLEX_LIGHT_IDS.includes(palette)
    ? false
    : getTheme() === "dark";
document.documentElement.classList.toggle("dark", isDark);

// Initialize glass level
const glassLevel = getGlassLevel();
if (glassLevel > 0) {
  document.documentElement.dataset.glassLevel = String(glassLevel);
}

createRoot(document.getElementById("root")!).render(<App />);
