import { invoke } from "@tauri-apps/api/core";

export type OpenAiOAuthMethod = "browser" | "device_code";

export interface OAuthLoginStart {
  session_id: string;
  connection_id: string;
  auth_mode: "chat_gpt_browser_oauth" | "chat_gpt_device_code";
  verification_url: string;
  user_code?: string;
}

export type ProviderKind = "open_ai" | "open_router" | "open_ai_compatible";
export type ProviderAuthMode = "chat_gpt_browser_oauth" | "chat_gpt_device_code" | "api_key" | "bearer_token" | "none";

export interface ProviderConnection {
  contract_version: number;
  id: string;
  display_name: string;
  kind: ProviderKind;
  base_url: string;
  auth_mode: ProviderAuthMode;
  organization?: string;
  project?: string;
}

export interface ProviderModel {
  id: string;
  display_name: string;
  owned_by?: string;
  context_window?: number;
  capabilities: Record<string, boolean | undefined>;
  price?: {
    input_per_million?: number;
    cached_input_per_million?: number;
    output_per_million?: number;
    currency: string;
    provenance: "provider_reported" | "unknown" | { mcg_catalog: { catalog_date: string } };
  };
}

export interface ProviderModelCatalog {
  connection_id: string;
  models: ProviderModel[];
  discovered_at: string;
}

export interface ProviderListItem {
  connection: ProviderConnection;
  catalog?: ProviderModelCatalog;
}

export interface ProviderListResponse {
  providers: ProviderListItem[];
}

