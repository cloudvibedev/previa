export const METHOD_COLORS: Record<string, string> = {
  GET: "bg-success/15 text-success",
  POST: "bg-primary/15 text-primary",
  PUT: "bg-warning/15 text-warning",
  PATCH: "bg-secondary/15 text-secondary",
  DELETE: "bg-destructive/15 text-destructive",
  OPTIONS: "bg-secondary/15 text-secondary",
  HEAD: "bg-muted text-muted-foreground",
};

export const PARAM_TYPE_COLORS: Record<string, string> = {
  path: "bg-warning/15 text-warning",
  query: "bg-primary/15 text-primary",
  header: "bg-secondary/15 text-secondary",
  cookie: "bg-warning/15 text-warning",
};

export const STATUS_BORDER: Record<string, string> = {
  pending: "border-l-muted-foreground/30",
  running: "border-l-primary",
  success: "border-l-success",
  error: "border-l-destructive",
};
