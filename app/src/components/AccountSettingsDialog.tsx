import { FormEvent, useEffect, useState } from "react";
import { LogOut, Save, UserCircle } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";

import { updateCurrentUser } from "@/lib/auth-client";
import { useAuthStore } from "@/stores/useAuthStore";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface AccountSettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function AccountSettingsDialog({ open, onOpenChange }: AccountSettingsDialogProps) {
  const navigate = useNavigate();
  const token = useAuthStore((state) => state.token);
  const user = useAuthStore((state) => state.user);
  const setSession = useAuthStore((state) => state.setSession);
  const clearSession = useAuthStore((state) => state.clearSession);
  const [username, setUsername] = useState("");
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [saving, setSaving] = useState(false);

  const canEdit = user?.source === "database";

  useEffect(() => {
    if (!open || !user) return;
    setUsername(user.username);
    setName(user.name ?? "");
    setEmail(user.email ?? "");
    setCurrentPassword("");
    setNewPassword("");
  }, [open, user]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canEdit) return;
    setSaving(true);
    try {
      const updated = await updateCurrentUser({
        username,
        name,
        email,
        currentPassword: currentPassword || undefined,
        newPassword: newPassword || undefined,
      });
      setSession(token, updated);
      setCurrentPassword("");
      setNewPassword("");
      toast.success("Conta atualizada");
    } catch {
      toast.error("Nao foi possivel atualizar a conta");
    } finally {
      setSaving(false);
    }
  }

  function handleLogout() {
    clearSession();
    onOpenChange(false);
    navigate("/login", { replace: true });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <UserCircle className="h-5 w-5" />
            Conta
          </DialogTitle>
          <DialogDescription>Perfil e credenciais do usuario atual.</DialogDescription>
        </DialogHeader>

        <form id="account-settings-form" className="space-y-4" onSubmit={handleSubmit}>
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="account-username">Usuario</Label>
              <Input
                id="account-username"
                value={username}
                disabled={!canEdit || saving}
                onChange={(event) => setUsername(event.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label>Role</Label>
              <div className="flex h-10 items-center rounded-md border px-3">
                <Badge variant="outline" className="capitalize">{user?.role ?? "-"}</Badge>
              </div>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="account-name">Nome</Label>
            <Input
              id="account-name"
              value={name}
              disabled={!canEdit || saving}
              onChange={(event) => setName(event.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="account-email">Email</Label>
            <Input
              id="account-email"
              type="email"
              value={email}
              disabled={!canEdit || saving}
              onChange={(event) => setEmail(event.target.value)}
            />
          </div>

          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <Label htmlFor="account-current-password">Senha atual</Label>
              <Input
                id="account-current-password"
                type="password"
                value={currentPassword}
                disabled={!canEdit || saving}
                autoComplete="current-password"
                onChange={(event) => setCurrentPassword(event.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="account-new-password">Nova senha</Label>
              <Input
                id="account-new-password"
                type="password"
                value={newPassword}
                disabled={!canEdit || saving}
                autoComplete="new-password"
                onChange={(event) => setNewPassword(event.target.value)}
              />
            </div>
          </div>

          {!canEdit ? (
            <p className="rounded-md border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
              Esta conta vem de variaveis de ambiente ou token. Perfil e senha devem ser alterados na configuracao do servidor.
            </p>
          ) : null}
        </form>

        <DialogFooter className="gap-2 sm:justify-between">
          <Button type="button" variant="outline" onClick={handleLogout}>
            <LogOut className="h-4 w-4" />
            Sair
          </Button>
          <Button type="submit" form="account-settings-form" disabled={!canEdit || saving || !username.trim()}>
            <Save className="h-4 w-4" />
            Salvar
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
