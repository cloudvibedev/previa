import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { useEventStore, type AppEvent } from "@/stores/useEventStore";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Bell, Trash2, X, Inbox } from "lucide-react";

function typeConfig(type: AppEvent["type"]) {
  switch (type) {
    case "error":
      return { label: "ERROR", badgeClass: "bg-destructive text-destructive-foreground", textClass: "text-destructive" };
    case "warning":
      return { label: "WARN", badgeClass: "bg-warning/90 text-warning-foreground", textClass: "text-warning" };
    case "success":
      return { label: "OK", badgeClass: "bg-success/90 text-success-foreground", textClass: "text-success" };
    case "info":
    default:
      return { label: "INFO", badgeClass: "bg-muted text-muted-foreground", textClass: "text-muted-foreground" };
  }
}

function EventCard({ event, onDismiss, onClose, navigate }: { event: AppEvent; onDismiss: () => void; onClose: () => void; navigate: (path: string) => void }) {
  const time = new Date(event.timestamp).toLocaleTimeString("pt-BR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  const config = typeConfig(event.type);

  return (
    <div className="group relative border border-border/50 rounded-md p-3 bg-background/50 hover:transition-colors">
      <button
        onClick={onDismiss}
        className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity text-muted-foreground hover:text-foreground"
      >
        <X className="h-3.5 w-3.5" />
      </button>
      <div className="flex items-center gap-2 mb-1.5">
        <span className="text-xs text-muted-foreground font-mono">{time}</span>
        <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-bold uppercase ${config.badgeClass}`}>
          {config.label}
        </span>
        {event.details?.method && (
          <Badge variant="outline" className="text-[10px] px-1.5 py-0 font-mono uppercase">
            {event.details.method}
          </Badge>
        )}
        {event.details?.statusCode && (
          <Badge
            variant={event.details.statusCode >= 500 ? "destructive" : "outline"}
            className="text-[10px] px-1.5 py-0 font-mono"
          >
            {event.details.statusCode}
          </Badge>
        )}
      </div>
      <p className="text-sm font-medium text-foreground">{event.title}</p>
      <p className={`text-xs mt-1 break-all ${config.textClass}`}>{event.message}</p>
      {event.details?.url && (
        <p className="text-[10px] text-muted-foreground mt-1 font-mono truncate">{event.details.url}</p>
      )}
      {event.actionUrl && (
        <Button
          variant="outline"
          size="sm"
          className="mt-2 h-6 text-[11px] px-2 py-0"
          onClick={() => {
            console.log("[EventsPanel] closing sheet and navigating to", event.actionUrl);
            onClose();
            setTimeout(() => navigate(event.actionUrl!), 50);
          }}
        >
          {event.actionLabel ?? "Ver"}
        </Button>
      )}
      {event.action && !event.actionUrl && (
        <Button
          variant="outline"
          size="sm"
          className="mt-2 h-6 text-[11px] px-2 py-0"
          onClick={() => { onClose(); event.action!.onClick(); }}
        >
          {event.action.label}
        </Button>
      )}
    </div>
  );
}

function EmptyState() {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center gap-3">
      <Inbox className="h-10 w-10 text-muted-foreground/40" />
      <div>
        <p className="text-sm font-medium text-muted-foreground">{t("events.empty.title")}</p>
        <p className="text-xs text-muted-foreground/70 mt-1">
          {t("events.empty.description")}
        </p>
      </div>
    </div>
  );
}

export function EventsPanel() {
  const { t } = useTranslation();
  const { events, clearEvents, dismissEvent } = useEventStore();
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const count = events.length;

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button variant="ghost" size="icon" className="h-8 w-8 relative" title={t("events.tooltip")}>
          <Bell className="h-4 w-4" />
          {count > 0 && (
            <span className="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive text-[10px] font-bold text-destructive-foreground px-1">
              {count > 99 ? "99+" : count}
            </span>
          )}
        </Button>
      </SheetTrigger>
      <SheetContent className="w-[380px] sm:w-[420px] flex flex-col">
        <SheetHeader className="flex-row items-center justify-between pr-6">
          <SheetTitle className="text-base">{t("events.title", { count })}</SheetTitle>
          {count > 0 && (
            <Button variant="ghost" size="sm" onClick={clearEvents} className="text-xs gap-1.5">
              <Trash2 className="h-3.5 w-3.5" />
              {t("events.clearAll")}
            </Button>
          )}
        </SheetHeader>
        <ScrollArea className="flex-1 -mx-6 px-6 mt-4">
          {count === 0 ? (
            <EmptyState />
          ) : (
            <div className="flex flex-col gap-2 pb-4">
              {events.map((evt) => (
                <EventCard key={evt.id} event={evt} onDismiss={() => dismissEvent(evt.id)} onClose={() => setOpen(false)} navigate={navigate} />
              ))}
            </div>
          )}
        </ScrollArea>
      </SheetContent>
    </Sheet>
  );
}
