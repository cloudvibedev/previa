import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Download } from "lucide-react";
import type { Project } from "@/types/project";
import { exportProject } from "@/lib/project-io";
import { toast } from "sonner";

interface ExportDialogProps {
  project: Project | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ExportDialog({ project, open, onOpenChange }: ExportDialogProps) {
  const { t } = useTranslation();
  const [includeHistory, setIncludeHistory] = useState(false);
  const [exporting, setExporting] = useState(false);

  const handleExport = async () => {
    if (!project) return;
    setExporting(true);
    try {
      await exportProject(project, includeHistory);
      toast.success(t("export.success"));
      onOpenChange(false);
    } catch (err) {
      toast.error(t("export.error"));
    } finally {
      setExporting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("export.title")}</DialogTitle>
          <DialogDescription>
            {t("export.description", { name: project?.name })}
          </DialogDescription>
        </DialogHeader>

        <div className="flex items-center space-x-2 py-4">
          <Checkbox
            id="include-history"
            checked={includeHistory}
            onCheckedChange={(v) => setIncludeHistory(v === true)}
          />
          <Label htmlFor="include-history">{t("export.includeHistory")}</Label>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={handleExport} disabled={exporting}>
            <Download className="h-4 w-4" />
            {exporting ? t("export.exporting") : t("common.export")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
