import { invoke } from "@tauri-apps/api/core";

export type OpenAiOAuthMethod = "browser" | "device_code";
export type AutonomyPreset = "guided" | "balanced" | "autonomous";

export interface LocalServiceStatus {
  ready: boolean;
  launched: boolean;
  service: string;
  version: string;
  detail: string;
}

export interface OpenInApplication {
  id: string;
  label: string;
  available: boolean;
}

export interface TerminalCommandResult {
  working_directory: string;
  output: string;
  exit_code: number;
}

export interface UpdateConfiguration {
  schema_version: number;
  configured: boolean;
  current_version: string;
  trusted_origin: string | null;
  detail: string;
  recovery_notice: string | null;
}

export interface UpdateCheck {
  schema_version: number;
  configured: boolean;
  current_version: string;
  available_version: string | null;
  notes: string | null;
  published_at: string | null;
  target: string | null;
  detail: string;
}

export type DiagnosticSeverity = "pass" | "warning" | "error";

export interface DiagnosticCheck {
  code: string;
  label: string;
  severity: DiagnosticSeverity;
  detail: string;
  remediation?: string;
}

export interface DiagnosticReport {
  schema_version: number;
  generated_at: string;
  service: string;
  version: string;
  os: string;
  architecture: string;
  overall: DiagnosticSeverity;
  checks: DiagnosticCheck[];
  counts: {
    events: number;
    tasks: number;
    artifacts: number;
    backups: number;
    providers: number;
    models: number;
    mcp_connections: number;
    mcp_tools: number;
    schedules: number;
    pending_approvals: number;
    provider_dispatch_started: number;
    provider_dispatch_completed: number;
    provider_dispatch_failed: number;
  };
}

export interface UserSettings {
  schema_version: number;
  revision: number;
  advanced_mode: boolean;
  autonomy: AutonomyPreset;
  shadow_experiments: boolean;
  codex_remote_endpoint?: string;
  codex_remote_auth_token_env?: string;
  updated_at: string;
}

export interface UpdateUserSettingsRequest {
  expected_revision: number;
  advanced_mode: boolean;
  autonomy: AutonomyPreset;
  shadow_experiments: boolean;
  reason: string;
}

export interface UserSettingsHistoryResponse {
  revisions: UserSettingsRevision[];
}

export interface UserSettingsRevision {
  settings: UserSettings;
  reason: string;
  changed_at: string;
}

export interface BackupBundleEntry {
  path: string;
  size_bytes: number;
  sha256: string;
}

export interface BackupSecretEncryptionMetadata {
  schema_version: number;
  entry_path: string;
  algorithm: string;
  kdf: string;
  argon2_version: number;
  memory_kib: number;
  iterations: number;
  parallelism: number;
  salt_base64: string;
  nonce_base64: string;
}

export interface BackupSummary {
  backup_id: string;
  file_name: string;
  sha256: string;
  size_bytes: number;
  manifest: {
    schema_version: number;
    application_version: string;
    created_at: string;
    secrets_included: boolean;
    secret_encryption?: BackupSecretEncryptionMetadata;
    entries: BackupBundleEntry[];
  };
}

export interface BackupListResponse {
  backups: BackupSummary[];
}

export type PluginCapability = "filesystem_read" | "filesystem_write" | "network" | "process" | "credentials";
export type PluginCapabilityScope =
  | { kind: "task_project" }
  | { kind: "network_origin"; origin: string }
  | { kind: "executable"; executable: string }
  | { kind: "credential"; provider: string; key: string };

export interface PluginCapabilityGrant {
  schema_version: number;
  capability: PluginCapability;
  scopes: PluginCapabilityScope[];
}

export interface PluginManifest {
  schema_version: number;
  id: string;
  display_name: string;
  version: string;
  class: "trusted_native" | "sandboxed_wasi";
  requested_capabilities: PluginCapability[];
}

export interface PluginRevision {
  schema_version: number;
  id: string;
  manifest: PluginManifest;
  module_sha256: string;
  module_size_bytes: number;
  source: { kind: "local_import" } | { kind: "registry"; registry: string; package: string };
  signature: {
    state: "unsigned" | "verified";
    scheme?: string;
    signer?: string;
    signature_sha256?: string;
  };
  installed_at: string;
}

export interface PluginInstallation {
  schema_version: number;
  plugin_id: string;
  active_revision_id: string;
  enabled: boolean;
  capability_grants: PluginCapabilityGrant[];
  revision: number;
  updated_at: string;
}

export interface PluginDescriptor {
  installation: PluginInstallation;
  active_revision: PluginRevision;
}

export interface PluginListResponse { plugins: PluginDescriptor[]; }
export interface InstallLocalPluginRequest { revision: PluginRevision; module_base64: string; }
export interface InstallRegistryPluginRequest extends InstallLocalPluginRequest {
  proof: { schema_version: number; scheme: string; signer: string; signature_base64: string };
}
export interface SetPluginGrantsRequest { expected_revision: number; grants: PluginCapabilityGrant[]; }
export interface PluginInvocationRecord {
  schema_version: number;
  id: string;
  plugin_id: string;
  plugin_revision_id: string;
  task_id?: string;
  input_sha256: string;
  state: "running" | "succeeded" | "failed" | "uncertain";
  result_sha256?: string;
  error_code?: string;
  started_at: string;
  completed_at?: string;
}
export interface PluginInvocationMutation {
  invocation: PluginInvocationRecord;
  result?: { schema_version: number; plugin_id: string; output: unknown };
  replayed: boolean;
}
export interface PluginInvocationHistoryResponse {
  invocation: PluginInvocationRecord;
  history: PluginInvocationRecord[];
}

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

export type ReasoningEffort = "none" | "low" | "medium" | "high" | "xhigh" | (string & {});
export type ExecutionSpeed = "standard" | "fast";
export type TeamPreset = "automatic" | "fast" | "balanced" | "thorough";
export type ApprovalMode = "ask" | "auto_review" | "read_only" | "full_access";
export type ExecutionApprovalPolicy = "untrusted" | "on-request" | "never";
export type ExecutionSandboxMode = "read-only" | "workspace-write" | "danger-full-access";
export interface CodexConfigOverride { key: string; value: unknown; }

export interface GitHubAccount {
  login: string;
  name?: string;
  avatar_url?: string;
  html_url: string;
}

export interface GitHubConnectionStatus {
  configured: boolean;
  connected: boolean;
  account?: GitHubAccount;
}

export interface GitHubLoginStart { session_id: string; authorization_url: string; }
export interface GitHubRepository {
  id: number;
  full_name: string;
  private: boolean;
  html_url: string;
  clone_url: string;
  default_branch: string;
}
export interface GitHubRepositoryList { repositories: GitHubRepository[]; }

export interface ChatRunPreferences {
  provider_id?: string;
  model_provider?: "ollama" | "lmstudio";
  model_id?: string;
  model_label?: string;
  reasoning_effort: ReasoningEffort;
  speed: ExecutionSpeed;
  service_tier?: string;
  approval_mode: ApprovalMode;
  approval_policy?: ExecutionApprovalPolicy;
  sandbox_mode?: ExecutionSandboxMode;
  permission_profile?: string;
  additional_writable_roots?: string[];
  codex_feature_overrides?: Record<string, boolean>;
  codex_config_overrides?: CodexConfigOverride[];
  codex_profile?: string;
  strict_config?: boolean;
  output_schema?: Record<string, unknown>;
  team_preset: TeamPreset;
  personality?: string;
}

export interface ExecutionPreferences {
  model_provider?: "ollama" | "lmstudio";
  model_id?: string;
  reasoning_effort?: string;
  service_tier?: string;
  approval_mode?: ApprovalMode;
  approval_policy?: ExecutionApprovalPolicy;
  sandbox_mode?: ExecutionSandboxMode;
  permission_profile?: string;
  additional_writable_roots?: string[];
  codex_feature_overrides?: Record<string, boolean>;
  codex_config_overrides?: CodexConfigOverride[];
  codex_profile?: string;
  strict_config?: boolean;
  output_schema?: Record<string, unknown>;
  team_preset?: TeamPreset;
  personality?: string;
}

export type ConversationState = "active" | "archived";
export type ConversationRole = "user" | "assistant" | "system";
export type ConversationTurnState = "pending" | "streaming" | "waiting_for_approval" | "recovering" | "indeterminate" | "completed" | "failed" | "interrupted";
export type ConversationMode = "default" | "plan";
export type ConversationGoalStatus = "active" | "paused" | "blocked" | "usageLimited" | "budgetLimited" | "complete";
export interface ConversationGoal { threadId: string; objective: string; status: ConversationGoalStatus; tokenBudget?: number; tokensUsed: number; timeUsedSeconds: number; createdAt: number; updatedAt: number; }
export interface ConversationInputOption { label: string; description: string; }
export interface ConversationInputQuestion { id: string; header: string; question: string; isOther: boolean; isSecret: boolean; options?: ConversationInputOption[]; }
export interface ConversationInputRequest { requestId: unknown; threadId: string; turnId: string; itemId: string; questions: ConversationInputQuestion[]; receivedAt: string; }

export interface Conversation {
  schema_version: number;
  id: string;
  title: string;
  project_id?: string;
  state: ConversationState;
  execution_preferences: ExecutionPreferences;
  mode?: ConversationMode;
  goal?: ConversationGoal;
  pending_input_requests?: ConversationInputRequest[];
  created_at: string;
  updated_at: string;
}

export interface ConversationMessage {
  schema_version: number;
  id: string;
  conversation_id: string;
  turn_id: string;
  role: ConversationRole;
  content: string;
  mentions?: ConversationMention[];
  codex_item_id?: string;
  created_at: string;
}

export type MentionKind = "file" | "folder" | "plugin";
export type MentionTarget =
  | { kind: "project_path"; project_id: string; relative_path: string }
  | { kind: "external_path"; canonical_path: string; artifact_sha256?: string; artifact_relative_path?: string }
  | { kind: "plugin"; plugin_id: string; plugin_revision_id: string; installation_revision: number; grants_sha256: string };
export interface ComposerMentionInput {
  kind: MentionKind;
  label: string;
  resource: string;
  byte_start: number;
  byte_end: number;
  plugin_revision_id?: string;
  installation_revision?: number;
}
export interface ConversationMention extends Omit<ComposerMentionInput, "resource" | "plugin_revision_id" | "installation_revision"> {
  id: string;
  target: MentionTarget;
  observed_size_bytes?: number;
  observed_modified_unix_milliseconds?: number;
  observed_sha256?: string;
}
export interface MentionCandidate {
  kind: MentionKind;
  label: string;
  resource: string;
  detail?: string;
  plugin_revision_id?: string;
  installation_revision?: number;
}
export interface MentionCandidateListResponse { candidates: MentionCandidate[]; }

export interface ConversationTurn {
  schema_version: number;
  id: string;
  conversation_id: string;
  state: ConversationTurnState;
  execution_preferences: ExecutionPreferences;
  codex_thread_id?: string;
  codex_turn_id?: string;
  task_id?: string;
  run_id?: string;
  error?: string;
  created_at: string;
  updated_at: string;
}

