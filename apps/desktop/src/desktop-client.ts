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

export interface ProviderListItem {
  connection: ProviderConnection;
  catalog?: ProviderModelCatalog;
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
    tool_capabilities: string[];
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
export interface OrganizationMutationResponse {
  organization: OrganizationGenome;
  event: { sequence: number; kind: string; occurred_at: string; payload?: Record<string, unknown> };
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
  run: { run: { id: string; state: string } };
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

export function listSchedules(): Promise<ScheduleListResponse> {
  return invoke("list_schedules");
}

export function listSites(): Promise<SiteListResponse> { return invoke("list_sites"); }
export function registerSite(request: CreateSiteRequest): Promise<Site> { return invoke("register_site", { request }); }
export function updateSite(siteId: string, request: UpdateSiteRequest): Promise<SiteMutation> { return invoke("update_site", { siteId, request }); }
export function loadSiteRevisions(siteId: string): Promise<SiteRevisionListResponse> { return invoke("site_revisions", { siteId }); }
export function loadSiteHealth(siteId: string): Promise<SiteHealthListResponse> { return invoke("site_health_history", { siteId }); }
export function checkSiteHealth(siteId: string, checkId: string): Promise<SiteHealthMutation> { return invoke("check_site_health", { siteId, request: { check_id: checkId } }); }
export function getSiteMonitor(siteId: string): Promise<SiteMonitor> { return invoke("get_site_monitor", { siteId }); }
export function createSiteMonitor(siteId: string, request: CreateSiteMonitorRequest): Promise<SiteMonitorMutation> { return invoke("create_site_monitor", { siteId, request }); }
export function loadSiteDeployments(siteId: string): Promise<SiteDeploymentListResponse> { return invoke("site_deployment_history", { siteId }); }
export function deploySite(siteId: string, deploymentId: string): Promise<SiteDeploymentMutation> { return invoke("deploy_site", { siteId, request: { deployment_id: deploymentId } }); }
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

export function launchTask(taskId: string): Promise<LaunchTaskResponse> {
  return invoke("launch_task", {
    taskId,
    request: {
      context_token_budget: 8192,
      requested_tools: [],
      estimated_output_tokens: 8192,
      max_model_calls: 16
    }
  });
}

export function retryTask(taskId: string): Promise<LaunchTaskResponse> {
  return invoke("retry_task", {
    taskId,
    request: {
      context_token_budget: 8192,
      requested_tools: [],
      estimated_output_tokens: 8192,
      max_model_calls: 16
    }
  });
}

export function resumeTask(taskId: string): Promise<LaunchTaskResponse> {
  return invoke("resume_task", {
    taskId,
    request: {
      context_token_budget: 8192,
      requested_tools: [],
      estimated_output_tokens: 8192,
      max_model_calls: 16
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

export function replayAgentRun(runId: string): Promise<RunReplayReport> {
  return invoke("replay_agent_run", { runId });
}

export function pickProjectDirectory(): Promise<string | null> {
  return invoke("pick_project_directory");
}

export function discoverProject(root: string, projectId?: string): Promise<ProjectMutation> {
  return invoke("discover_project", { request: { root, project_id: projectId ?? null } });
}

export function listProjects(): Promise<ProjectListResponse> {
  return invoke("list_projects");
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

export function listSupervisionObservations(): Promise<SupervisionObservationListResponse> {
  return invoke("list_supervision_observations");
}

export function listOrganizations(): Promise<OrganizationListResponse> {
  return invoke("list_organizations");
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
