import { useState, useEffect } from "react";
import { Trans, useTranslation } from "react-i18next";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Input } from "@/components/ui/input";

interface ConfirmDeleteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  itemName: string;
  onConfirm: () => void;
  itemType?: string;
}

export function ConfirmDeleteDialog({ open, onOpenChange, itemName, onConfirm, itemType = "item" }: ConfirmDeleteDialogProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState("");

  useEffect(() => {
    if (!open) setValue("");
  }, [open]);

  return (
    <ConfirmDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t("confirmDelete.title", { type: itemType })}
      description={(
        <Trans
          i18nKey="confirmDelete.description"
          values={{ name: itemName }}
          components={{ strong: <strong className="font-semibold text-foreground" /> }}
        />
      )}
      confirmLabel={t("common.remove")}
      variant="destructive"
      confirmDisabled={value !== itemName}
      onConfirm={onConfirm}
    >
      <Input
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder={itemName}
        autoFocus
      />
    </ConfirmDialog>
  );
}