export type ConversationActivityKind =
  | "working" | "waiting" | "reasoning_summary" | "plan" | "command" | "file_change"
  | "mcp_call" | "collaboration_call" | "web_search" | "image_view" | "approval"
  | "elicitation" | "context_compaction" | "hook" | "usage" | "recovery" | "interruption"
  | "failure" | "completion";
export type ConversationActivityState = "running" | "completed" | "failed" | "declined";

export interface ConversationActivity {
  schema_version: number;
  conversation_id: string;
  turn_id: string;
  agent_session_id?: string;
  delegation_id?: string;
  codex_item_id: string;
  kind: ConversationActivityKind;
  state: ConversationActivityState;
  command?: string;
  cwd?: string;
  output?: string;
  exit_code?: number;
  duration_ms?: number;
  started_at: string;
  updated_at: string;
}

export type ConversationAgentKind = "chat" | "coordinator" | "coder" | "tester";
export type ConversationAgentSessionState = "starting" | "ready" | "working" | "waiting" | "failed" | "retired";
export interface ConversationAgentSession {
  schema_version: number;
  id: string;
  conversation_id: string;
  role_id: string;
  organization_revision_id: string;
  kind: ConversationAgentKind;
  display_name: string;
  parent_session_id?: string;
  state: ConversationAgentSessionState;
  codex_thread_id?: string;
  model?: string;
  prompt_revision: number;
  created_at: string;
  updated_at: string;
}
export type AgentDelegationState = "requested" | "preparing" | "running" | "waiting" | "blocked" | "reported" | "integrating" | "completed" | "failed" | "cancelled";
export type DelegationFailureClass = "none" | "precondition" | "policy_denied" | "transient_tool" | "agent_omission" | "verification" | "conflict" | "delivery" | "cancelled" | "uncertain";
export type DelegationRecoveryDirective = "none" | "retry_same_thread" | "retry_new_delegation" | "replan" | "await_owner" | "retain_for_review" | "do_not_retry";
export interface DelegationGovernance {
  contract: { objective_sha256: string; allowed_scopes: string[]; required_checks: string[]; required_evidence: string[]; require_integration_ready: boolean; context_token_budget: number; max_attempts: number };
  preflight: { passed: boolean; checks: string[]; blockers: string[]; receipt_sha256?: string; completed_at?: string };
  context_receipts: Array<{ source_count: number; approximate_tokens: number; token_budget: number; truncated: boolean; source_sha256: string; created_at?: string }>;
  outcome: { latency_ms: number; model_turns: number; approximate_tokens: number; tool_calls: number; attempts: number; owner_interventions: number; changed_file_count: number; check_count: number; successful: boolean };
  failure_class: DelegationFailureClass;
  recovery: DelegationRecoveryDirective;
  dependencies: string[];
  reserved_scopes: string[];
}
export interface AgentDelegation {
  schema_version: number;
  id: string;
  conversation_id: string;
  source_session_id: string;
  coder_session_id?: string;
  project_id: string;
  organization_revision_id: string;
  revision: number;
  prompt_revision: number;
  permission_mode?: ApprovalMode;
  state: AgentDelegationState;
  brief: { objective: string; scope: string[]; constraints: string[]; success_conditions: string[]; context_references: string[] };
  governance?: DelegationGovernance;
  task_id?: string;
  run_id?: string;
  agent_instance_id?: string;
  agent_lease_id?: string;
  worktree_id?: string;
  retry_of_id?: string;
  coordination_batch_id?: string;
  origin_turn_id?: string;
  coordinator_decision?: "accept" | "block";
  coordinator_rationale?: string;
  coordinator_reviewed_at?: string;
  idempotency_key?: string;
  summary?: string;
  changed_files: string[];
  checks: string[];
  evidence?: string[];
  unresolved_issues?: string[];
  integration_ready?: boolean;
  report_sha256?: string;
  accepted_codex_item_id?: string;
  error?: string;
  created_at: string;
  updated_at: string;
}
export interface AgentMessage {
  schema_version: number; id: string; conversation_id: string; delegation_id?: string;
  from_session_id: string; to_session_id: string;
  kind: "joined" | "left" | "delegation" | "update" | "question" | "answer" | "amendment" | "report" | "cancellation" | "tool_call" | "tool_result";
  room?: "frontstage" | "backstage";
  tool_name?: string;
  correlation_id?: string;
  content: string; created_at: string;
}
export interface AgentContextCheckpoint {
  schema_version: number; id: string; conversation_id: string; session_id: string;
  room: "frontstage" | "backstage"; revision: number; through_message_id?: string;
  summary: string; source_references: string[]; approximate_tokens: number; created_at: string;
  cause?: "legacy" | "delegation_sealed" | "question" | "blocker" | "final_review" | "size_threshold";
  content_sha256?: string;
}
export interface CoordinationBatch {
  schema_version: number; id: string; conversation_id: string; origin_turn_id: string;
  coordinator_session_id: string; state: "open" | "running" | "review_pending" | "reviewing" | "chat_pending" | "completed" | "failed" | "recovery_required";
  delivery_target?: "codex_sites" | "custom_project"; delivery_rationale?: string; site_owner_session_id?: string;
  revision: number; delegation_ids: string[]; coordinator_turn_count: number; coordinator_approximate_tokens: number;
  coordinator_bypassed?: boolean; routing_rationale?: string; owner_intervention_count?: number;
  last_error?: string; created_at: string; updated_at: string;
}
export interface ConversationPreview {
  schema_version: number; id: string; conversation_id: string; coordination_batch_id: string;
  delegation_id: string; project_id: string; delivery_target: "codex_sites" | "custom_project";
  strategy: "codex_sites" | "static_files" | "manifest_command";
  state: "preparing_site" | "building" | "validating" | "publishing_private" | "approval_required" | "available" | "running" | "stopped" | "failed" | "stale";
  source_revision: number; source_sha256: string; source_root?: string; entry_path?: string;
  site_id?: string; site_version_id?: string; deployment_id?: string; safe_url?: string;
  browser_tab_id?: string; last_error?: string; created_at: string; updated_at: string; last_accessed_at?: string;
}
export interface AgentWake {
  schema_version: number; id: string; conversation_id: string; coordination_batch_id: string;
  target_session_id: string; target: "coordinator" | "chat"; kind: "initial_request" | "question" | "blocker" | "final_review" | "chat_completion";
  state: "pending" | "claimed" | "completed" | "failed" | "recovery_required"; idempotency_key: string;
  delegation_id?: string; source_message_id?: string; delegation_revisions: Array<[string, number]>; attempt_count: number;
  claimed_until?: string; payload: string; last_error?: string; created_at: string; updated_at: string;
}
export interface BlackboardEntry {
  schema_version: number; id: string; conversation_id: string; delegation_id?: string;
  author_session_id: string; kind: "goal" | "context" | "decision" | "status" | "evidence" | "question" | "answer" | "result" | "blocker";
  content: string; source_references: string[]; supersedes_id?: string; created_at: string;
}
export interface ManagedWorktree {
  schema_version: number; id: string; conversation_id: string; delegation_id: string; project_id: string;
  root: string; branch: string; base_head: string; baseline_head?: string; baseline_sha256: string;
  workspace_mode?: "git_worktree" | "direct_workspace";
  baseline_confidence?: "verified" | "review_required";
  baseline_entries?: Array<{ normalized_path: string; source_kind: "tracked" | "untracked"; size: number; mode: number; sha256: string }>;
  baseline_exclusions?: Array<{ normalized_path: string; reason: string; review_blocker: boolean }>;
  integration_journal?: { id: string; state: string; live_head?: string; touched_paths: Array<{ normalized_path: string; live_sha256?: string; preimage_sha256?: string; preimage_path?: string }>; error?: string; created_at: string; updated_at: string };
  state: "preparing" | "ready" | "dirty" | "integrating" | "integrated" | "conflict" | "retained" | "removed" | "failed";
  revision: number; retained_reason?: string; storage_bytes: number;
  created_at: string; updated_at: string;
}
export interface CloudDiffApply {
  schema_version: number; id: string; project_id: string; task_id: string;
  state: "preparing" | "ready" | "applying" | "applied" | "conflict" | "rolled_back" | "failed" | "retained" | "removed";
  root: string; branch: string; base_head: string; baseline_head: string; baseline_sha256: string;
  baseline_entries: Array<{ normalized_path: string; source_kind: "tracked" | "untracked"; size: number; mode: number; sha256: string }>;
  baseline_exclusions: Array<{ normalized_path: string; reason: string; review_blocker: boolean }>;
  changed_paths: string[];
  touched_paths: Array<{ normalized_path: string; live_content_id?: string; expected_content_id?: string; preimage_sha256?: string; preimage_path?: string }>;
  patch_sha256?: string; output?: string; error?: string; revision: number; created_at: string; updated_at: string;
}
export interface CloudDiffApplyListResponse { applies: CloudDiffApply[]; }
export interface DelegationReview { delegation: AgentDelegation; worktree?: ManagedWorktree; patch: string; }
export interface DelegationRetryMutation { delegation_id: string; retry_of_id: string; replayed: boolean; acknowledgement: string; }
export interface ConversationTeamState {
  sessions: ConversationAgentSession[];
  delegations: AgentDelegation[];
  messages: AgentMessage[];
  blackboard: BlackboardEntry[];
  worktrees: ManagedWorktree[];
  context_checkpoints?: AgentContextCheckpoint[];
  coordination_batches?: CoordinationBatch[];
  wakes?: AgentWake[];
  previews?: ConversationPreview[];
}

export interface ConversationView {
  conversation: Conversation;
  turns: ConversationTurn[];
  messages: ConversationMessage[];
  activities: ConversationActivity[];
  team?: ConversationTeamState;
}

export interface ConversationListResponse { conversations: Conversation[]; }
export interface CreateConversationRequest { title?: string; project_id?: string; execution_preferences: ExecutionPreferences; }
export interface UpdateConversationRequest { title: string; project_id?: string; }
export interface SendConversationMessageRequest { content: string; execution_preferences?: ExecutionPreferences; mentions?: ComposerMentionInput[]; }

export interface CodexReasoningEffort { reasoningEffort: string; description: string; }
export interface CodexServiceTier { id: string; name: string; description: string; }
export interface CodexModel {
  id: string;
  model: string;
  displayName: string;
  description: string;
  hidden: boolean;
  isDefault: boolean;
  defaultReasoningEffort: string;
  supportedReasoningEfforts: CodexReasoningEffort[];
  inputModalities: string[];
  serviceTiers: CodexServiceTier[];
  defaultServiceTier?: string;
}
export type CodexFeatureStage = "beta" | "underDevelopment" | "stable" | "deprecated" | "removed";
export interface CodexFeature {
  name: string;
  stage: CodexFeatureStage;
  displayName?: string;
  description?: string;
  announcement?: string;
  enabled: boolean;
  defaultEnabled: boolean;
}
export interface CodexPermissionProfile { id: string; description?: string; allowed: boolean; }
export interface CodexAccount { authenticated: boolean; account_type?: string; email?: string; plan_type?: string; requires_openai_auth: boolean; }
export type CodexLoginMethod = "browser" | "device_code" | "api_key" | "access_token";
export interface CodexLoginStart { loginId?: string; authUrl?: string; verificationUrl?: string; userCode?: string; }
export type CodexTransportConfig =
  | { kind: "local_stdio" }
  | { kind: "remote_web_socket"; endpoint: string; auth_token_env?: string }
  | { kind: "remote_unix_socket"; socket_path: string };
