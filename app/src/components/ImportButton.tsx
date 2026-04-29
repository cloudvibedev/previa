import { useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Upload, Link, AlertCircle, Radio } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";

interface ImportButtonProps {
  onImport: (content: string, sourceUrl?: string, liveCheck?: boolean) => void;
  accept?: string;
  title?: string;
  dialogTitle?: string;
  dialogDescription?: string;
  variant?: "outline" | "ghost" | "default";
  size?: "sm" | "default";
  className?: string;
}

export function ImportButton({
  onImport,
  accept = ".json,.yaml,.yml",
  title,
  dialogTitle,
  dialogDescription,
  variant = "outline",
  size = "sm",
  className,
}: ImportButtonProps) {
  const { t } = useTranslation();
  const resolvedTitle = title ?? t("import.title");
  const resolvedDialogTitle = dialogTitle ?? t("import.dialogTitle");
  const resolvedDialogDescription = dialogDescription ?? t("import.dialogDescription");

  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [liveCheck, setLiveCheck] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const resetState = () => {
    setUrl("");
    setError(null);
    setLoading(false);
    setLiveCheck(false);
  };

  const handleOpenChange = (newOpen: boolean) => {
    if (!newOpen) resetState();
    setOpen(newOpen);
  };

  const handleUrl = async () => {
    if (!url.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(url.trim());
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const text = await res.text();
      onImport(text, url.trim(), liveCheck);
      setOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : t("import.urlError"));
    } finally {
      setLoading(false);
    }
  };

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setLoading(true);
    setError(null);
    try {
      const text = await file.text();
      onImport(text);
      setOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : t("import.fileError"));
    } finally {
      setLoading(false);
      if (fileRef.current) {
        fileRef.current.value = "";
      }
    }
  };

  return (
    <>
      <Button
        variant={variant}
        size={size}
        onClick={() => setOpen(true)}
        className={className}
      >
        <Upload className="h-3 w-3" />
        {resolvedTitle}
      </Button>

      <Dialog open={open} onOpenChange={handleOpenChange}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{resolvedDialogTitle}</DialogTitle>
            <DialogDescription>{resolvedDialogDescription}</DialogDescription>
          </DialogHeader>

          <div className="space-y-4 pt-2">
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Link className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  placeholder="https://example.com/spec.json"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  className="pl-9"
                  onKeyDown={(e) => e.key === "Enter" && handleUrl()}
                />
              </div>
              <Button onClick={handleUrl} disabled={loading || !url.trim()}>
                {loading ? t("import.loading") : t("import.load")}
              </Button>
            </div>

            <div className="flex items-center justify-between rounded-md border border-border/50 px-3 py-2">
              <Label htmlFor="live-check-import" className="text-xs font-medium flex items-center gap-1.5 cursor-pointer">
                <Radio className="h-3.5 w-3.5" />
                {t("import.liveCheck")}
                <span className="text-muted-foreground font-normal">{t("import.liveCheckDesc")}</span>
              </Label>
              <Switch
                id="live-check-import"
                checked={liveCheck}
                onCheckedChange={setLiveCheck}
              />
            </div>

            <div className="relative flex items-center gap-4">
              <div className="h-px flex-1 bg-border/50" />
              <span className="text-xs text-muted-foreground">{t("import.or")}</span>
              <div className="h-px flex-1 bg-border/50" />
            </div>

            <input
              ref={fileRef}
              type="file"
              accept={accept}
              className="hidden"
              onChange={handleFile}
            />
            <Button
              variant="outline"
              className="w-full"
              onClick={() => fileRef.current?.click()}
              disabled={loading}
            >
              <Upload className="h-4 w-4" />
              {t("import.uploadFile")}
            </Button>

            {error && (
              <Alert variant="destructive">
                <AlertCircle className="h-4 w-4" />
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
