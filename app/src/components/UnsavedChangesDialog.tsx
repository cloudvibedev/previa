import { useTranslation } from "react-i18next";
import { ConfirmDialog } from "@/components/ConfirmDialog";

interface UnsavedChangesDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSave: () => void;
  onDiscard: () => void;
}

export function UnsavedChangesDialog({ open, onOpenChange, onSave, onDiscard }: UnsavedChangesDialogProps) {
  const { t } = useTranslation();

  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("unsavedChanges.title")}
      description={t("unsavedChanges.description")}
      onConfirm={onSave}
      confirmLabel={t("unsavedChanges.save")}
      onSecondaryAction={onDiscard}
      secondaryLabel={t("unsavedChanges.discard")}
      secondaryVariant="outline"
    />
  );
}