export interface CodexRateLimitWindow { used_percent: number; remaining_percent: number; resets_at?: number; }
export interface CodexRateLimitBucket extends CodexRateLimitWindow { limit_id: string; limit_name?: string; }
export interface CodexRateLimits {
  plan_type?: string;
  primary?: CodexRateLimitWindow;
  secondary?: CodexRateLimitWindow;
  buckets: Record<string, CodexRateLimitBucket>;
  credits?: unknown;
  refreshed_at: string;
}
export interface CodexHarnessStatus {
  bundled_version: string;
  protocol_version: string;
  schema_sha256: string;
  process_state: "stopped" | "starting" | "ready" | "failed";
  initialized: boolean;
  account_state: string;
  active_threads: number;
  pending_approvals: number;
  pending_requests: number;
  unsupported_requests: number;
  last_unsupported_request?: string;
  last_event?: string;
  last_notification?: string;
  last_unknown_notification?: string;
  restart_count: number;
  configuration_state: string;
  windows_sandbox_state: string;
  mcp_bridge_state: string;
  recovery_state: string;
  transport: CodexTransportConfig;
}

export interface CodexConfigurationStatus {
  requirements: unknown;
  provider_capabilities: unknown;
  windows_sandbox: CodexWindowsSandboxReadiness;
  local_providers?: CodexLocalProviderDiscovery;
}

export interface CodexLocalProviderDiscovery {
  configured_provider?: string;
  providers: CodexLocalProviderStatus[];
}

export interface CodexLocalProviderStatus {
  id: "ollama" | "lmstudio" | string;
  display_name: string;
  default_model: string;
  endpoint: string;
  running: boolean;
  responses_supported: boolean;
  version?: string;
  models: string[];
  detail: string;
}

export type CodexWindowsSandboxSetupMode = "elevated" | "unelevated";
export interface CodexWindowsSandboxReadiness {
  status: "ready" | "notConfigured" | "updateRequired" | string;
}
export interface CodexWindowsSandboxSetupResult {
  mode: CodexWindowsSandboxSetupMode;
  readiness: string;
  config_write_status: string;
}
export interface CodexWindowsSandboxReadGrant {
  path: string;
  permission_profile: string;
}

export interface CodexPendingRequest {
  request_id: number | string;
  method: string;
  params: unknown;
  supported: boolean;
  received_at: string;
}

export interface CodexOwnedThread {
  conversation_id: string;
  codex_thread_id: string;
  ownership_source: string;
  manageable: boolean;
  recovery_state: string;
  last_reconciled_item_id?: string;
}

export interface CodexAppInfo {
  id: string;
  name: string;
  description?: string | null;
  logoUrl?: string | null;
  logoUrlDark?: string | null;
  isAccessible?: boolean;
  isEnabled?: boolean;
  distributionChannel?: string | null;
  pluginDisplayNames?: string[];
}

export interface CodexStableDiscovery {
  skills: unknown[];
  hooks: CodexHooksListEntry[];
  apps: CodexAppInfo[];
  appsAvailable: boolean;
  appsUnavailableReason?: string;
  refreshedAt: string;
}

export interface CodexHookError { path: string; message: string; }
export interface CodexHookMetadata {
  key: string;
  eventName: string;
  handlerType: string;
  command?: string;
  async?: boolean;
  server?: string;
  tool?: string;
  matcher?: string;
  timeoutSec: number;
  statusMessage?: string;
  additionalContextLimit?: number;
  sourcePath: string;
  source: string;
  pluginId?: string;
  displayOrder: number;
  enabled: boolean;
  isManaged: boolean;
  currentHash: string;
  trustStatus: "managed" | "untrusted" | "trusted" | "modified" | string;
}
export interface CodexHooksListEntry {
  cwd: string;
  hooks: CodexHookMetadata[];
  warnings: string[];
  errors: CodexHookError[];
}
export interface CodexHookStateUpdate {
  cwd: string;
  key: string;
  expectedCurrentHash: string;
  enabled?: boolean;
  trustCurrentDefinition: boolean;
}

export type CodexReviewTarget =
  | { type: "uncommittedChanges" }
  | { type: "baseBranch"; branch: string }
  | { type: "commit"; sha: string; title?: string }
  | { type: "custom"; instructions: string };
export type CodexReviewDelivery = "inline" | "detached";
export interface CodexReviewResult { reviewThreadId: string; turn: unknown; delivery: CodexReviewDelivery; }
export interface CodexCommandExecRequest { command: string[]; cwd?: string; timeoutMs?: number; writable?: boolean; }
export interface CodexCommandExecResult { exitCode: number; stdout: string; stderr: string; }
export interface CodexFsWatch { watchId: string; path: string; }

export interface ProviderListItem {
  connection: ProviderConnection;
  catalog?: ProviderModelCatalog;
  profile?: OAuthAccountProfile;
}

export interface OAuthAccountProfile {
  display_name?: string;
  username?: string;
  email?: string;
  avatar_url?: string;
  plan_type?: string;
  account_structure?: string;
  account_role?: string;
  usage?: CodexUsageSummary;
}

export interface CodexUsageWindow {
  used_percent: number;
  remaining_percent: number;
  window_seconds?: number;
  reset_at?: number;
}

export interface CodexAdditionalRateLimit {
  name: string;
  primary_window?: CodexUsageWindow;
  secondary_window?: CodexUsageWindow;
}

export interface CodexUsageSummary {
  primary_window?: CodexUsageWindow;
  secondary_window?: CodexUsageWindow;
  additional_rate_limits: CodexAdditionalRateLimit[];
  allowed?: boolean;
  limit_reached?: boolean;
  credits_unlimited?: boolean;
  has_credits?: boolean;
  overage_limit_reached?: boolean;
}

export interface ProviderListResponse {
  providers: ProviderListItem[];
}

export interface ProxyConversation {
  schema_version: number;
  id: string;
  project_id?: string;
  task_id?: string;
  created_at: string;
  updated_at: string;
}

export interface ProxyUsageSummary {
  conversation_count: number;
  call_count: number;
  usage: {
    input_tokens: number;
    cached_input_tokens: number;
    output_tokens: number;
    reasoning_tokens: number;
    total_tokens: number;
  };
  known_actual_cost_usd: number;
  unknown_cost_call_count: number;
}

