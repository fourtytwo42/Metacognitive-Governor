import { invoke } from "@tauri-apps/api/core";

export type OpenAiOAuthMethod = "browser" | "device_code";

export interface OAuthLoginStart {
  session_id: string;
  connection_id: string;
  auth_mode: "chat_gpt_browser_oauth" | "chat_gpt_device_code";
  verification_url: string;
  user_code?: string;
}

export interface ProviderMutation {
  connection: {
    id: string;
    display_name: string;
    auth_mode: string;
  };
  catalog: { models: unknown[] };
}

export type McpAuthMode = "none" | "bearer_token";
export type McpProtocolMode = "auto" | "modern20260728" | "legacy";

export interface McpConnection {
  contract_version: number;
  id: string;
  display_name: string;
  transport: { kind: "streamable_http"; url: string };
  auth_mode: McpAuthMode;
  protocol_mode: McpProtocolMode;
  enabled: boolean;
}

export interface McpToolCatalog {
  connection_id: string;
  protocol_version: string;
  server_info: { name: string; version?: string };
  tools: Array<{ name: string; title?: string; description?: string }>;
  discovered_at: string;
  expires_at: string;
}

export interface McpListItem {
  connection: McpConnection;
  catalog?: McpToolCatalog;
}

export interface McpListResponse {
  connections: McpListItem[];
}

export interface McpMutation {
  connection: McpConnection;
  catalog: McpToolCatalog;
}

export function startOpenAiOAuth(method: OpenAiOAuthMethod): Promise<OAuthLoginStart> {
  return invoke("start_openai_oauth", {
    request: {
      connection_id: "openai-subscription",
      display_name: "OpenAI Subscription",
      method
    }
  });
}

export function completeOpenAiOAuth(sessionId: string): Promise<ProviderMutation> {
  return invoke("complete_openai_oauth", { sessionId });
}

export function cancelOpenAiOAuth(sessionId: string): Promise<void> {
  return invoke("cancel_openai_oauth", { sessionId });
}

export function openExternalUrl(url: string): Promise<void> {
  return invoke("open_external_url", { url });
}

export function listMcpConnections(): Promise<McpListResponse> {
  return invoke("list_mcp_connections");
}

export function configureMcpConnection(
  connection: McpConnection,
  credential?: string
): Promise<McpMutation> {
  return invoke("configure_mcp_connection", {
    request: { connection, credential: credential || null }
  });
}

export function discoverMcpConnection(connectionId: string): Promise<McpToolCatalog> {
  return invoke("discover_mcp_connection", { connectionId });
}
