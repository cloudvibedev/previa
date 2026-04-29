import type { OpenAPIRoute } from "@/types/pipeline";

export interface KeyValue {
  key: string;
  value: string;
}

export interface TryItResponse {
  status: number;
  statusText: string;
  headers: Record<string, string>;
  body: unknown;
  duration: number;
}

export interface TryItDrawerProps {
  route: OpenAPIRoute | null;
  servers: Record<string, string>;
  onClose: () => void;
  allRoutes?: OpenAPIRoute[];
  onSelectRoute?: (route: OpenAPIRoute) => void;
}