export interface ProxyConversationListResponse {
  conversations: ProxyConversation[];
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

export type ExperimentState = "draft" | "ready" | "running" | "awaiting_decision" | "promoted" | "rejected" | "reverted";

export interface SupervisionObservation {
  schema_version: number;
  id: string;
  loop_kind: "fast" | "medium" | "slow";
  signal: string;
  severity_basis_points: number;
  recommended_intervention: string;
  task_id?: string;
  run_id?: string;
  organization_revision_id?: string;
  summary: string;
  evidence: Array<{ method: string; artifact_sha256?: string; event_sequence?: number; detail: string }>;
  observed_at: string;
}

export interface ExperimentDefinition {
  schema_version: number;
  id: string;
  organization_id: string;
  baseline_revision_id: string;
  candidate_revision_id: string;
  mode: "shadow" | "scenario";
  state: ExperimentState;
  hypothesis: string;
  task_classes: string[];
  required_benchmarks: number;
  maximum_regressions: number;
  minimum_confidence: { basis_points: number; sample_size: number };
  observation_ids: string[];
  created_at: string;
  updated_at: string;
}

export interface OrganizationGenome {
  schema_version: number;
  organization_id: string;
  revision_id: string;
  revision: number;
  parent_revision_id?: string;
  inherited_from_revision_id?: string;
  name: string;
  roles: Array<{
    id: string;
    kind: string;
    name: string;
    purpose: string;
    prompt_policy: string;
    model_policy: {
      capability_class: string;
      preferred_models: string[];
      allow_fallback: boolean;
      max_escalation_tier: number;
    };
    tool_capabilities: string[];
    memory_namespace: string;
  }>;
  relationships: Array<{ from: string; to: string; kind: string }>;
  policy: {
    prefer_minimal_organization: boolean;
    max_parallel_agents: number;
    max_total_agents: number;
    max_organizational_overhead_ratio: number;
    default_lease_seconds: number;
  };
  evaluation: {
    require_independent_evaluation: boolean;
    require_objective_evidence: boolean;
    minimum_confidence: number;
    calibration_window: number;
  };
  created_at: string;
}

export interface OrganizationListResponse { organizations: OrganizationGenome[] }
export interface OrganizationHistoryResponse { revisions: OrganizationGenome[] }
export interface OrganizationAssignments {
  global_organization_id: string;
  project_overrides: Record<string, string>;
  updated_at: string;
}
export interface OrganizationAssignmentResponse { assignments: OrganizationAssignments }
export interface OrganizationRevisionPreview {
  organization: OrganizationGenome;
  changes: string[];
  warnings: string[];
}
export interface OrganizationMutationResponse {
  organization: OrganizationGenome;
  event: { sequence: number; kind: string; occurred_at: string; payload?: Record<string, unknown> };
}

export interface AgentRunView {
  run: {
    schema_version: number;
    id: string;
    task_id: string;
    organization_revision_id: string;
    state: "planned" | "running" | "waiting" | "recovering" | "completed" | "failed" | "cancelled";
    metrics: Record<string, number>;
    created_at: string;
    updated_at: string;
    completed_at?: string;
  };
  plan: {
    run_id: string;
    task_id: string;
    organization_revision_id: string;
    items: Array<{ id: string; goal: string; assigned_role: string; state: string; dependencies: string[]; budget: Record<string, unknown>; risk_notes: string[] }>;
    selected_roles: string[];
    selection_reason: string;
    estimated_organizational_overhead: number;
    created_at: string;
  };
  instances: Array<{
    id: string; role_id: string; state: string;
    model?: { provider_connection_id: string; model_id: string; route_reason: string };
    confidence: { basis_points: number; sample_size?: number };
    created_at: string; updated_at: string;
  }>;
  leases: Array<{
    id: string; instance_id: string; work_item_id: string; goal: string; state: string;
    budget: { limit: Record<string, number | null>; consumed_tokens: number; consumed_cost_usd: number; elapsed_seconds: number };
    tool_permissions: string[]; expires_at: string; heartbeat_at: string;
  }>;
  usage: Array<{ id: string; instance_id?: string; kind: string; model_id?: string; input_tokens: number; cached_input_tokens: number; output_tokens: number; cost_usd?: number; duration_ms: number; occurred_at: string }>;
  evaluations: Array<{ id: string; outcome: string; confidence: { basis_points: number }; evidence: Array<{ method: string; passed: boolean; detail: string }> }>;
  results: Array<{
    pinned_route: Record<string, unknown>;
    response: {
      contract_version: number;
      id: string;
      model: string;
      status?: string;
      output: Array<
        | { type: "message"; role: string; content: Array<{ type: "text"; text: string } | Record<string, unknown>> }
        | { type: "refusal"; text: string }
        | Record<string, unknown>
      >;
      usage: Record<string, number>;
      provider_metadata?: unknown;
    };
    model_calls: Array<Record<string, unknown>>;
    tool_outcomes: Array<Record<string, unknown>>;
  }>;
}

export interface GovernorOverview {
  status: RuntimeStatus;
  autonomy: AutonomyPreset;
  assignments: OrganizationAssignments;
  active_organization: OrganizationGenome;
  active_runs: number;
  recent_runs: number;
  subsystems: Array<{ id: string; label: string; state: string; detail: string }>;
}

export interface GovernorControlPolicy {
  schema_version: number;
  governor_model_enabled: boolean;
  requires_owner_approval: boolean;
  allowed_actions: string[];
  denied_actions: string[];
  protected_surfaces: string[];
  implementation_path: string;
}

export type GovernorProposalState = "draft" | "validated" | "awaiting_owner_approval" | "approved" | "implementation_requested" | "evaluating" | "promoted" | "rejected" | "reverted" | "expired";
export interface GovernorProposal {
  schema_version: number;
  id: string;
  organization_id: string;
  baseline_revision_id: string;
  target: "prompt_policy" | "routing_policy" | "organization_topology" | "source_code" | "authority_boundary";
  risk: "low" | "moderate" | "high" | "prohibited";
  state: GovernorProposalState;
  title: string;
  diagnosis: string;
  proposed_change: string;
  expected_benefits: string[];
  expected_risks: string[];
  success_criteria: string[];
  observation_ids: string[];
  rollback_revision_id?: string;
  validation_findings: string[];
  owner_approved_at?: string;
  expires_at: string;
  created_at: string;
  updated_at: string;
}

export interface GovernorProposalListResponse { proposals: GovernorProposal[] }
export interface GovernorProposalMutation { proposal: GovernorProposal; replayed: boolean }
export interface ManagedWorktreeListResponse { worktrees: ManagedWorktree[] }

export interface AgentRunListResponse { runs: AgentRunView[] }

export interface RuntimeStatus {
  service: string;
  version: string;
  state: string;
  advanced_mode: boolean;
  autonomy: AutonomyPreset;
  shadow_experiments: boolean;
  recovered_events: number;
  recovered_tasks: number;
  recovered_tool_leases: number;
  recovered_agent_runs: number;
  recovered_agent_instances: number;
  recovered_agent_leases: number;
  recovered_schedule_occurrences: number;
  recovered_platform_sessions: number;
  expired_platform_sessions: number;
  recovered_computer_action_batches: number;
}

export interface PortableOrganizationGenome {
  schema_version: number;
  source: {
    organization_id: string;
    revision_id: string;
    revision: number;
    exported_at: string;
  };
  definition: {
    name: string;
    roles: Array<{
      key: string;
      kind: string;
      name: string;
      purpose: string;
      prompt_policy: string;
      model_policy: {
        capability_class: string;
        preferred_models: string[];
        allow_fallback: boolean;
        max_escalation_tier: number;
      };
      tool_capabilities: string[];
      memory_namespace: string;
    }>;
    relationships: Array<{ from: string; to: string; kind: string }>;
    policy: OrganizationGenome["policy"];
    evaluation: OrganizationGenome["evaluation"];
  };
  content_sha256: string;
}

export interface PortableOrganizationImportMutation {
  organization: OrganizationGenome;
  portable_sha256: string;
  replayed: boolean;
  event?: { sequence: number; kind: string; occurred_at: string; payload?: Record<string, unknown> };
}

export interface PortableOrganizationComparison {
  equivalent: boolean;
  left_sha256: string;
  right_sha256: string;
  sections_changed: Array<"name" | "policy" | "evaluation">;
  roles_added: string[];
  roles_removed: string[];
  roles_changed: string[];
  relationships_added: string[];
  relationships_removed: string[];
}

export type OrganizationPublisherState = "trusted" | "revoked";
export type OrganizationShareReviewState = "pending" | "imported" | "rejected";

export interface TrustedOrganizationPublisher {
  schema_version: number;
  id: string;
  display_name: string;
  public_key_base64: string;
  registries: string[];
  state: OrganizationPublisherState;
  revision: number;
  reason: string;
  created_at: string;
  updated_at: string;
}

export interface OrganizationShareSignature {
  scheme: string;
  signature_base64: string;
}

export interface OrganizationSharePackage {
  schema_version: number;
  registry: string;
  package: string;
  publisher: string;
  portable: PortableOrganizationGenome;
  signature: OrganizationShareSignature;
}

export interface VerifiedOrganizationShareEvidence {
  schema_version: number;
  registry: string;
  package: string;
  publisher: string;
  publisher_key_sha256: string;
  portable_sha256: string;
  package_sha256: string;
  signature_sha256: string;
}

export interface OrganizationShareReview {
  schema_version: number;
  id: string;
  package: OrganizationSharePackage;
  evidence: VerifiedOrganizationShareEvidence;
  state: OrganizationShareReviewState;
  import_id?: string;
  organization_id?: string;
  decision_reason?: string;
  created_at: string;
  updated_at: string;
}

export interface OrganizationPublisherMutation {
  publisher: TrustedOrganizationPublisher;
  replayed: boolean;
  event?: { sequence: number; kind: string; occurred_at: string; payload?: Record<string, unknown> };
}

export interface OrganizationShareReviewMutation {
  review: OrganizationShareReview;
  replayed: boolean;
  event?: { sequence: number; kind: string; occurred_at: string; payload?: Record<string, unknown> };
}

export interface OrganizationShareImportMutation {
  review: OrganizationShareReview;
  organization: OrganizationGenome;
  replayed: boolean;
  events: Array<{ sequence: number; kind: string; occurred_at: string; payload?: Record<string, unknown> }>;
}

export interface SupervisionObservationListResponse { observations: SupervisionObservation[] }
export interface ExperimentListResponse { experiments: ExperimentDefinition[] }
export interface ShadowExecutionListResponse { receipts: Array<Record<string, unknown>> }
export interface ExperimentBenchmarkListResponse { benchmarks: Array<Record<string, unknown>> }
export interface ExperimentDecisionListResponse { decisions: Array<Record<string, unknown>> }

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
  thread_id?: string;
  constraints: string[];
  success_conditions: string[];
  risk?: RiskLevel;
  budget?: TaskEnvelope["budget"];
}

export interface ProjectModel {
  schema_version: number;
  id: string;
  project_id: string;
  root: string;
  fingerprint: string;
  observed_at: string;
  repository: { is_git: boolean; branch?: string; head?: string; dirty: boolean; recent_commits: Array<{ id: string; summary: string }> };
  languages: string[];
  build_systems: string[];
  test_systems: string[];
  package_managers: string[];
  services: string[];
  ci_systems: string[];
  important_directories: string[];
  commands: Array<{ name: string; command: string; executable: string; arguments: string[]; source: string; confidence_basis: string }>;
  instructions: { conflicts: unknown[] };
  existing_memory?: string;
  world: {
    nodes: Array<{
      id: string;
      kind: "service" | "database" | "domain" | "api" | "deployment" | "dependency" | "repository" | "runtime" | "command";
      label: string;
      source: string;
    }>;
    relationships: Array<{ from: string; relation: string; to: string; source: string }>;
  };
  watched_inputs: Record<string, string>;
}

export interface ProjectMutation {
  project: ProjectModel;
}

export interface ProjectListResponse {
  projects: ProjectModel[];
  archived_projects: ProjectModel[];
}

export interface ProjectTopologyPlan {
  schema_version: number;
  sha256: string;
  project_id: string;
  project_snapshot_id: string;
  status: "ready" | "empty" | "blocked_by_cycle";
  stages: Array<{
    index: number;
    nodes: Array<{
      id: string;
      kind: ProjectModel["world"]["nodes"][number]["kind"];
      label: string;
      source: string;
    }>;
  }>;
  unresolved_nodes: Array<{
    id: string;
    kind: ProjectModel["world"]["nodes"][number]["kind"];
    label: string;
    source: string;
  }>;
}

export type ReviewedTopologyPlanState = "reviewed" | "running" | "waiting" | "succeeded" | "failed" | "uncertain" | "cancelled";
export type ReviewedTopologyStepState = "pending" | "executing" | "succeeded" | "failed" | "uncertain" | "skipped";

export interface ReviewedTopologyStep {
  id: string;
  stage_index: number;
  name: string;
  command_name: string;
  command: string;
  executable: string;
  arguments: string[];
  source: string;
  timeout_seconds: number;
  state: ReviewedTopologyStepState;
  detail: string;
  exit_code?: number;
  log_artifact_sha256: string[];
}

export interface ReviewedTopologyPlan {
  schema_version: number;
  id: string;
  definition_hash: string;
  project_id: string;
  project_snapshot_id: string;
  project_fingerprint: string;
  topology_sha256: string;
  display_name: string;
  reason: string;
  state: ReviewedTopologyPlanState;
  steps: ReviewedTopologyStep[];
  created_at: string;
  updated_at: string;
}

export interface ReviewedTopologyStepRequest {
  id: string;
  stage_index: number;
  name: string;
  command_name: string;
  command: string;
  timeout_seconds: number;
}

export interface ReviewedTopologyPlanCreateRequest {
  id: string;
  project_snapshot_id: string;
  topology_sha256: string;
  display_name: string;
  reason: string;
  steps: ReviewedTopologyStepRequest[];
}

export interface ReviewedTopologyPlanMutation {
  plan: ReviewedTopologyPlan;
  replayed: boolean;
  events: Array<{ sequence: number; kind: string; occurred_at: string }>;
  approval_id?: string;
  outcomes: unknown[];
}

export interface ReviewedTopologyPlanListResponse { plans: ReviewedTopologyPlan[]; }

export type MemoryState = "new" | "working" | "durable" | "decaying" | "superseded" | "retracted";
export type MemoryLifecycleAction = "reinforce" | "decay" | "retract";

export interface ClaimProvenance {
  source_kind: "user" | "file" | "tool_observation" | "provider_observation" | "derived" | "imported";
  source: string;
  task_id?: string;
  artifact_sha256?: string;
}

export interface MemoryClaim {
  schema_version: number;
  id: string;
  project_id: string;
  namespace: string;
  key: string;
  value: unknown;
  state: MemoryState;
  confidence: number;
  provenance: ClaimProvenance;
  observed_at: string;
  fresh_until?: string;
  supersedes?: string;
  correction_count: number;
  usefulness_count: number;
}

export interface MemoryClaimMutation {
  claim: MemoryClaim;
  previous?: MemoryClaim;
  event: { sequence: number; kind: string; occurred_at: string };
}

export interface MemoryClaimListResponse { claims: MemoryClaim[]; }
export interface MemoryClaimHistoryResponse { claims: MemoryClaim[]; }
export interface ProjectMemoryTransfer {
  schema_version: number;
  id: string;
  source_project_id: string;
  source_claim_id: string;
  source_claim_sha256: string;
  target_project_id: string;
  target_claim_id: string;
  reason: string;
  created_at: string;
}
export interface ProjectMemoryTransferPreview {
  claim: MemoryClaim;
  source_claim_sha256: string;
}
export interface ProjectMemoryTransferMutation {
  transfer: ProjectMemoryTransfer;
  claim: MemoryClaim;
  replayed: boolean;
  events: Array<{ sequence: number; kind: string; occurred_at: string }>;
}
export interface ProjectMemoryTransferListResponse { transfers: ProjectMemoryTransfer[]; }
export interface ManagedMemoryResponse { markdown: string; }

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
  definition_hash: string;
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
  schedule_id: string;
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

