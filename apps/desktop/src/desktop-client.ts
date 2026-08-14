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
export type McpTransportKind = "streamable_http" | "stdio";

export type McpTransport =
  | { kind: "streamable_http"; url: string }
  | { kind: "stdio"; command: string; args: string[] };

export interface McpConnection {
  contract_version: number;
  id: string;
  display_name: string;
  transport: McpTransport;
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

export type TaskState =
  | "pending"
  | "active"
  | "waiting_for_approval"
  | "blocked"
  | "completed"
  | "failed"
  | "cancelled";

export type RiskLevel = "low" | "medium" | "high" | "critical";

export interface TaskEnvelope {
  schema_version: number;
  id: string;
  project_id?: string;
  thread_id?: string;
  goal: string;
  constraints: string[];
  success_conditions: string[];
  risk: RiskLevel;
  budget: {
    max_tokens?: number;
    max_cost_usd?: number;
    max_duration_seconds?: number;
  };
  state: TaskState;
  created_at: string;
  updated_at: string;
}

export interface CreateTaskRequest {
  goal: string;
  project_id?: string;
  constraints: string[];
  success_conditions: string[];
  risk?: RiskLevel;
  budget?: TaskEnvelope["budget"];
}

export interface ProjectModel {
  id: string;
  project_id: string;
  root: string;
  observed_at: string;
  repository: { is_git: boolean; branch?: string; dirty: boolean };
  languages: string[];
  build_systems: string[];
  test_systems: string[];
  instructions: { conflicts: unknown[] };
}

export interface ProjectMutation {
  project: ProjectModel;
}

export interface ProjectListResponse {
  projects: ProjectModel[];
}

export interface TaskListResponse {
  tasks: TaskEnvelope[];
}

export interface TaskMutationResponse {
  task: TaskEnvelope;
  event: { sequence: number; kind: string; occurred_at: string };
}

export interface TaskEventsResponse {
  events: Array<{ sequence: number; kind: string; occurred_at: string }>;
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

export function listTasks(): Promise<TaskListResponse> {
  return invoke("list_tasks");
}

export function createTask(request: CreateTaskRequest): Promise<TaskMutationResponse> {
  return invoke("create_task", { request });
}

export function transitionTask(taskId: string, state: TaskState): Promise<TaskEnvelope> {
  return invoke("transition_task", { taskId, request: { state } });
}

export function loadTaskEvents(taskId: string): Promise<TaskEventsResponse> {
  return invoke("task_events", { taskId });
}

export function pickProjectDirectory(): Promise<string | null> {
  return invoke("pick_project_directory");
}

export function discoverProject(root: string): Promise<ProjectMutation> {
  return invoke("discover_project", { request: { root, project_id: null } });
}

export function listProjects(): Promise<ProjectListResponse> {
  return invoke("list_projects");
}
