import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, Download, MonitorSmartphone } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type BeforeInstallPromptEvent = Event & {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed"; platform: string }>;
};

function isStandaloneMode() {
  return window.matchMedia("(display-mode: standalone)").matches || ("standalone" in navigator && Boolean((navigator as Navigator & { standalone?: boolean }).standalone));
}

function detectInstallContext() {
  const userAgent = window.navigator.userAgent.toLowerCase();
  const isIOS = /iphone|ipad|ipod/.test(userAgent);
  const isAndroid = /android/.test(userAgent);
  const isChrome = /chrome|chromium|crios/.test(userAgent) && !/edg|opr|opera/.test(userAgent);
  const isSafari = /safari/.test(userAgent) && !/chrome|chromium|crios|android/.test(userAgent);

  if (isIOS && isSafari) {
    return {
      title: "Instalar no iPhone/iPad",
      description: "No Safari, a instalação é manual.",
      steps: [
        "Toque no botão Compartilhar do Safari.",
        "Escolha “Adicionar à Tela de Início”.",
        "Confirme para instalar o app no seu aparelho.",
      ],
    };
  }

  if (isAndroid && isChrome) {
    return {
      title: "Instalar no Chrome",
      description: "Se o prompt não abrir automaticamente, você ainda pode instalar pelo menu do navegador.",
      steps: [
        "Abra o menu de três pontos do Chrome.",
        "Toque em “Instalar app” ou “Adicionar à tela inicial”.",
        "Confirme para baixar como app.",
      ],
    };
  }

  if (isChrome) {
    return {
      title: "Instalar no Chrome",
      description: "Se o prompt automático não aparecer, use a opção manual do navegador.",
      steps: [
        "Clique no menu de três pontos do Chrome.",
        "Procure por “Instalar app” ou pelo ícone de instalar na barra de endereço.",
        "Confirme a instalação.",
      ],
    };
  }

  return {
    title: "Instalar este app",
    description: "Seu navegador pode não oferecer o prompt automático, mas alguns navegadores permitem instalar pelo menu.",
    steps: [
      "Abra o menu do navegador atual.",
      "Procure por uma opção como “Instalar app”, “Adicionar à tela inicial” ou equivalente.",
      "Se a opção não aparecer, este navegador provavelmente não suporta a instalação automática deste PWA.",
    ],
  };
}

export function InstallAppButton() {
  const [deferredPrompt, setDeferredPrompt] = useState<BeforeInstallPromptEvent | null>(null);
  const [isInstalled, setIsInstalled] = useState(false);
  const [isDialogOpen, setIsDialogOpen] = useState(false);

  useEffect(() => {
    setIsInstalled(isStandaloneMode());

    const handleBeforeInstallPrompt = (event: Event) => {
      event.preventDefault();
      setDeferredPrompt(event as BeforeInstallPromptEvent);
    };

    const handleInstalled = () => {
      setIsInstalled(true);
      setDeferredPrompt(null);
      setIsDialogOpen(false);
    };

    const mediaQuery = window.matchMedia("(display-mode: standalone)");
    const handleDisplayModeChange = () => setIsInstalled(mediaQuery.matches || isStandaloneMode());

    window.addEventListener("beforeinstallprompt", handleBeforeInstallPrompt);
    window.addEventListener("appinstalled", handleInstalled);
    mediaQuery.addEventListener?.("change", handleDisplayModeChange);

    return () => {
      window.removeEventListener("beforeinstallprompt", handleBeforeInstallPrompt);
      window.removeEventListener("appinstalled", handleInstalled);
      mediaQuery.removeEventListener?.("change", handleDisplayModeChange);
    };
  }, []);

  const fallback = useMemo(() => detectInstallContext(), []);

  const handleInstallClick = useCallback(async () => {
    if (isInstalled) return;

    if (deferredPrompt) {
      await deferredPrompt.prompt();
      const choice = await deferredPrompt.userChoice;
      if (choice.outcome !== "accepted") {
        setIsDialogOpen(true);
      }
      setDeferredPrompt(null);
      return;
    }

    setIsDialogOpen(true);
  }, [deferredPrompt, isInstalled]);

  return (
    <>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="h-9 w-9 rounded-full"
        onClick={handleInstallClick}
        aria-label={isInstalled ? "Aplicativo instalado" : "Baixar como app"}
        title={isInstalled ? "Aplicativo instalado" : "Baixar como app"}
      >
        {isInstalled ? <Check className="h-4 w-4" /> : <Download className="h-4 w-4" />}
      </Button>

      <Dialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
        <DialogContent className="max-w-md border-border/80">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <MonitorSmartphone className="h-5 w-5 text-primary" />
              {fallback.title}
            </DialogTitle>
            <DialogDescription>{fallback.description}</DialogDescription>
          </DialogHeader>

          <ol className="space-y-3 text-sm text-muted-foreground">
            {fallback.steps.map((step, index) => (
              <li key={step} className="flex gap-3 rounded-lg border border-border/70 bg-muted/40 px-3 py-3">
                <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-xs font-semibold text-primary">
                  {index + 1}
                </span>
                <span className="leading-6">{step}</span>
              </li>
            ))}
          </ol>

          <DialogFooter>
            <Button type="button" onClick={() => setIsDialogOpen(false)}>
              Entendi
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