export type SiteEnvironment = "development" | "preview" | "production";
export type SiteState = "active" | "monitoring_paused" | "archived";
export interface SiteDeploymentCommand { executable: string; arguments: string[]; working_directory: string; }
export interface Site {
  schema_version: number;
  id: string;
  definition_hash: string;
  revision: number;
  project_id: string;
  display_name: string;
  environment: SiteEnvironment;
  public_url: string;
  health_url: string;
  hosting_provider: string;
  deployment_command?: SiteDeploymentCommand;
  credential_reference?: string;
  state: SiteState;
  created_at: string;
  updated_at: string;
}
export interface CodexSiteInventoryItem {
  id: string;
  display_name: string;
  description?: string;
  public_url: string;
  screenshot_url?: string;
  status: string;
  created_at: string;
  updated_at: string;
}
export interface CodexSiteInventory { sites: CodexSiteInventoryItem[]; }
export interface SiteRevision { id: string; site_id: string; revision: number; definition_hash: string; reason: string; created_at: string; }
export interface SiteHealthCheck { id: string; site_id: string; state: "healthy" | "degraded" | "unavailable"; status_code?: number; latency_ms?: number; detail: string; response_artifact_sha256?: string; checked_at: string; }
export interface SiteDeployment { id: string; site_id: string; site_revision: number; state: "executing" | "succeeded" | "failed" | "uncertain"; detail: string; log_artifact_sha256: string[]; requested_at: string; finished_at?: string; }
export interface SiteMonitor { schema_version: number; site_id: string; schedule_id: string; interval_seconds: number; expected_status: number; last_state?: SiteHealthCheck["state"]; created_at: string; updated_at: string; }
export interface CreateSiteRequest {
  site_id: string;
  project_id: string;
  display_name: string;
  environment: SiteEnvironment;
  public_url: string;
  health_url: string;
  hosting_provider: string;
  deployment_command?: SiteDeploymentCommand;
  credential_reference?: string;
  state: SiteState;
}
export interface UpdateSiteRequest extends Omit<CreateSiteRequest, "site_id" | "project_id"> {
  expected_revision: number;
  reason: string;
}
export interface SiteListResponse { sites: Site[]; }
export interface SiteRevisionListResponse { revisions: SiteRevision[]; }
export interface SiteHealthListResponse { checks: SiteHealthCheck[]; }
export interface SiteDeploymentListResponse { deployments: SiteDeployment[]; }
export interface SiteHealthMutation { check: SiteHealthCheck; replayed: boolean; }
export interface SiteDeploymentMutation { deployment: SiteDeployment; replayed: boolean; }
export interface SiteMutation { site: Site; replayed: boolean; }
export interface SiteMonitorMutation { monitor: SiteMonitor; schedule: ScheduleDefinition; replayed: boolean; }
export interface CreateSiteMonitorRequest { schedule_id: string; interval_seconds: number; expected_status: number; first_run_at?: string; }

export type SshHostState = "active" | "disabled" | "archived";
export type SshOperationKind =
  | { kind: "inspect" }
  | { kind: "execute"; executable: string; arguments: string[]; working_directory?: string }
  | { kind: "upload"; local_path: string; remote_path: string; overwrite: boolean; expected_sha256: string }
  | { kind: "download"; remote_path: string; local_path: string; overwrite: boolean; expected_sha256?: string };
export interface SshHost { id: string; revision: number; definition_hash: string; display_name: string; hostname: string; port: number; username: string; known_hosts_entry: string; host_key_sha256: string; capabilities: string[]; state: SshHostState; created_at: string; updated_at: string; }
export interface SshOperation { id: string; host_id: string; host_revision: number; state: "executing" | "succeeded" | "failed" | "uncertain"; kind: SshOperationKind; detail: string; exit_code?: number; artifact_sha256: string[]; created_at: string; updated_at: string; }
export interface CreateSshHostRequest { host_id: string; display_name: string; hostname: string; port: number; username: string; platform: "posix"; authentication: "agent"; known_hosts_entry: string; host_key_sha256: string; capabilities: string[]; state: SshHostState; }
export interface UpdateSshHostRequest extends Omit<CreateSshHostRequest, "host_id"> { expected_revision: number; reason: string; }
export interface RunSshOperationRequest { operation_id: string; task_id?: string; operation: SshOperationKind; timeout_seconds: number; }
export interface SshHostListResponse { hosts: SshHost[]; }
export interface SshOperationListResponse { operations: SshOperation[]; }
export interface SshHostMutation { host: SshHost; replayed: boolean; }
export interface SshOperationMutation { operation: SshOperation; replayed: boolean; }
export type SshWorkflowState = "planned" | "running" | "waiting" | "succeeded" | "failed" | "uncertain" | "cancelled";
export type SshWorkflowStepState = "pending" | "succeeded" | "failed" | "uncertain" | "skipped";
export interface SshWorkflowStepRequest { id: string; name: string; host_id?: string; host_revision?: number; required_capabilities: string[]; dependencies: string[]; operation: SshOperationKind; timeout_seconds: number; }
export interface CreateSshWorkflowRequest { id: string; display_name: string; task_id?: string; steps: SshWorkflowStepRequest[]; }
export interface SshWorkflowStep extends Omit<SshWorkflowStepRequest, "host_id" | "host_revision"> { host_id: string; host_revision: number; routed_automatically: boolean; operation_id: string; state: SshWorkflowStepState; detail: string; }
export interface SshWorkflow { schema_version: number; id: string; definition_hash: string; display_name: string; task_id?: string; state: SshWorkflowState; steps: SshWorkflowStep[]; created_at: string; updated_at: string; }
export interface SshWorkflowListResponse { workflows: SshWorkflow[]; }
export interface SshWorkflowMutation { workflow: SshWorkflow; replayed: boolean; }

export type PlatformCapabilityKind = "browser_automation" | "computer_use" | "native_desktop" | "ssh_client" | "sites";
export type PlatformCapabilityAvailability = "available" | "unavailable" | "unsupported";
export interface PlatformCapabilitySnapshot {
  schema_version: number;
  id: string;
  os: string;
  architecture: string;
  capabilities: Array<{
    kind: PlatformCapabilityKind;
    availability: PlatformCapabilityAvailability;
    detail: string;
  }>;
  observed_at: string;
}

export type PlatformSessionKind = "browser" | "computer_use";
export type ComputerUseCapability = "screen_read" | "mouse" | "keyboard" | "clipboard" | "window_list" | "process_inspect";
export interface ComputerUseRegion { x: number; y: number; width: number; height: number; }
export interface ComputerUseTarget { display_id?: number; region?: ComputerUseRegion; }
export type PlatformSessionState = "requested" | "active" | "handed_off" | "interrupted" | "closed" | "failed";
export interface PlatformSession {
  schema_version: number;
  id: string;
  definition_hash: string;
  kind: PlatformSessionKind;
  state: PlatformSessionState;
  owner: "mcg" | "local_owner";
  project_id?: string;
  task_id?: string;
  display_name: string;
  allowed_origins: string[];
  computer_capabilities: ComputerUseCapability[];
  computer_target?: ComputerUseTarget;
  created_at: string;
  updated_at: string;
  expires_at: string;
  closed_at?: string;
  recovery_count: number;
}

export interface PlatformSessionEvent {
  id: string;
  session_id: string;
  from_state?: PlatformSessionState;
  to_state: PlatformSessionState;
  owner: "mcg" | "local_owner";
  reason: string;
  occurred_at: string;
}

export interface CreatePlatformSessionRequest {
  session_id: string;
  kind: PlatformSessionKind;
  project_id?: string;
  task_id?: string;
  display_name: string;
  allowed_origins: string[];
  computer_capabilities?: ComputerUseCapability[];
  computer_target?: ComputerUseTarget;
  lifetime_seconds: number;
}

export interface PlatformSessionListResponse { sessions: PlatformSession[]; }
export interface PlatformSessionHistoryResponse { events: PlatformSessionEvent[]; }
export type PersistentBrowserMode = "headless" | "visible";
export interface PersistentBrowserObservation {
  session_id: string;
  mode: PersistentBrowserMode;
  safe_url: string;
  title?: string;
  handed_off: boolean;
  denied_requests: number;
}
export interface PlatformBrowserMutation {
  session: PlatformSession;
  browser: PersistentBrowserObservation;
}
export interface ComputerUseObservation {
  session_id: string;
  display_id: number;
  region: ComputerUseRegion;
  capabilities: ComputerUseCapability[];
  handed_off: boolean;
}
export interface ComputerUseFrame {
  observation: ComputerUseObservation;
  sha256: string;
  media_type: string;
  png_base64: string;
  captured_at: string;
}
export type ComputerUseAction =
  | { type: "move"; x: number; y: number }
  | { type: "click"; x: number; y: number; button: "left" | "middle" | "right" }
  | { type: "scroll"; x: number; y: number; delta_x: number; delta_y: number }
  | { type: "type_text"; text: string }
  | { type: "key"; key: "enter" | "tab" | "escape" | "backspace" | "delete" | "space" | "home" | "end" | "page_up" | "page_down" | "arrow_up" | "arrow_down" | "arrow_left" | "arrow_right" };
export interface ApplyComputerActionsRequest { batch_id: string; expected_frame_sha256: string; actions: ComputerUseAction[]; }
export interface ComputerUseBatchResult {
  prior_frame_sha256: string;
  actions_executed: number;
  frame: ComputerUseFrame;
}
export interface ComputerActionBatch {
  id: string;
  session_id: string;
  request_hash: string;
  state: "executing" | "succeeded" | "rejected" | "uncertain";
  action_count: number;
  result_frame_sha256?: string;
  detail: string;
  created_at: string;
  updated_at: string;
}
export interface PlatformComputerActionMutation {
  batch: ComputerActionBatch;
  replayed: boolean;
  result?: ComputerUseBatchResult;
}
export interface PlatformComputerMutation { session: PlatformSession; computer: ComputerUseObservation; }