export interface ProviderMutation {
  connection: ProviderConnection;
  catalog: ProviderModelCatalog;
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

export type ScheduleState = "enabled" | "paused" | "completed";
export type ScheduleTrigger =
  | { kind: "once"; at: string }
  | { kind: "interval"; seconds: number }
  | { kind: "cron"; expression: string }
  | { kind: "file_watch"; relative_path: string; poll_seconds: number }
  | { kind: "event"; event_kind: string }
  | { kind: "heartbeat"; idle_seconds: number; busy_seconds: number };

export interface ScheduleDefinition {
  schema_version: number;
  id: string;
  name: string;
  project_id: string;
  goal: string;
  trigger: ScheduleTrigger;
  state: ScheduleState;
  next_run_at?: string;
  watch_fingerprint?: string;
  last_watch_at?: string;
  created_at: string;
  updated_at: string;
}

export interface CreateScheduleRequest {
  name: string;
  project_id: string;
  goal: string;
  constraints: string[];
  success_conditions: string[];
  risk?: RiskLevel;
  budget?: TaskEnvelope["budget"];
  trigger: ScheduleTrigger;
  context_token_budget: number;
  requested_tools: string[];
  estimated_output_tokens: number;
  max_model_calls: number;
}

export interface ScheduleOccurrence {
  id: string;
  schedule_id: string;
  task_id?: string;
  due_at: string;
  state: "running" | "recovering" | "waiting_for_approval" | "completed" | "failed" | "budget_exhausted";
  attempt: number;
  detail?: string;
  updated_at: string;
}

export interface ScheduleListResponse { schedules: ScheduleDefinition[]; }
export interface ScheduleHistoryResponse { occurrences: ScheduleOccurrence[]; }

export interface SchedulerNotification {
  id: string;
  schedule_id: string;
  occurrence_id?: string;
  severity: "info" | "warning" | "high";
  title: string;
  detail: string;
  read: boolean;
  created_at: string;
}

export interface SchedulerNotificationListResponse { notifications: SchedulerNotification[]; }

export interface TaskEventsResponse {
  events: Array<{
    sequence: number;
    kind: string;
    occurred_at: string;
    payload?: {
      outcome?: "passed" | "failed" | "inconclusive";
      conditions?: Array<{ passed: boolean; detail: string }>;
      evidence?: Array<{ method: string; detail: string }>;
    };
  }>;
}

export interface ArtifactReference {
  id: string;
  sha256: string;
  media_type: string;
  size_bytes: number;
  relative_path: string;
}

export interface VerificationEvidence {
  method: string;
  passed: boolean;
  detail: string;
}

export interface ToolAuditRecord {
  schema_version: number;
  execution_id: string;
  request_id: string;
  task_id: string;
  tool: string;
  resource: string;
  request_fingerprint: string;
  phase: "decision" | "completion";
  decision: string;
  result?: "succeeded" | "failed" | "denied" | "approval_required" | "timed_out" | "cancelled";
  artifacts: ArtifactReference[];
  verification: VerificationEvidence[];
  recorded_at: string;
}

export interface ToolAuditResponse {
  records: ToolAuditRecord[];
}

export interface LaunchTaskResponse {
  task: TaskEnvelope;
  replayed: boolean;
  run: { run: { id: string; state: string } };
  execution: {
    state: "not_started" | "awaiting_approval" | "awaiting_evaluation" | "blocked" | "completed" | "failed";
    detail: string;
  };
}

export interface ApprovalRequest {
  id: string;
  task_id: string;
  action: string;
  resource: string;
  reason: string;
  state: "pending" | "approved" | "denied" | "expired" | "cancelled";
}

export interface ApprovalListResponse {
  approvals: ApprovalRequest[];
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

export function listProviders(): Promise<ProviderListResponse> {
  return invoke("list_providers");
}

export function configureProvider(
  connection: ProviderConnection,
  credential?: string
): Promise<ProviderMutation> {
  return invoke("configure_provider", {
    request: { connection, credential: credential || null }
  });
}

export function discoverProvider(connectionId: string): Promise<ProviderModelCatalog> {
  return invoke("discover_provider", { connectionId });
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

export function listSchedules(): Promise<ScheduleListResponse> {
  return invoke("list_schedules");
}

export function createSchedule(request: CreateScheduleRequest): Promise<ScheduleDefinition> {
  return invoke("create_schedule", { request });
}

export function setScheduleState(scheduleId: string, state: ScheduleState): Promise<ScheduleDefinition> {
  return invoke("set_schedule_state", { scheduleId, request: { state } });
}

export function loadScheduleHistory(scheduleId: string): Promise<ScheduleHistoryResponse> {
  return invoke("schedule_history", { scheduleId });
}

export function wakeSchedule(scheduleId: string, reason: string): Promise<unknown> {
  return invoke("wake_schedule", {
    scheduleId,
    request: { severity: "warning", reason }
  });
}

export function listSchedulerNotifications(): Promise<SchedulerNotificationListResponse> {
  return invoke("list_scheduler_notifications");
}

export function acknowledgeSchedulerNotification(notificationId: string): Promise<SchedulerNotification> {
  return invoke("acknowledge_scheduler_notification", { notificationId });
}

export function createTask(request: CreateTaskRequest): Promise<TaskMutationResponse> {
  return invoke("create_task", { request });
}

export function transitionTask(taskId: string, state: TaskState): Promise<TaskEnvelope> {
  return invoke("transition_task", { taskId, request: { state } });
}

export function launchTask(taskId: string): Promise<LaunchTaskResponse> {
  return invoke("launch_task", {
    taskId,
    request: {
      context_token_budget: 8192,
      requested_tools: [],
      estimated_output_tokens: 4096,
      max_model_calls: 8
    }
  });
}

export function retryTask(taskId: string): Promise<LaunchTaskResponse> {
  return invoke("retry_task", {
    taskId,
    request: {
      context_token_budget: 8192,
      requested_tools: [],
      estimated_output_tokens: 4096,
      max_model_calls: 8
    }
  });
}

export function resumeTask(taskId: string): Promise<LaunchTaskResponse> {
  return invoke("resume_task", {
    taskId,
    request: {
      context_token_budget: 8192,
      requested_tools: [],
      estimated_output_tokens: 4096,
      max_model_calls: 8
    }
  });
}

export function listApprovals(): Promise<ApprovalListResponse> {
  return invoke("list_approvals");
}

export function approveRequest(approvalId: string): Promise<ApprovalRequest> {
  return invoke("resolve_approval", {
    approvalId,
    request: { state: "approved", reason: "Approved in MCG desktop" }
  });
}

export function denyRequest(approvalId: string): Promise<ApprovalRequest> {
  return invoke("resolve_approval", {
    approvalId,
    request: { state: "denied", reason: "Denied in MCG desktop" }
  });
}

export function loadTaskEvents(taskId: string): Promise<TaskEventsResponse> {
  return invoke("task_events", { taskId });
}

export function loadTaskAudit(taskId: string): Promise<ToolAuditResponse> {
  return invoke("task_tool_audit", { taskId });
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
