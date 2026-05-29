export interface ParsedApiError {
  code: string;
  message: string;
  raw: string;
}

export function parseApiErrorText(text: string): ParsedApiError {
  const raw = text ?? "";

  try {
    const payload = JSON.parse(raw) as { error?: unknown; message?: unknown };
    const code = typeof payload.error === "string" && payload.error.trim()
      ? payload.error
      : "http_error";
    const message = typeof payload.message === "string" && payload.message.trim()
      ? payload.message
      : raw || "Request failed";

    return { code, message, raw };
  } catch {
    return {
      code: "http_error",
      message: raw || "Request failed",
      raw,
    };
  }
}

export function userFacingApiErrorMessage(error: ParsedApiError): string {
  switch (error.code) {
    case "forbidden":
      return "You do not have permission to perform this action.";
    case "unauthorized":
      return "Authentication is required. Sign in again or configure an API token.";
    case "not_found":
      return `Not found: ${error.message}`;
    case "service_unavailable":
      return `Service unavailable: ${error.message}`;
    case "bad_request":
      return `Invalid request: ${error.message}`;
    case "conflict":
      return `Conflict: ${error.message}`;
    default:
      return error.message;
  }
}
