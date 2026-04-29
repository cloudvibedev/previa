/**
 * MCP (Model Context Protocol) client for communicating with the backend MCP server.
 * Uses Streamable HTTP transport (JSON-RPC over POST) with proper initialize handshake.
 */

export interface McpTool {
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
}

export interface McpToolResult {
  content: Array<{ type: string; text?: string; [key: string]: unknown }>;
  isError?: boolean;
}

export interface McpSession {
  url: string;
  sessionId: string;
}

const MCP_PROTOCOL_VERSION = "2025-11-25";

const MCP_HEADERS: Record<string, string> = {
  "Content-Type": "application/json",
  Accept: "application/json, text/event-stream",
};

// --- Internal helpers ---

function buildHeaders(sessionId?: string): Record<string, string> {
  const h: Record<string, string> = { ...MCP_HEADERS };
  if (sessionId) {
    h["mcp-session-id"] = sessionId;
    h["mcp-protocol-version"] = MCP_PROTOCOL_VERSION;
  }
  return h;
}

async function parseResponse(res: Response): Promise<any> {
  const contentType = res.headers.get("content-type") ?? "";
  if (contentType.includes("text/event-stream")) {
    const text = await res.text();
    return parseLastSseResult(text);
  }
  return (await res.json())?.result ?? null;
}

function parseLastSseResult(sseText: string): any | null {
  let lastData: any = null;
  for (const line of sseText.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("data: ")) continue;
    const jsonStr = trimmed.slice(6).trim();
    if (jsonStr === "[DONE]") continue;
    try {
      const parsed = JSON.parse(jsonStr);
      if (parsed.result) lastData = parsed.result;
    } catch {
      // skip
    }
  }
  return lastData;
}

/**
 * Initialize an MCP session. Returns the session (url + sessionId).
 */
export async function mcpInitialize(mcpUrl: string): Promise<McpSession> {
  const res = await fetch(mcpUrl, {
    method: "POST",
    headers: MCP_HEADERS,
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: MCP_PROTOCOL_VERSION,
        capabilities: {},
        clientInfo: { name: "previa-studio", version: "1.0.0" },
      },
    }),
  });

  if (!res.ok) {
    throw new Error(`MCP initialize failed: ${res.status}`);
  }

  const sessionId = res.headers.get("mcp-session-id");
  if (!sessionId) {
    throw new Error("MCP server did not return mcp-session-id header");
  }

  // Parse body to ensure no error
  const contentType = res.headers.get("content-type") ?? "";
  if (contentType.includes("text/event-stream")) {
    await res.text(); // consume
  } else {
    const data = await res.json();
    if (data.error) throw new Error(`MCP init error: ${JSON.stringify(data.error)}`);
  }

  return { url: mcpUrl, sessionId };
}

/**
 * Initialize + list tools in one flow. Returns session and tools.
 */
export async function mcpConnect(mcpUrl: string): Promise<{ session: McpSession; tools: McpTool[] }> {
  const session = await mcpInitialize(mcpUrl);
  const tools = await mcpListTools(session);
  return { session, tools };
}

/**
 * Fetch a prompt from the MCP server using prompts/get.
 * Returns the prompt messages array, or null if not supported / not found.
 */
export async function mcpGetPrompt(
  session: McpSession,
  promptName: string,
  promptArgs: Record<string, string> = {},
): Promise<Array<{ role: string; content: { type: string; text: string } }> | null> {
  try {
    const res = await fetch(session.url, {
      method: "POST",
      headers: buildHeaders(session.sessionId),
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: Date.now(),
        method: "prompts/get",
        params: { name: promptName, arguments: promptArgs },
      }),
    });

    if (!res.ok) return null;

    const result = await parseResponse(res);
    if (!result?.messages) return null;
    return result.messages;
  } catch {
    return null;
  }
}

/**
 * Discover available tools from the MCP server (requires an active session).
 */
export async function mcpListTools(session: McpSession): Promise<McpTool[]> {
  const res = await fetch(session.url, {
    method: "POST",
    headers: buildHeaders(session.sessionId),
    body: JSON.stringify({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} }),
  });

  if (!res.ok) {
    throw new Error(`MCP tools/list failed: ${res.status}`);
  }

  const result = await parseResponse(res);
  return result?.tools ?? [];
}

/**
 * Call a tool on the MCP server and return the result.
 */
export async function mcpCallTool(
  session: McpSession,
  toolName: string,
  args: Record<string, unknown>,
): Promise<McpToolResult> {
  const res = await fetch(session.url, {
    method: "POST",
    headers: buildHeaders(session.sessionId),
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: Date.now(),
      method: "tools/call",
      params: { name: toolName, arguments: args },
    }),
  });

  if (!res.ok) {
    const text = await res.text();
    return { content: [{ type: "text", text: `MCP error ${res.status}: ${text}` }], isError: true };
  }

  const result = await parseResponse(res);
  if (!result) {
    return { content: [{ type: "text", text: "Empty MCP response" }], isError: true };
  }
  // Check for JSON-RPC error in parsed result
  if (result.error) {
    return { content: [{ type: "text", text: JSON.stringify(result.error) }], isError: true };
  }
  return result.content ? result : { content: [{ type: "text", text: JSON.stringify(result) }] };
}

/**
 * Convert MCP tools to OpenAI function-calling format.
 */
export function mcpToolsToOpenAI(mcpTools: McpTool[]): Array<{
  type: "function";
  function: { name: string; description: string; parameters: Record<string, unknown> };
}> {
  return mcpTools.map((t) => ({
    type: "function" as const,
    function: {
      name: t.name,
      description: t.description ?? "",
      parameters: t.inputSchema ?? { type: "object", properties: {}, required: [] },
    },
  }));
}

/**
 * Extract text content from an MCP tool result.
 */
export function mcpResultToText(result: McpToolResult): string {
  return result.content
    .filter((c) => c.type === "text" && c.text)
    .map((c) => c.text!)
    .join("\n");
}
