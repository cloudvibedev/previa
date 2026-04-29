import { Button } from "@/components/ui/button";
import { useTranslation } from "react-i18next";

const LANG_CYCLE = ["en", "es", "fr", "de", "pt-BR", "ja", "ko", "zh-CN"] as const;
const LANG_LABELS: Record<string, string> = {
  en: "EN",
  es: "ES",
  fr: "FR",
  de: "DE",
  "pt-BR": "PT",
  ja: "JA",
  ko: "KO",
  "zh-CN": "ZH",
};

export function LanguageToggle() {
  const { i18n } = useTranslation();
  const currentLang = i18n.language;

  const toggle = () => {
    const idx = LANG_CYCLE.indexOf(currentLang as any);
    const next = LANG_CYCLE[(idx + 1) % LANG_CYCLE.length];
    i18n.changeLanguage(next);
  };

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={toggle}
      className="h-8 w-8 text-xs font-bold"
      title="Switch language"
    >
      {LANG_LABELS[currentLang] || "EN"}
    </Button>
  );
}