export interface TaskEventsResponse {
  events: Array<{
    sequence: number;
    kind: string;
    occurred_at: string;
    payload?: {
      outcome?: "passed" | "failed" | "inconclusive";
      conditions?: Array<{ passed: boolean; detail: string }>;
      evidence?: Array<{ method: string; detail: string }>;
      run?: { id: string };
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
  run: AgentRunView;
  execution: {
    state: "not_started" | "awaiting_approval" | "awaiting_evaluation" | "blocked" | "completed" | "failed";
    detail: string;
  };
}

export interface RunReplayReport {
  schema_version: number;
  run_id: string;
  task_id: string;
  organization_revision_id: string;
  terminal: boolean;
  side_effect_free: boolean;
  evidence_sha256: string;
  counts: {
    events: number;
    tool_audit_records: number;
    work_items: number;
    instances: number;
    leases: number;
    usage_records: number;
    evaluations: number;
    turn_results: number;
  };
  invariants: Array<{ code: string; passed: boolean; detail: string }>;
  evidence: unknown;
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

export function getLocalServiceStatus(): Promise<LocalServiceStatus> {
  return invoke("local_service_status");
}

export function runDiagnostics(): Promise<DiagnosticReport> {
  return invoke("run_diagnostics");
}

export function getUpdateConfiguration(): Promise<UpdateConfiguration> {
  return invoke("update_configuration");
}

export function checkForUpdate(): Promise<UpdateCheck> {
  return invoke("check_for_update");
}

export function installUpdate(expectedVersion: string, backupId: string): Promise<void> {
  return invoke("install_update", { expectedVersion, backupId });
}

export function getUserSettings(): Promise<UserSettings> {
  return invoke("get_user_settings");
}

export function updateUserSettings(request: UpdateUserSettingsRequest): Promise<UserSettings> {
  return invoke("update_user_settings", { request });
}

export function loadUserSettingsHistory(): Promise<UserSettingsHistoryResponse> {
  return invoke("user_settings_history");
}

export function listBackups(): Promise<BackupListResponse> {
  return invoke("list_backups");
}

export function createBackup(backupId: string): Promise<BackupSummary> {
  return invoke("create_backup", { backupId });
}

export function createEncryptedBackup(backupId: string, passphrase: string): Promise<BackupSummary> {
  return invoke("create_encrypted_backup", { backupId, passphrase });
}

export function verifyBackup(backupId: string): Promise<BackupSummary> {
  return invoke("verify_backup", { backupId });
}

export function listPlugins(): Promise<PluginListResponse> {
  return invoke("list_plugins");
}

export function installLocalPlugin(request: InstallLocalPluginRequest): Promise<PluginInstallation> {
  return invoke("install_local_plugin", { request });
}

export function installRegistryPlugin(request: InstallRegistryPluginRequest): Promise<PluginInstallation> {
  return invoke("install_registry_plugin", { request });
}

export function setPluginGrants(pluginId: string, request: SetPluginGrantsRequest): Promise<PluginInstallation> {
  return invoke("set_plugin_grants", { pluginId, request });
}

export function setPluginEnabled(pluginId: string, expectedRevision: number, enabled: boolean): Promise<PluginInstallation> {
  return invoke("set_plugin_enabled", { pluginId, request: { expected_revision: expectedRevision, enabled } });
}

export function invokePlugin(pluginId: string, invocationId: string, input: unknown, taskId?: string): Promise<PluginInvocationMutation> {
  return invoke("invoke_plugin", { pluginId, invocationId, request: { task_id: taskId, input } });
}

export function getPluginInvocation(pluginId: string, invocationId: string): Promise<PluginInvocationHistoryResponse> {
  return invoke("get_plugin_invocation", { pluginId, invocationId });
}

export function providerProxyUsage(): Promise<ProxyUsageSummary> {
  return invoke("provider_proxy_usage");
}

export function providerProxyConversations(): Promise<ProxyConversationListResponse> {
  return invoke("provider_proxy_conversations");
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

export function disconnectProvider(connectionId: string): Promise<void> {
  return invoke("disconnect_provider", { connectionId });
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

export function listConversations(): Promise<ConversationListResponse> {
  return invoke("list_conversations", { includeArchived: true });
}

export function createConversation(request: CreateConversationRequest): Promise<Conversation> {
  return invoke("create_conversation", { request });
}

export function getConversation(conversationId: string): Promise<ConversationView> {
  return invoke("get_conversation", { conversationId });
}

export function updateConversation(conversationId: string, request: UpdateConversationRequest): Promise<Conversation> {
  return invoke("update_conversation", { conversationId, request });
}

export function archiveConversation(conversationId: string, archived = true): Promise<Conversation> {
  return invoke("archive_conversation", { conversationId, archived });
}

export function sendConversationMessage(conversationId: string, request: SendConversationMessageRequest): Promise<ConversationView> {
  return invoke("send_conversation_message", { conversationId, request });
}

export function searchProjectMentions(projectId: string, query = "", limit = 50): Promise<MentionCandidateListResponse> {
  return invoke("search_project_mentions", { projectId, query, limit });
}

export function pickMentionFiles(): Promise<string[]> { return invoke("pick_mention_files"); }
export function pickMentionFolder(): Promise<string | null> { return invoke("pick_mention_folder"); }
export function pickWritableFolder(): Promise<string | null> { return invoke("pick_writable_folder"); }
export function pickSandboxReadFolder(): Promise<string | null> { return invoke("pick_sandbox_read_folder"); }

export function updateConversationMode(conversationId: string, mode: ConversationMode): Promise<ConversationView> {
  return invoke("update_conversation_mode", { conversationId, request: { mode } });
}
export function updateConversationGoal(conversationId: string, request: { objective?: string; status?: ConversationGoalStatus }): Promise<ConversationView> {
  return invoke("update_conversation_goal", { conversationId, request });
}
export function clearConversationGoal(conversationId: string): Promise<ConversationView> {
  return invoke("clear_conversation_goal", { conversationId });
}
export function answerConversationInput(conversationId: string, requestId: unknown, answers: Record<string, { answers: string[] }>): Promise<ConversationView> {
  return invoke("answer_conversation_input", { conversationId, request: { request_id: requestId, answers } });
}

export function getConversationTeam(conversationId: string): Promise<ConversationTeamState> {
  return invoke("get_conversation_team", { conversationId });
}

export function listManagedWorktrees(): Promise<ManagedWorktreeListResponse> {
  return invoke("list_managed_worktrees");
}

export function prepareCloudDiffApply(projectId: string, taskId: string): Promise<CloudDiffApply> {
  return invoke("prepare_cloud_diff_apply", { projectId, taskId });
}

export function listCloudDiffApplies(projectId: string): Promise<CloudDiffApplyListResponse> {
  return invoke("list_cloud_diff_applies", { projectId });
}

export function applyCloudDiff(applyId: string, expectedRevision: number): Promise<CloudDiffApply> {
  return invoke("apply_cloud_diff", { applyId, expectedRevision });
}

export function discardCloudDiffApply(applyId: string, expectedRevision: number): Promise<CloudDiffApply> {
  return invoke("discard_cloud_diff_apply", { applyId, expectedRevision });
}

export function reviewConversationDelegation(conversationId: string, delegationId: string): Promise<DelegationReview> {
  return invoke("review_conversation_delegation", { conversationId, delegationId });
}

export function cancelConversationDelegation(conversationId: string, delegationId: string, expectedRevision: number): Promise<ConversationView> {
  return invoke("cancel_conversation_delegation", { conversationId, delegationId, expectedRevision });
}

export function retryConversationDelegation(conversationId: string, delegationId: string, expectedRevision: number, idempotencyKey: string): Promise<DelegationRetryMutation> {
  return invoke("retry_conversation_delegation", { conversationId, delegationId, expectedRevision, idempotencyKey });
}

export function applyConversationDelegation(conversationId: string, delegationId: string, expectedRevision: number): Promise<ConversationView> {
  return invoke("apply_conversation_delegation", { conversationId, delegationId, expectedRevision });
}

export function discardConversationDelegation(conversationId: string, delegationId: string, expectedRevision: number): Promise<ConversationView> {
  return invoke("discard_conversation_delegation", { conversationId, delegationId, expectedRevision });
}

export function startConversationPreview(conversationId: string, previewId: string): Promise<ConversationPreview> {
  return invoke("start_conversation_preview", { conversationId, previewId });
}

export function stopConversationPreview(conversationId: string, previewId: string): Promise<ConversationPreview> {
  return invoke("stop_conversation_preview", { conversationId, previewId });
}

export function interruptConversationTurn(conversationId: string, turnId: string): Promise<ConversationTurn> {
  return invoke("interrupt_conversation_turn", { conversationId, turnId });
}

export function steerConversationTurn(conversationId: string, turnId: string, content: string, mentions: ComposerMentionInput[] = []): Promise<ConversationView> {
  return invoke("steer_conversation_turn", { conversationId, turnId, content, mentions });
}

export function getCodexModels(): Promise<CodexModel[]> { return invoke("codex_models"); }
export function getCodexFeatures(): Promise<CodexFeature[]> { return invoke("codex_features"); }
export function getCodexPermissionProfiles(projectId?: string): Promise<CodexPermissionProfile[]> { return invoke("codex_permission_profiles", { projectId }); }
export function setCodexFeature(name: string, enabled: boolean): Promise<CodexFeature[]> { return invoke("set_codex_feature", { name, enabled }); }
export function getCodexAccount(): Promise<CodexAccount> { return invoke("codex_account"); }
export function getCodexRateLimits(): Promise<CodexRateLimits> { return invoke("codex_rate_limits"); }
export function getCodexHarnessStatus(): Promise<CodexHarnessStatus> { return invoke("codex_harness_status"); }
export function getCodexAccountUsage(): Promise<unknown> { return invoke("codex_account_usage"); }
export function getCodexStableDiscovery(cwds: string[] = [], forceReload = false): Promise<CodexStableDiscovery> { return invoke("codex_stable_discovery", { cwds, forceReload }); }
export function updateCodexHookState(request: CodexHookStateUpdate): Promise<CodexStableDiscovery> { return invoke("update_codex_hook_state", { request }); }
export function getCodexConfigurationStatus(): Promise<CodexConfigurationStatus> { return invoke("codex_configuration_status"); }
export function setupWindowsSandbox(mode: CodexWindowsSandboxSetupMode, projectId?: string): Promise<CodexWindowsSandboxSetupResult> { return invoke("setup_windows_sandbox", { mode, projectId }); }
export function grantWindowsSandboxReadDirectory(projectId: string, path: string, permissionProfile?: string): Promise<CodexWindowsSandboxReadGrant> { return invoke("grant_windows_sandbox_read_directory", { projectId, path, permissionProfile }); }
export function getCodexPendingRequests(): Promise<CodexPendingRequest[]> { return invoke("codex_pending_requests"); }
export function getCodexOwnedThreads(): Promise<CodexOwnedThread[]> { return invoke("codex_owned_threads"); }
export function reconcileOwnedCodexThread(conversationId: string): Promise<unknown> { return invoke("reconcile_owned_codex_thread", { conversationId }); }
export function forkOwnedCodexThread(conversationId: string): Promise<unknown> { return invoke("fork_owned_codex_thread", { conversationId }); }
export function compactOwnedCodexThread(conversationId: string): Promise<unknown> { return invoke("compact_owned_codex_thread", { conversationId }); }
export function unsubscribeOwnedCodexThread(conversationId: string): Promise<unknown> { return invoke("unsubscribe_owned_codex_thread", { conversationId }); }
export function getOwnedCodexThreadHistory(conversationId: string): Promise<unknown> { return invoke("owned_codex_thread_history", { conversationId }); }
export function startOwnedCodexReview(conversationId: string, target: CodexReviewTarget, delivery: CodexReviewDelivery): Promise<CodexReviewResult> { return invoke("start_owned_codex_review", { conversationId, target, delivery }); }
export function execGovernedCodexCommand(conversationId: string, request: CodexCommandExecRequest): Promise<CodexCommandExecResult> { return invoke("exec_governed_codex_command", { conversationId, request }); }
export function watchConversationProject(conversationId: string): Promise<CodexFsWatch> { return invoke("watch_conversation_project", { conversationId }); }
export function unwatchConversationProject(conversationId: string): Promise<unknown> { return invoke("unwatch_conversation_project", { conversationId }); }
export function startCodexLogin(method: CodexLoginMethod = "browser", credential?: string): Promise<CodexLoginStart | CodexAccount> { return invoke("start_codex_login", { method, credential }); }
export function updateCodexTransport(expectedRevision: number, endpoint?: string, authTokenEnv?: string, reason = "Updated Codex transport from desktop"): Promise<UserSettings> { return invoke("update_codex_transport", { expectedRevision, endpoint, authTokenEnv, reason }); }
export function cancelCodexLogin(loginId: string): Promise<unknown> { return invoke("cancel_codex_login", { loginId }); }
export function logoutCodexAccount(): Promise<unknown> { return invoke("logout_codex_account"); }

export function listSchedules(): Promise<ScheduleListResponse> {
  return invoke("list_schedules");
}

export function listSites(): Promise<SiteListResponse> { return invoke("list_sites"); }
export function listCodexSites(): Promise<CodexSiteInventory> { return invoke("list_codex_sites"); }
export function registerSite(request: CreateSiteRequest): Promise<Site> { return invoke("register_site", { request }); }
export function updateSite(siteId: string, request: UpdateSiteRequest): Promise<SiteMutation> { return invoke("update_site", { siteId, request }); }
export function loadSiteRevisions(siteId: string): Promise<SiteRevisionListResponse> { return invoke("site_revisions", { siteId }); }
export function loadSiteHealth(siteId: string): Promise<SiteHealthListResponse> { return invoke("site_health_history", { siteId }); }
export function checkSiteHealth(siteId: string, checkId: string): Promise<SiteHealthMutation> { return invoke("check_site_health", { siteId, request: { check_id: checkId } }); }
export function getSiteMonitor(siteId: string): Promise<SiteMonitor> { return invoke("get_site_monitor", { siteId }); }
export function createSiteMonitor(siteId: string, request: CreateSiteMonitorRequest): Promise<SiteMonitorMutation> { return invoke("create_site_monitor", { siteId, request }); }
export function loadSiteDeployments(siteId: string): Promise<SiteDeploymentListResponse> { return invoke("site_deployment_history", { siteId }); }
export function deploySite(siteId: string, deploymentId: string): Promise<SiteDeploymentMutation> { return invoke("deploy_site", { siteId, request: { deployment_id: deploymentId } }); }
export interface SiteBrowserBounds { x: number; y: number; width: number; height: number; }
export type SiteBrowserAction = "back" | "forward" | "reload";
export function openSiteInAppBrowser(tabId: string, url: string, bounds: SiteBrowserBounds): Promise<void> { return invoke("open_site_in_app_browser", { tabId, url, bounds }); }
export function updateSiteInAppBrowser(tabId: string, bounds: SiteBrowserBounds | null, visible: boolean): Promise<void> { return invoke("update_site_in_app_browser", { tabId, bounds, visible }); }
export function closeSiteInAppBrowser(tabId: string): Promise<void> { return invoke("close_site_in_app_browser", { tabId }); }
export function controlSiteInAppBrowser(tabId: string, action: SiteBrowserAction): Promise<void> { return invoke("control_site_in_app_browser", { tabId, action }); }
export function openSiteExternalUrl(url: string): Promise<void> { return invoke("open_site_external_url", { url }); }
export function listSshHosts(): Promise<SshHostListResponse> { return invoke("list_ssh_hosts"); }
export function registerSshHost(request: CreateSshHostRequest): Promise<SshHost> { return invoke("register_ssh_host", { request }); }
export function updateSshHost(hostId: string, request: UpdateSshHostRequest): Promise<SshHostMutation> { return invoke("update_ssh_host", { hostId, request }); }
export function loadSshOperations(hostId: string): Promise<SshOperationListResponse> { return invoke("ssh_operation_history", { hostId }); }
export function runSshOperation(hostId: string, request: RunSshOperationRequest): Promise<SshOperationMutation> { return invoke("run_ssh_operation", { hostId, request }); }
export function listSshWorkflows(): Promise<SshWorkflowListResponse> { return invoke("list_ssh_workflows"); }
export function createSshWorkflow(request: CreateSshWorkflowRequest): Promise<SshWorkflowMutation> { return invoke("create_ssh_workflow", { request }); }
export function runSshWorkflow(workflowId: string): Promise<SshWorkflowMutation> { return invoke("run_ssh_workflow", { workflowId }); }
export function cancelSshWorkflow(workflowId: string): Promise<SshWorkflowMutation> { return invoke("cancel_ssh_workflow", { workflowId }); }

export function loadPlatformCapabilities(): Promise<PlatformCapabilitySnapshot> {
  return invoke("platform_capabilities");
}

export function listPlatformSessions(): Promise<PlatformSessionListResponse> {
  return invoke("list_platform_sessions");
}

export function requestPlatformSession(request: CreatePlatformSessionRequest): Promise<PlatformSession> {
  return invoke("request_platform_session", { request });
}

export function loadPlatformSessionHistory(sessionId: string): Promise<PlatformSessionHistoryResponse> {
  return invoke("platform_session_history", { sessionId });
}

export function closePlatformSession(sessionId: string, reason: string): Promise<PlatformSession> {
  return invoke("close_platform_session", { sessionId, request: { reason } });
}

export function attachBrowserSession(sessionId: string, mode: PersistentBrowserMode): Promise<PlatformBrowserMutation> {
  return invoke("attach_browser_session", { sessionId, request: { mode } });
}

export function navigateBrowserSession(sessionId: string, url: string): Promise<PersistentBrowserObservation> {
  return invoke("navigate_browser_session", { sessionId, request: { url } });
}

export function handoffBrowserSession(sessionId: string): Promise<PlatformBrowserMutation> {
  return invoke("handoff_browser_session", { sessionId });
}

export function reclaimBrowserSession(sessionId: string): Promise<PlatformBrowserMutation> {
  return invoke("reclaim_browser_session", { sessionId });
}

export function attachComputerSession(sessionId: string): Promise<PlatformComputerMutation> {
  return invoke("attach_computer_session", { sessionId });
}

export function captureComputerSession(sessionId: string): Promise<ComputerUseFrame> {
  return invoke("capture_computer_session", { sessionId });
}

export function applyComputerActions(sessionId: string, request: ApplyComputerActionsRequest): Promise<PlatformComputerActionMutation> {
  return invoke("apply_computer_actions", { sessionId, request });
}

export function handoffComputerSession(sessionId: string): Promise<PlatformComputerMutation> {
  return invoke("handoff_computer_session", { sessionId });
}

export function reclaimComputerSession(sessionId: string): Promise<PlatformComputerMutation> {
  return invoke("reclaim_computer_session", { sessionId });
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

function launchRequest(preferences?: ChatRunPreferences, organizationId?: string) {
  const usesCodexLocalProvider = Boolean(preferences?.model_provider);
  return {
    organization_id: organizationId || null,
    context_token_budget: 8192,
    requested_tools: [],
    estimated_output_tokens: 8192,
    max_model_calls: 16,
    provider_id: usesCodexLocalProvider ? null : preferences?.provider_id || null,
    model_id: usesCodexLocalProvider ? null : preferences?.model_id || null,
    reasoning_effort: preferences?.reasoning_effort ?? "medium",
    service_tier: preferences?.speed === "fast" ? "priority" : null
  };
}

export function launchTask(taskId: string, organizationId?: string, preferences?: ChatRunPreferences): Promise<LaunchTaskResponse> {
  return invoke("launch_task", {
    taskId,
    request: launchRequest(preferences, organizationId)
  });
}

export function retryTask(taskId: string, preferences?: ChatRunPreferences): Promise<LaunchTaskResponse> {
  return invoke("retry_task", {
    taskId,
    request: launchRequest(preferences)
  });
}

export function resumeTask(taskId: string, preferences?: ChatRunPreferences): Promise<LaunchTaskResponse> {
  return invoke("resume_task", {
    taskId,
    request: launchRequest(preferences)
  });
}

export function listApprovals(): Promise<ApprovalListResponse> {
  return invoke("list_approvals");
}

export function approveRequest(approvalId: string): Promise<ApprovalRequest> {
  return invoke("resolve_approval", {
    approvalId,
    request: { state: "approved", reason: "Approved in MetaCog desktop" }
  });
}

export function denyRequest(approvalId: string): Promise<ApprovalRequest> {
  return invoke("resolve_approval", {
    approvalId,
    request: { state: "denied", reason: "Denied in MetaCog desktop" }
  });
}

export function loadTaskEvents(taskId: string): Promise<TaskEventsResponse> {
  return invoke("task_events", { taskId });
}

export function loadTaskAudit(taskId: string): Promise<ToolAuditResponse> {
  return invoke("task_tool_audit", { taskId });
}

export function replayAgentRun(runId: string): Promise<RunReplayReport> {
  return invoke("replay_agent_run", { runId });
}

export function pickProjectDirectory(): Promise<string | null> {
  return invoke("pick_project_directory");
}

export function discoverProject(root: string, projectId?: string): Promise<ProjectMutation> {
  return invoke("discover_project", { request: { root, project_id: projectId ?? null, initialize_git: true } });
}
export function initializeProjectGit(projectId: string): Promise<ProjectMutation> {
  return invoke("initialize_project_git", { projectId });
}

export function githubStatus(): Promise<GitHubConnectionStatus> { return invoke("github_status"); }
export function startGitHubLogin(): Promise<GitHubLoginStart> { return invoke("start_github_login"); }
export function completeGitHubLogin(sessionId: string): Promise<GitHubConnectionStatus> { return invoke("complete_github_login", { sessionId }); }
export function disconnectGitHub(): Promise<GitHubConnectionStatus> { return invoke("disconnect_github"); }
export function listGitHubRepositories(): Promise<GitHubRepositoryList> { return invoke("list_github_repositories"); }
export function createGitHubRepository(request: { name: string; description?: string; private: boolean }): Promise<GitHubRepository> {
  return invoke("create_github_repository", { request });
}
export function linkGitHubRepository(projectId: string, cloneUrl: string): Promise<void> {
  return invoke("link_github_repository", { request: { project_id: projectId, clone_url: cloneUrl } });
}

export function listProjects(): Promise<ProjectListResponse> {
  return invoke("list_projects");
}

export function archiveProject(projectId: string, archived: boolean): Promise<ProjectModel> {
  return invoke("archive_project", { projectId, archived });
}

export function removeProject(projectId: string): Promise<void> {
  return invoke("remove_project", { projectId });
}

export function projectTopologyPlan(projectId: string): Promise<ProjectTopologyPlan> {
  return invoke("project_topology_plan", { projectId });
}

export function listReviewedTopologyPlans(projectId: string): Promise<ReviewedTopologyPlanListResponse> {
  return invoke("list_reviewed_topology_plans", { projectId });
}

export function createReviewedTopologyPlan(
  projectId: string,
  request: ReviewedTopologyPlanCreateRequest
): Promise<ReviewedTopologyPlanMutation> {
  return invoke("create_reviewed_topology_plan", { projectId, request });
}

export function getReviewedTopologyPlan(projectId: string, planId: string): Promise<ReviewedTopologyPlan> {
  return invoke("get_reviewed_topology_plan", { projectId, planId });
}

export function runReviewedTopologyPlan(projectId: string, planId: string): Promise<ReviewedTopologyPlanMutation> {
  return invoke("run_reviewed_topology_plan", { projectId, planId });
}

export function cancelReviewedTopologyPlan(projectId: string, planId: string): Promise<ReviewedTopologyPlanMutation> {
  return invoke("cancel_reviewed_topology_plan", { projectId, planId });
}

export function listProjectMemory(projectId: string): Promise<MemoryClaimListResponse> {
  return invoke("list_project_memory", { projectId });
}

export function createProjectMemory(
  projectId: string,
  request: {
    namespace: string;
    key: string;
    value: unknown;
    confidence: number;
    provenance: ClaimProvenance;
    fresh_until?: string;
  }
): Promise<MemoryClaimMutation> {
  return invoke("create_project_memory", { projectId, request });
}

export function correctProjectMemory(
  projectId: string,
  claimId: string,
  request: { value: unknown; confidence: number; provenance: ClaimProvenance; fresh_until?: string }
): Promise<MemoryClaimMutation> {
  return invoke("correct_project_memory", { projectId, claimId, request });
}

export function transitionProjectMemory(
  projectId: string,
  claimId: string,
  action: MemoryLifecycleAction
): Promise<MemoryClaimMutation> {
  return invoke("transition_project_memory", {
    projectId,
    claimId,
    request: {
      action,
      provenance: { source_kind: "user", source: "Desktop owner" }
    }
  });
}

export function reviewProjectMemoryTransfer(
  projectId: string,
  claimId: string
): Promise<ProjectMemoryTransferPreview> {
  return invoke("review_project_memory_transfer", { projectId, claimId });
}

export function transferProjectMemory(
  projectId: string,
  request: {
    transfer_id: string;
    source_claim_id: string;
    source_claim_sha256: string;
    target_project_id: string;
    reason: string;
  }
): Promise<ProjectMemoryTransferMutation> {
  return invoke("transfer_project_memory", { projectId, request });
}

export function listProjectMemoryTransfers(
  projectId: string
): Promise<ProjectMemoryTransferListResponse> {
  return invoke("list_project_memory_transfers", { projectId });
}

export function loadProjectMemoryHistory(
  projectId: string,
  namespace: string,
  key: string
): Promise<MemoryClaimHistoryResponse> {
  return invoke("project_memory_history", { projectId, namespace, key });
}

export function loadManagedProjectMemory(projectId: string): Promise<ManagedMemoryResponse> {
  return invoke("managed_project_memory", { projectId });
}

export function listOpenInApplications(): Promise<OpenInApplication[]> {
  return invoke("list_open_in_applications");
}

export function openWorkspaceIn(application: string, workingDirectory: string): Promise<void> {
  return invoke("open_workspace_in", { application, workingDirectory });
}

export function runTerminalCommand(workingDirectory: string, command: string): Promise<TerminalCommandResult> {
  return invoke("run_terminal_command", { request: { working_directory: workingDirectory, command } });
}

export function listSupervisionObservations(): Promise<SupervisionObservationListResponse> {
  return invoke("list_supervision_observations");
}

export function listOrganizations(): Promise<OrganizationListResponse> {
  return invoke("list_organizations");
}

export function getGovernorOverview(): Promise<GovernorOverview> {
  return invoke("governor_overview");
}

export function getGovernorControlPolicy(): Promise<GovernorControlPolicy> {
  return invoke("governor_control_policy");
}

export function listGovernorProposals(organizationId: string): Promise<GovernorProposalListResponse> {
  return invoke("list_governor_proposals", { organizationId });
}

export function getGovernorProposal(proposalId: string): Promise<GovernorProposal> {
  return invoke("get_governor_proposal", { proposalId });
}

export function decideGovernorProposal(proposalId: string, approve: boolean): Promise<GovernorProposalMutation> {
  return invoke("decide_governor_proposal", { proposalId, approve });
}

export function requestGovernorImplementation(proposalId: string, conversationId: string): Promise<Record<string, unknown>> {
  return invoke("request_governor_implementation", { proposalId, conversationId });
}

export function getOrganizationAssignments(): Promise<OrganizationAssignmentResponse> {
  return invoke("organization_assignments");
}

export function setGlobalOrganizationAssignment(organizationId: string): Promise<OrganizationAssignmentResponse> {
  return invoke("set_global_organization_assignment", { request: { organization_id: organizationId } });
}

export function setProjectOrganizationAssignment(projectId: string, organizationId?: string): Promise<OrganizationAssignmentResponse> {
  return invoke("set_project_organization_assignment", { projectId, request: { organization_id: organizationId || null } });
}

export function previewOrganizationRevision(organizationId: string, organization: OrganizationGenome): Promise<OrganizationRevisionPreview> {
  return invoke("preview_organization_revision", {
    organizationId,
    request: {
      expected_parent_revision_id: organization.revision_id,
      name: organization.name,
      roles: organization.roles.map((role) => ({ ...role, id: role.id || null })),
      relationships: organization.relationships,
      policy: organization.policy,
      evaluation: organization.evaluation
    }
  });
}

export function publishOrganizationRevision(preview: OrganizationRevisionPreview, changeSummary: string): Promise<OrganizationMutationResponse> {
  const organization = preview.organization;
  return invoke("publish_organization_revision", {
    organizationId: organization.organization_id,
    request: {
      expected_parent_revision_id: organization.parent_revision_id,
      name: organization.name,
      roles: organization.roles,
      relationships: organization.relationships,
      policy: organization.policy,
      evaluation: organization.evaluation,
      change_summary: changeSummary.trim()
    }
  });
}

export function listAgentRuns(): Promise<AgentRunListResponse> {
  return invoke("list_agent_runs");
}

export function getAgentRun(runId: string): Promise<AgentRunView> {
  return invoke("get_agent_run", { runId });
}

export function cancelAgentRun(runId: string): Promise<AgentRunView> {
  return invoke("cancel_agent_run", { runId });
}

export function loadOrganizationHistory(organizationId: string): Promise<OrganizationHistoryResponse> {
  return invoke("organization_history", { organizationId });
}

export function createOrganization(name?: string): Promise<OrganizationMutationResponse> {
  return invoke("create_organization", { request: { name: name?.trim() || null } });
}

export function inheritOrganization(organizationId: string, name: string): Promise<OrganizationMutationResponse> {
  return invoke("inherit_organization", {
    organizationId,
    request: { name: name.trim() }
  });
}

export function exportOrganization(revisionId: string): Promise<PortableOrganizationGenome> {
  return invoke("export_organization", { revisionId });
}

export function importOrganization(
  portable: PortableOrganizationGenome,
  importId: string,
  name?: string
): Promise<PortableOrganizationImportMutation> {
  return invoke("import_organization", {
    request: { import_id: importId, portable, name: name?.trim() || null }
  });
}

export function compareOrganizations(
  leftRevisionId: string,
  rightRevisionId: string
): Promise<PortableOrganizationComparison> {
  return invoke("compare_organizations", { leftRevisionId, rightRevisionId });
}

export function listOrganizationPublishers(): Promise<TrustedOrganizationPublisher[]> {
  return invoke("list_organization_publishers");
}

export function trustOrganizationPublisher(request: {
  id: string;
  display_name: string;
  public_key_base64: string;
  registries: string[];
  expected_revision?: number;
  reason: string;
}): Promise<OrganizationPublisherMutation> {
  return invoke("trust_organization_publisher", { request });
}

export function revokeOrganizationPublisher(
  publisherId: string,
  expectedRevision: number,
  reason: string
): Promise<OrganizationPublisherMutation> {
  return invoke("revoke_organization_publisher", {
    publisherId,
    request: { expected_revision: expectedRevision, reason: reason.trim() }
  });
}

export function listOrganizationShareReviews(): Promise<OrganizationShareReview[]> {
  return invoke("list_organization_share_reviews");
}

export function reviewSharedOrganization(
  reviewId: string,
  organizationPackage: OrganizationSharePackage
): Promise<OrganizationShareReviewMutation> {
  return invoke("review_shared_organization", {
    request: { review_id: reviewId, package: organizationPackage }
  });
}

export function importSharedOrganization(
  reviewId: string,
  importId: string,
  reason: string,
  name?: string
): Promise<OrganizationShareImportMutation> {
  return invoke("import_shared_organization", {
    reviewId,
    request: { import_id: importId, name: name?.trim() || null, reason: reason.trim() }
  });
}

export function rejectSharedOrganization(
  reviewId: string,
  reason: string
): Promise<OrganizationShareReviewMutation> {
  return invoke("reject_shared_organization", {
    reviewId,
    request: { reason: reason.trim() }
  });
}

export function recordSupervisionObservation(observation: SupervisionObservation): Promise<unknown> {
  return invoke("record_supervision_observation", { observation });
}

export function recordCandidateRevision(candidate: Record<string, unknown>): Promise<unknown> {
  return invoke("record_candidate_revision", { candidate });
}

export function listExperiments(organizationId: string): Promise<ExperimentListResponse> {
  return invoke("list_experiments", { organizationId });
}

export function getExperiment(experimentId: string): Promise<ExperimentDefinition> {
  return invoke("get_experiment", { experimentId });
}

export function createExperiment(experiment: ExperimentDefinition): Promise<unknown> {
  return invoke("create_experiment", { experiment });
}

export function transitionExperiment(experimentId: string, expected: ExperimentState, next: ExperimentState): Promise<unknown> {
  return invoke("transition_experiment", { experimentId, request: { expected, next } });
}

export async function loadExperimentEvidence(experimentId: string): Promise<{
  shadow: ShadowExecutionListResponse;
  benchmarks: ExperimentBenchmarkListResponse;
  decisions: ExperimentDecisionListResponse;
}> {
  const [shadow, benchmarks, decisions] = await Promise.all([
    invoke<ShadowExecutionListResponse>("list_shadow_executions", { experimentId }),
    invoke<ExperimentBenchmarkListResponse>("list_experiment_benchmarks", { experimentId }),
    invoke<ExperimentDecisionListResponse>("list_experiment_decisions", { experimentId })
  ]);
  return { shadow, benchmarks, decisions };
}

export function recordShadowExecution(experimentId: string, receipt: Record<string, unknown>): Promise<unknown> {
  return invoke("record_shadow_execution", { experimentId, receipt });
}

export function recordExperimentBenchmark(experimentId: string, benchmark: Record<string, unknown>): Promise<unknown> {
  return invoke("record_experiment_benchmark", { experimentId, benchmark });
}

export function decideExperiment(experimentId: string, decision: Record<string, unknown>): Promise<unknown> {
  return invoke("decide_experiment", { experimentId, decision });
}
