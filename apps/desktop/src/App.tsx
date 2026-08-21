import { useEffect, useRef, useState, type MouseEvent } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  approveRequest,
  applyComputerActions,
  attachBrowserSession,
  attachComputerSession,
  cancelOpenAiOAuth,
  closePlatformSession,
  checkSiteHealth,
  createBackup,
  createEncryptedBackup,
  createProjectMemory,
  createReviewedTopologyPlan,
  createOrganization,
  createSiteMonitor,
  createSshWorkflow,
  completeOpenAiOAuth,
  configureMcpConnection,
  configureProvider,
  captureComputerSession,
  createSchedule,
  createExperiment,
  createTask,
  deploySite,
  denyRequest,
  discoverProject,
  discoverMcpConnection,
  discoverProvider,
  disconnectProvider,
  exportOrganization,
  handoffBrowserSession,
  handoffComputerSession,
  inheritOrganization,
  importOrganization,
  importSharedOrganization,
  listMcpConnections,
  listOrganizationPublishers,
  listOrganizationShareReviews,
  listOrganizations,
  listPlugins,
  listExperiments,
  listSupervisionObservations,
  listProviders,
  providerProxyConversations,
  providerProxyUsage,
  replayAgentRun,
  listApprovals,
  listBackups,
  listSchedules,
  listSchedulerNotifications,
  listProjects,
  listProjectMemory,
  listProjectMemoryTransfers,
  listReviewedTopologyPlans,
  projectTopologyPlan,
  listPlatformSessions,
  listSites,
  listSshHosts,
  listSshWorkflows,
  listTasks,
  launchTask,
  loadTaskAudit,
  loadTaskEvents,
  loadExperimentEvidence,
  loadOrganizationHistory,
  loadScheduleHistory,
  loadPlatformCapabilities,
  loadPlatformSessionHistory,
  loadProjectMemoryHistory,
  loadManagedProjectMemory,
  loadSiteDeployments,
  loadSiteHealth,
  loadSshOperations,
  getSiteMonitor,
  getLocalServiceStatus,
  getUpdateConfiguration,
  getUserSettings,
  loadUserSettingsHistory,
  navigateBrowserSession,
  compareOrganizations,
  type McpAuthMode,
  type ExperimentDefinition,
  type ExperimentState,
  type McpListItem,
  type McpProtocolMode,
  type McpTransportKind,
  type PluginCapability,
  type PluginCapabilityGrant,
  type PluginDescriptor,
  type PluginInvocationMutation,
  type PluginManifest,
  type InstallRegistryPluginRequest,
  openExternalUrl,
  type OAuthLoginStart,
  type OrganizationGenome,
  type OrganizationSharePackage,
  type OrganizationShareReview,
  type PortableOrganizationComparison,
  type PortableOrganizationGenome,
  type TrustedOrganizationPublisher,
  type OpenAiOAuthMethod,
  type ProviderAuthMode,
  type ProviderKind,
  type ProviderListItem,
  type ProviderModel,
  type ProxyConversation,
  type ProxyUsageSummary,
  type PlatformCapabilitySnapshot,
  type PlatformSession,
  type PlatformSessionHistoryResponse,
  type PlatformSessionKind,
  type ComputerUseFrame,
  type PersistentBrowserObservation,
  pickProjectDirectory,
  type ProjectModel,
  type ProjectTopologyPlan,
  type ReviewedTopologyPlan,
  type MemoryClaim,
  type MemoryClaimHistoryResponse,
  type MemoryLifecycleAction,
  type ProjectMemoryTransfer,
  type ProjectMemoryTransferPreview,
  type RiskLevel,
  type RunReplayReport,
  type ScheduleDefinition,
  type ScheduleHistoryResponse,
  type ScheduleTrigger,
  type SchedulerNotification,
  type SupervisionObservation,
  type Site,
  type SiteDeploymentListResponse,
  type SiteHealthListResponse,
  type SiteMonitor,
  type SshHost,
  type SshOperationKind,
  type SshOperationListResponse,
  type SshWorkflow,
  type SshWorkflowStepRequest,
  retryTask,
  resumeTask,
  requestPlatformSession,
  reclaimBrowserSession,
  reclaimComputerSession,
  registerSite,
  rejectSharedOrganization,
  recordCandidateRevision,
  recordExperimentBenchmark,
  recordShadowExecution,
  recordSupervisionObservation,
  registerSshHost,
  runSshOperation,
  runSshWorkflow,
  runReviewedTopologyPlan,
  cancelSshWorkflow,
  cancelReviewedTopologyPlan,
  runDiagnostics,
  checkForUpdate,
  installUpdate,
  installLocalPlugin,
  installRegistryPlugin,
  invokePlugin,
  getPluginInvocation,
  setPluginEnabled,
  setPluginGrants,
  startOpenAiOAuth,
  reviewSharedOrganization,
  revokeOrganizationPublisher,
  setScheduleState,
  type TaskEnvelope,
  type ApprovalRequest,
  type BackupSummary,
  type DiagnosticReport,
  type LocalServiceStatus,
  type UpdateCheck,
  type UpdateConfiguration,
  type TaskEventsResponse,
  type ToolAuditResponse,
  type UserSettings,
  type UserSettingsRevision,
  transitionTask,
  transitionProjectMemory,
  reviewProjectMemoryTransfer,
  transferProjectMemory,
  transitionExperiment,
  trustOrganizationPublisher,
  correctProjectMemory,
  updateSite,
  updateUserSettings,
  verifyBackup,
  acknowledgeSchedulerNotification,
  decideExperiment,
  wakeSchedule
} from "./desktop-client";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function projectRootKey(root: string): string {
  const trimmed = root.trim();
  const windowsPath = /^\\\\\?\\|^[A-Za-z]:[\\/]|^\\\\/.test(trimmed);
  if (!windowsPath) return trimmed.replace(/\/+$/, "");
  return trimmed
    .replace(/^\\\\\?\\UNC\\/i, "\\\\")
    .replace(/^\\\\\?\\/, "")
    .replace(/\//g, "\\")
    .replace(/\\+$/, "")
    .toLocaleLowerCase("en-US");
}

function reviewedTopologyErrorMessage(error: unknown): string {
  const message = errorMessage(error);
  return /409 Conflict|code conflict/i.test(message)
    ? `${message} Refresh the project folder to revalidate its files, then create a new review.`
    : message;
}

function positiveWholeNumber(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive whole number.`);
  }
  return parsed;
}

function memoryValue(value: string): unknown {
  const trimmed = value.trim();
  if (!trimmed) throw new Error("Enter a memory value.");
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    return trimmed;
  }
}

function memoryText(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}

function worldNodeLabel(project: ProjectModel, id: string): string {
  return project.world?.nodes.find((node) => node.id === id)?.label ?? id;
}

const desktopMemoryProvenance = {
  source_kind: "user" as const,
  source: "Desktop owner"
};

const SCHEDULE_EVENT_KINDS = [
  "task_created", "task_started", "task_state_changed", "task_completed", "task_failed",
  "approval_requested", "approval_resolved", "provider_call_started", "provider_call_completed",
  "provider_connection_recorded", "provider_connection_removed", "provider_catalog_discovered", "provider_credentials_refreshed",
  "mcp_connection_recorded", "mcp_catalog_discovered", "tool_requested", "tool_completed",
  "project_discovered", "memory_claim_recorded", "memory_claim_corrected", "memory_claim_state_changed",
  "organization_revision_recorded", "agent_run_planned", "agent_run_state_changed",
  "agent_instance_recorded", "agent_instance_state_changed", "agent_lease_recorded",
  "agent_lease_state_changed", "agent_usage_recorded", "evaluation_checkpointed", "evaluation_recorded",
  "agent_run_metrics_updated", "agent_turn_completed", "agent_turn_waiting", "agent_turn_failed",
  "mcg_intervention"
] as const;

function freshScheduleId(): string {
  return crypto.randomUUID();
}

function freshPlatformSessionId(): string {
  return crypto.randomUUID();
}

type CodeReviewScope = "branch" | "uncommitted" | "commit" | "custom";
type ScheduleFilter = "all" | "enabled" | "paused" | "completed";
type PluginDirectoryTab = "openai" | "workspace" | "personal" | "installed";
type SettingsTab = "general" | "import" | "profile" | "appearance" | "voice" | "configuration" | "personalization" | "shortcuts" | "usage" | "account" | "plugins" | "browser" | "computer" | "hooks" | "connections" | "git" | "environments" | "worktrees" | "archived";

const SCHEDULE_FILTERS: Array<[ScheduleFilter, string]> = [
  ["all", "All"],
  ["enabled", "Active"],
  ["paused", "Paused"],
  ["completed", "Completed"]
];

const PLUGIN_DIRECTORY_TABS: Array<[PluginDirectoryTab, string]> = [
  ["openai", "OpenAI"],
  ["workspace", "Workspace"],
  ["personal", "Personal"],
  ["installed", "Installed"]
];

const CODE_REVIEW_GOAL_PREFIX = "Review code changes";

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function modelPrice(model: ProviderModel): string {
  if (!model.price || (model.price.input_per_million === undefined && model.price.output_per_million === undefined)) {
    return "Cost unknown";
  }
  const input = model.price.input_per_million === undefined ? "?" : `$${model.price.input_per_million.toFixed(2)}`;
  const output = model.price.output_per_million === undefined ? "?" : `$${model.price.output_per_million.toFixed(2)}`;
  return `${input} in · ${output} out / 1M`;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes.slice().buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function capabilityLabel(capability: PluginCapability): string {
  return capability.replaceAll("_", " ");
}

function TaskEvidence({
  history,
  audit,
  advanced,
  state,
  replay,
  replayBusy,
  onReplay
}: {
  history: TaskEventsResponse;
  audit: ToolAuditResponse;
  advanced: boolean;
  state: TaskEnvelope["state"];
  replay?: RunReplayReport;
  replayBusy: boolean;
  onReplay?: () => void;
}) {
  const evaluation = history.events
    .slice()
    .reverse()
    .find((event) => event.kind === "evaluation_recorded" && event.payload?.outcome);
  const conditions = evaluation?.payload?.conditions ?? [];
  const completions = audit.records.filter((record) => record.phase === "completion");
  const artifacts = Array.from(
    new Map(
      completions.flatMap((record) => record.artifacts).map((artifact) => [artifact.sha256, artifact])
    ).values()
  );
  const passingChecks = conditions.filter((condition) => condition.passed).length;
  const outcome = evaluation?.payload?.outcome ?? ({
    pending: "Pending",
    active: "In progress",
    waiting_for_approval: "Waiting for approval",
    blocked: "Blocked",
    completed: "Completed",
    failed: "Failed",
    cancelled: "Cancelled"
  } satisfies Record<TaskEnvelope["state"], string>)[state];

  return (
    <section className="task-evidence" aria-label="Task evidence">
      <div className="evidence-overview">
        <div><span>Outcome</span><strong>{outcome}</strong></div>
        <div><span>Checks</span><strong>{passingChecks} / {conditions.length}</strong></div>
        <div><span>Tool runs</span><strong>{completions.length}</strong></div>
        <div><span>Artifacts</span><strong>{artifacts.length}</strong></div>
      </div>
      {onReplay && <div className="task-actions">
        <button className="secondary" type="button" disabled={replayBusy} onClick={onReplay}>
          {replayBusy ? "Verifying replay..." : "Verify replay"}
        </button>
        {replay && <span role="status">Verified · {replay.invariants.filter((item) => item.passed).length}/{replay.invariants.length} invariants</span>}
      </div>}
      {replay && <div className="replay-verification" aria-label="Deterministic replay verification">
        <strong>Durable replay verified</strong>
        <span>{replay.side_effect_free ? "No provider or tool side effects" : "Side effects reported"}</span>
        <code title={replay.evidence_sha256}>SHA-256 {replay.evidence_sha256}</code>
        {advanced && <small>{replay.counts.events} events · {replay.counts.tool_audit_records} tool records · {replay.counts.evaluations} evaluations</small>}
      </div>}
      {conditions.length > 0 && <ul className="condition-results">
        {conditions.map((condition, index) => (
          <li className={condition.passed ? "condition-pass" : "condition-fail"} key={index}>
            <strong>{condition.passed ? "Passed" : "Failed"}</strong>
            <span>{condition.detail}</span>
          </li>
        ))}
      </ul>}
      {advanced && <div className="evidence-details">
        <div>
          <h4>Event timeline</h4>
          <ol className="event-history">
            {history.events.map((event) => (
              <li key={event.sequence}>
                <span>{event.sequence}</span>{event.kind.replaceAll("_", " ")}
              </li>
            ))}
          </ol>
        </div>
        <div>
          <h4>Governed tool activity</h4>
          {completions.length === 0 ? <p className="empty-inline">No completed tool runs.</p> : <ul className="tool-evidence-list">
            {completions.map((record) => (
              <li key={`${record.execution_id}-${record.recorded_at}`}>
                <div><strong>{record.tool}</strong><span className={`tool-result result-${record.result ?? "unknown"}`}>{record.result ?? "unknown"}</span></div>
                <code>{record.resource}</code>
                {record.verification.map((item, index) => <small key={`${record.execution_id}-verification-${index}`}>{item.passed ? "Pass" : "Fail"} · {item.method}: {item.detail}</small>)}
              </li>
            ))}
          </ul>}
        </div>
        {artifacts.length > 0 && <div className="artifact-evidence">
          <h4>Immutable artifacts</h4>
          <ul>
            {artifacts.map((artifact) => <li key={artifact.sha256}>
              <span>{artifact.media_type} · {formatBytes(artifact.size_bytes)}</span>
              <code title={artifact.sha256}>{artifact.sha256}</code>
            </li>)}
          </ul>
        </div>}
      </div>}
    </section>
  );
}

export function App() {
  const modeButtonRef = useRef<HTMLButtonElement>(null);
  const goalInputRef = useRef<HTMLTextAreaElement>(null);
  const chatSearchRef = useRef<HTMLInputElement>(null);
  const [activeSurface, setActiveSurface] = useState("tasks");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [chatSearch, setChatSearch] = useState("");
  const [selectedChatId, setSelectedChatId] = useState<string | null>(null);
  const [ownerMenuOpen, setOwnerMenuOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("general");
  const [settingsSearch, setSettingsSearch] = useState("");
  const [desktopMenu, setDesktopMenu] = useState<string | null>(null);
  const [pinnedSummaryOpen, setPinnedSummaryOpen] = useState(true);
  const [projectTreeOpen, setProjectTreeOpen] = useState(true);
  const [zoomLevel, setZoomLevel] = useState(1);
  const [pinnedTaskIds, setPinnedTaskIds] = useState<string[]>(() => {
    try {
      const stored = window.localStorage.getItem("mcg.pinned-task-ids");
      return stored ? JSON.parse(stored) as string[] : [];
    } catch {
      return [];
    }
  });
  const [globalFolders, setGlobalFolders] = useState<string[]>(() => {
    try {
      const stored = window.localStorage.getItem("mcg.global-folders");
      const parsed = stored ? JSON.parse(stored) as unknown : [];
      return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === "string" && value.trim().length > 0) : [];
    } catch {
      return [];
    }
  });
  const [globalFolderBusy, setGlobalFolderBusy] = useState(false);
  const [globalFolderError, setGlobalFolderError] = useState<string | null>(null);
  const [localService, setLocalService] = useState<LocalServiceStatus | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [userSettings, setUserSettings] = useState<UserSettings | null>(null);
  const [settingsHistory, setSettingsHistory] = useState<UserSettingsRevision[]>([]);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [backups, setBackups] = useState<BackupSummary[]>([]);
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupError, setBackupError] = useState<string | null>(null);
  const [backupPassphrase, setBackupPassphrase] = useState("");
  const [backupConfirmation, setBackupConfirmation] = useState("");
  const [diagnostics, setDiagnostics] = useState<DiagnosticReport | null>(null);
  const [diagnosticsBusy, setDiagnosticsBusy] = useState(false);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [updateConfiguration, setUpdateConfiguration] = useState<UpdateConfiguration | null>(null);
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [updateReview, setUpdateReview] = useState(false);
  const [login, setLogin] = useState<OAuthLoginStart | null>(null);
  const [providerStatus, setProviderStatus] = useState("Not connected");
  const [busy, setBusy] = useState(false);
  const [providerError, setProviderError] = useState<string | null>(null);
  const [providers, setProviders] = useState<ProviderListItem[]>([]);
  const [proxyUsage, setProxyUsage] = useState<ProxyUsageSummary | null>(null);
  const [proxyConversations, setProxyConversations] = useState<ProxyConversation[]>([]);
  const [providerSetup, setProviderSetup] = useState(false);
  const [providerName, setProviderName] = useState("");
  const [providerId, setProviderId] = useState("");
  const [providerKind, setProviderKind] = useState<ProviderKind>("open_ai_compatible");
  const [providerUrl, setProviderUrl] = useState("");
  const [providerAuth, setProviderAuth] = useState<ProviderAuthMode>("api_key");
  const [providerCredential, setProviderCredential] = useState("");
  const [providerOrganization, setProviderOrganization] = useState("");
  const [providerProject, setProviderProject] = useState("");
  const [mcpConnections, setMcpConnections] = useState<McpListItem[]>([]);
  const [mcpName, setMcpName] = useState("");
  const [mcpId, setMcpId] = useState("");
  const [mcpUrl, setMcpUrl] = useState("");
  const [mcpTransport, setMcpTransport] = useState<McpTransportKind>("streamable_http");
  const [mcpCommand, setMcpCommand] = useState("");
  const [mcpArgs, setMcpArgs] = useState("");
  const [mcpAuth, setMcpAuth] = useState<McpAuthMode>("none");
  const [mcpProtocol, setMcpProtocol] = useState<McpProtocolMode>("auto");
  const [mcpCredential, setMcpCredential] = useState("");
  const [mcpBusy, setMcpBusy] = useState(false);
  const [mcpError, setMcpError] = useState<string | null>(null);
  const [plugins, setPlugins] = useState<PluginDescriptor[]>([]);
  const [pluginManifestFile, setPluginManifestFile] = useState<File | null>(null);
  const [pluginModuleFile, setPluginModuleFile] = useState<File | null>(null);
  const [pluginRegistryPackageFile, setPluginRegistryPackageFile] = useState<File | null>(null);
  const [pluginScopeDrafts, setPluginScopeDrafts] = useState<Record<string, Record<string, string>>>({});
  const [pluginInputDrafts, setPluginInputDrafts] = useState<Record<string, string>>({});
  const [pluginInvocations, setPluginInvocations] = useState<Record<string, PluginInvocationMutation | undefined>>({});
  const [pluginHistoryCounts, setPluginHistoryCounts] = useState<Record<string, number>>({});
  const [pluginSearch, setPluginSearch] = useState("");
  const [pluginDirectoryTab, setPluginDirectoryTab] = useState<PluginDirectoryTab>("installed");
  const [pluginAccessReviewId, setPluginAccessReviewId] = useState<string | null>(null);
  const [pluginBusy, setPluginBusy] = useState(false);
  const [pluginError, setPluginError] = useState<string | null>(null);
  const [tasks, setTasks] = useState<TaskEnvelope[]>([]);
  const [taskGoal, setTaskGoal] = useState("");
  const [taskConstraints, setTaskConstraints] = useState("");
  const [taskSuccess, setTaskSuccess] = useState("");
  const [taskRisk, setTaskRisk] = useState<RiskLevel>("medium");
  const [taskMaxCost, setTaskMaxCost] = useState("");
  const [taskBusy, setTaskBusy] = useState(false);
  const [taskError, setTaskError] = useState<string | null>(null);
  const [codeReviewScope, setCodeReviewScope] = useState<CodeReviewScope>("branch");
  const [codeReviewBase, setCodeReviewBase] = useState("main");
  const [codeReviewInstructions, setCodeReviewInstructions] = useState("");
  const [codeReviewError, setCodeReviewError] = useState<string | null>(null);
  const [taskLaunch, setTaskLaunch] = useState<Record<string, string>>({});
  const [taskHistory, setTaskHistory] = useState<Record<string, TaskEventsResponse | undefined>>({});
  const [taskAudit, setTaskAudit] = useState<Record<string, ToolAuditResponse | undefined>>({});
  const [taskRunIds, setTaskRunIds] = useState<Record<string, string | undefined>>({});
  const [taskReplays, setTaskReplays] = useState<Record<string, RunReplayReport | undefined>>({});
  const [replayBusyTask, setReplayBusyTask] = useState<string | null>(null);
  const [selectedProject, setSelectedProject] = useState<ProjectModel | null>(null);
  const [projects, setProjects] = useState<ProjectModel[]>([]);
  const [topologyPlan, setTopologyPlan] = useState<ProjectTopologyPlan | null>(null);
  const [topologyPlanError, setTopologyPlanError] = useState<string | null>(null);
  const [reviewedTopologyPlans, setReviewedTopologyPlans] = useState<ReviewedTopologyPlan[]>([]);
  const [reviewedTopologyPlanId, setReviewedTopologyPlanId] = useState(() => crypto.randomUUID());
  const [reviewedTopologyStepId, setReviewedTopologyStepId] = useState(() => crypto.randomUUID());
  const [reviewedTopologyCommand, setReviewedTopologyCommand] = useState("0");
  const [reviewedTopologyStage, setReviewedTopologyStage] = useState("0");
  const [reviewedTopologyReason, setReviewedTopologyReason] = useState("");
  const [reviewedTopologyApprovals, setReviewedTopologyApprovals] = useState<Record<string, string | undefined>>({});
  const [reviewedTopologyBusy, setReviewedTopologyBusy] = useState(false);
  const [reviewedTopologyError, setReviewedTopologyError] = useState<string | null>(null);
  const [memoryClaims, setMemoryClaims] = useState<MemoryClaim[]>([]);
  const [memoryNamespace, setMemoryNamespace] = useState("project");
  const [memoryKey, setMemoryKey] = useState("");
  const [memoryDraft, setMemoryDraft] = useState("");
  const [memoryConfidence, setMemoryConfidence] = useState("0.8");
  const [memoryFreshUntil, setMemoryFreshUntil] = useState("");
  const [memoryCorrections, setMemoryCorrections] = useState<Record<string, string>>({});
  const [memoryHistory, setMemoryHistory] = useState<Record<string, MemoryClaimHistoryResponse | undefined>>({});
  const [memoryTransfers, setMemoryTransfers] = useState<ProjectMemoryTransfer[]>([]);
  const [memoryTransferSource, setMemoryTransferSource] = useState<MemoryClaim | null>(null);
  const [memoryTransferPreview, setMemoryTransferPreview] = useState<ProjectMemoryTransferPreview | null>(null);
  const [memoryTransferTarget, setMemoryTransferTarget] = useState("");
  const [memoryTransferReason, setMemoryTransferReason] = useState("");
  const [memoryTransferId, setMemoryTransferId] = useState(() => crypto.randomUUID());
  const [managedMemory, setManagedMemory] = useState("");
  const [memoryBusy, setMemoryBusy] = useState(false);
  const [memoryError, setMemoryError] = useState<string | null>(null);
  const [organizations, setOrganizations] = useState<OrganizationGenome[]>([]);
  const [organizationName, setOrganizationName] = useState("");
  const [organizationBaseId, setOrganizationBaseId] = useState("");
  const [organizationBusy, setOrganizationBusy] = useState(false);
  const [organizationError, setOrganizationError] = useState<string | null>(null);
  const [portableOrganizationJson, setPortableOrganizationJson] = useState("");
  const [organizationImportId, setOrganizationImportId] = useState(() => crypto.randomUUID());
  const [organizationDuplicateIds, setOrganizationDuplicateIds] = useState<Record<string, string>>({});
  const [organizationHistory, setOrganizationHistory] = useState<Record<string, OrganizationGenome[] | undefined>>({});
  const [organizationCompareLeft, setOrganizationCompareLeft] = useState("");
  const [organizationCompareRight, setOrganizationCompareRight] = useState("");
  const [organizationComparison, setOrganizationComparison] = useState<PortableOrganizationComparison | null>(null);
  const [organizationPublishers, setOrganizationPublishers] = useState<TrustedOrganizationPublisher[]>([]);
  const [organizationShareReviews, setOrganizationShareReviews] = useState<OrganizationShareReview[]>([]);
  const [publisherId, setPublisherId] = useState("");
  const [publisherDisplayName, setPublisherDisplayName] = useState("");
  const [publisherPublicKey, setPublisherPublicKey] = useState("");
  const [publisherRegistries, setPublisherRegistries] = useState("");
  const [publisherReason, setPublisherReason] = useState("");
  const [organizationShareJson, setOrganizationShareJson] = useState("");
  const [organizationShareReviewId, setOrganizationShareReviewId] = useState(() => crypto.randomUUID());
  const [organizationShareReasons, setOrganizationShareReasons] = useState<Record<string, string>>({});
  const [organizationShareNames, setOrganizationShareNames] = useState<Record<string, string>>({});
  const [organizationShareImportIds, setOrganizationShareImportIds] = useState<Record<string, string>>({});
  const [organizationShareBusy, setOrganizationShareBusy] = useState(false);
  const [organizationShareError, setOrganizationShareError] = useState<string | null>(null);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [projectErrorSurface, setProjectErrorSurface] = useState<"task" | "projects">("projects");
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([]);
  const [schedules, setSchedules] = useState<ScheduleDefinition[]>([]);
  const [scheduleNotifications, setScheduleNotifications] = useState<SchedulerNotification[]>([]);
  const [scheduleHistory, setScheduleHistory] = useState<Record<string, ScheduleHistoryResponse | undefined>>({});
  const [scheduleId, setScheduleId] = useState(freshScheduleId);
  const [scheduleName, setScheduleName] = useState("");
  const [scheduleGoal, setScheduleGoal] = useState("");
  const [scheduleSetupPrompt, setScheduleSetupPrompt] = useState("");
  const [scheduleSearch, setScheduleSearch] = useState("");
  const [scheduleFilter, setScheduleFilter] = useState<ScheduleFilter>("all");
  const [scheduleDestination, setScheduleDestination] = useState<"standalone" | "chat">("standalone");
  const [scheduleKind, setScheduleKind] = useState<ScheduleTrigger["kind"]>("interval");
  const [scheduleDetail, setScheduleDetail] = useState("3600");
  const [scheduleSecondary, setScheduleSecondary] = useState("30");
  const [scheduleRisk, setScheduleRisk] = useState<RiskLevel>("medium");
  const [scheduleMaxTokens, setScheduleMaxTokens] = useState("32768");
  const [scheduleMaxCost, setScheduleMaxCost] = useState("");
  const [scheduleMaxDuration, setScheduleMaxDuration] = useState("3600");
  const [scheduleModelCalls, setScheduleModelCalls] = useState("16");
  const [scheduleBusy, setScheduleBusy] = useState(false);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const [platformCapabilities, setPlatformCapabilities] = useState<PlatformCapabilitySnapshot | null>(null);
  const [platformSessions, setPlatformSessions] = useState<PlatformSession[]>([]);
  const [platformHistory, setPlatformHistory] = useState<Record<string, PlatformSessionHistoryResponse | undefined>>({});
  const [platformSessionId, setPlatformSessionId] = useState(freshPlatformSessionId);
  const [platformName, setPlatformName] = useState("");
  const [platformKind, setPlatformKind] = useState<PlatformSessionKind>("browser");
  const [platformOrigins, setPlatformOrigins] = useState("");
  const [platformBrowserUrls, setPlatformBrowserUrls] = useState<Record<string, string>>({});
  const [platformBrowsers, setPlatformBrowsers] = useState<Record<string, PersistentBrowserObservation | undefined>>({});
  const [platformComputerFrames, setPlatformComputerFrames] = useState<Record<string, ComputerUseFrame | undefined>>({});
  const [platformComputerCoordinates, setPlatformComputerCoordinates] = useState<Record<string, { x: string; y: string }>>({});
  const [platformComputerText, setPlatformComputerText] = useState<Record<string, string>>({});
  const [platformComputerBatchIds, setPlatformComputerBatchIds] = useState<Record<string, string | undefined>>({});
  const [platformComputerScreenOnly, setPlatformComputerScreenOnly] = useState(false);
  const [platformComputerRegion, setPlatformComputerRegion] = useState({ x: "", y: "", width: "", height: "" });
  const [platformBusy, setPlatformBusy] = useState(false);
  const [platformError, setPlatformError] = useState<string | null>(null);
  const [sites, setSites] = useState<Site[]>([]);
  const [siteRegistrationId, setSiteRegistrationId] = useState(() => crypto.randomUUID());
  const [siteName, setSiteName] = useState("");
  const [sitePublicUrl, setSitePublicUrl] = useState("");
  const [siteHealthUrl, setSiteHealthUrl] = useState("");
  const [siteProvider, setSiteProvider] = useState("");
  const [siteDeployExecutable, setSiteDeployExecutable] = useState("");
  const [siteDeployArguments, setSiteDeployArguments] = useState("");
  const [siteBusy, setSiteBusy] = useState(false);
  const [siteError, setSiteError] = useState<string | null>(null);
  const [siteHealth, setSiteHealth] = useState<Record<string, SiteHealthListResponse | undefined>>({});
  const [siteDeployments, setSiteDeployments] = useState<Record<string, SiteDeploymentListResponse | undefined>>({});
  const [siteDeploymentIds, setSiteDeploymentIds] = useState<Record<string, string | undefined>>({});
  const [siteMonitors, setSiteMonitors] = useState<Record<string, SiteMonitor | undefined>>({});
  const [siteMonitorIntervals, setSiteMonitorIntervals] = useState<Record<string, string>>({});
  const [sshHosts, setSshHosts] = useState<SshHost[]>([]);
  const [sshHostId, setSshHostId] = useState(() => crypto.randomUUID());
  const [sshName, setSshName] = useState("");
  const [sshHostname, setSshHostname] = useState("");
  const [sshUsername, setSshUsername] = useState("");
  const [sshKnownHosts, setSshKnownHosts] = useState("");
  const [sshFingerprint, setSshFingerprint] = useState("");
  const [sshCapabilities, setSshCapabilities] = useState("");
  const [sshOperationJson, setSshOperationJson] = useState<Record<string, string>>({});
  const [sshOperations, setSshOperations] = useState<Record<string, SshOperationListResponse | undefined>>({});
  const [sshWorkflows, setSshWorkflows] = useState<SshWorkflow[]>([]);
  const [sshWorkflowId, setSshWorkflowId] = useState(() => crypto.randomUUID());
  const [sshWorkflowName, setSshWorkflowName] = useState("");
  const [sshWorkflowHosts, setSshWorkflowHosts] = useState<string[]>([]);
  const [sshWorkflowStepsJson, setSshWorkflowStepsJson] = useState("");
  const [sshBusy, setSshBusy] = useState(false);
  const [sshError, setSshError] = useState<string | null>(null);
  const [observations, setObservations] = useState<SupervisionObservation[]>([]);
  const [experiments, setExperiments] = useState<ExperimentDefinition[]>([]);
  const [experimentOrganizationId, setExperimentOrganizationId] = useState("");
  const [experimentJson, setExperimentJson] = useState("");
  const [experimentAction, setExperimentAction] = useState<"candidate" | "observation" | "experiment" | "shadow" | "benchmark" | "decision">("observation");
  const [experimentTargetId, setExperimentTargetId] = useState("");
  const [experimentEvidence, setExperimentEvidence] = useState<Record<string, { shadow: number; benchmarks: number; decisions: number }>>({});
  const [experimentBusy, setExperimentBusy] = useState(false);
  const [experimentError, setExperimentError] = useState<string | null>(null);

  useEffect(() => {
    const previousScrollRestoration = window.history.scrollRestoration;
    window.history.scrollRestoration = "manual";
    window.requestAnimationFrame(() => window.scrollTo({ top: 0, left: 0, behavior: "instant" }));
    return () => {
      window.history.scrollRestoration = previousScrollRestoration;
    };
  }, []);

  useEffect(() => {
    window.localStorage.setItem("mcg.pinned-task-ids", JSON.stringify(pinnedTaskIds));
  }, [pinnedTaskIds]);

  useEffect(() => {
    window.localStorage.setItem("mcg.global-folders", JSON.stringify(globalFolders));
  }, [globalFolders]);

  useEffect(() => {
    const restoreSurface = () => {
      const surface = window.location.hash.slice(1);
      if (surface) setActiveSurface(surface);
      window.requestAnimationFrame(() => window.scrollTo({ top: 0, left: 0, behavior: "instant" }));
    };
    window.addEventListener("popstate", restoreSurface);
    return () => window.removeEventListener("popstate", restoreSurface);
  }, []);

  useEffect(() => {
    const handleAppShortcut = (event: KeyboardEvent) => {
      const command = event.ctrlKey || event.metaKey;
      const key = event.key.toLocaleLowerCase();
      if (event.key === "Escape") {
        setDesktopMenu(null);
        return;
      }
      if (event.key === "F11") {
        event.preventDefault();
        void toggleFullScreen();
        return;
      }
      if (!command || event.altKey) return;
      if (key === "o") {
        event.preventDefault();
        if (!globalFolderBusy) void addGlobalFolder();
      } else if (key === "n" && !event.shiftKey) {
        event.preventDefault();
        startNewChat();
      } else if (key === "b" && !event.shiftKey) {
        event.preventDefault();
        setSidebarOpen((current) => !current);
      } else if (key === "e" && event.shiftKey) {
        event.preventDefault();
        setProjectTreeOpen((current) => !current);
      } else if (key === "f") {
        event.preventDefault();
        setSidebarOpen(true);
        window.requestAnimationFrame(() => chatSearchRef.current?.focus());
      } else if (event.key === ",") {
        event.preventDefault();
        openSettings("general");
      } else if (event.key === "/") {
        event.preventDefault();
        openSettings("shortcuts");
      } else if (key === "w") {
        event.preventDefault();
        void closeCurrentWindow();
      } else if (key === "q") {
        event.preventDefault();
        void closeCurrentWindow(true);
      } else if (event.key === "[") {
        event.preventDefault();
        if (event.shiftKey) openAdjacentChat(-1);
        else window.history.back();
      } else if (event.key === "]") {
        event.preventDefault();
        if (event.shiftKey) openAdjacentChat(1);
        else window.history.forward();
      } else if (key === "0") {
        event.preventDefault();
        setAppZoom(1);
      } else if (key === "-" || key === "_") {
        event.preventDefault();
        setAppZoom(zoomLevel - 0.1);
      } else if (key === "+" || key === "=") {
        event.preventDefault();
        setAppZoom(zoomLevel + 0.1);
      }
    };
    window.addEventListener("keydown", handleAppShortcut);
    return () => window.removeEventListener("keydown", handleAppShortcut);
  }, [globalFolderBusy, selectedChatId, tasks, zoomLevel]);

  function navigateSurface(surface: string, addHistory = true) {
    setActiveSurface(surface);
    setOwnerMenuOpen(false);
    setDesktopMenu(null);
    if (addHistory) window.history.pushState(null, "", `#${surface}`);
    window.requestAnimationFrame(() => {
      const target = surface === "tasks"
        ? document.querySelector<HTMLElement>(".workspace")
        : surface === "threads"
          ? document.querySelector<HTMLElement>(".task-panel")
          : document.getElementById(surface);
      if (target) {
        const headerOffset = advanced
          ? (document.querySelector<HTMLElement>(".topbar")?.offsetHeight ?? 0) + 18
          : 0;
        const top = advanced
          ? Math.max(0, target.getBoundingClientRect().top + window.scrollY - headerOffset)
          : 0;
        window.scrollTo({ top, left: 0, behavior: "instant" });
      } else {
        window.scrollTo({ top: 0, left: 0, behavior: "instant" });
      }
    });
  }

  function openSettings(tab: SettingsTab) {
    setSettingsTab(tab);
    navigateSurface("settings");
  }

  function openAdjacentChat(offset: -1 | 1) {
    if (tasks.length === 0) return;
    const current = selectedChatId ? tasks.findIndex((task) => task.id === selectedChatId) : -1;
    const next = current < 0
      ? (offset > 0 ? 0 : tasks.length - 1)
      : (current + offset + tasks.length) % tasks.length;
    openChat(tasks[next].id);
  }

  function runEditCommand(command: "undo" | "redo" | "cut" | "copy" | "paste" | "delete" | "selectAll") {
    document.execCommand(command);
    setDesktopMenu(null);
  }

  function setAppZoom(next: number) {
    const bounded = Math.min(2, Math.max(0.5, Math.round(next * 10) / 10));
    setZoomLevel(bounded);
    document.body.style.zoom = String(bounded);
    setDesktopMenu(null);
  }

  async function openNewWindow() {
    setDesktopMenu(null);
    if (!("__TAURI_INTERNALS__" in window)) {
      window.open(window.location.href, "_blank", "noopener,noreferrer");
      return;
    }
    new WebviewWindow(`mcg-${Date.now()}`, {
      url: window.location.href,
      title: "MCG",
      width: Math.max(960, window.innerWidth),
      height: Math.max(700, window.innerHeight)
    });
  }

  async function closeCurrentWindow(force = false) {
    setDesktopMenu(null);
    if (!("__TAURI_INTERNALS__" in window)) {
      window.close();
      return;
    }
    if (force) await getCurrentWindow().destroy();
    else await getCurrentWindow().close();
  }

  async function toggleFullScreen() {
    setDesktopMenu(null);
    if ("__TAURI_INTERNALS__" in window) {
      const currentWindow = getCurrentWindow();
      await currentWindow.setFullscreen(!(await currentWindow.isFullscreen()));
      return;
    }
    if (document.fullscreenElement) await document.exitFullscreen();
    else await document.documentElement.requestFullscreen();
  }

  function openSurface(event: MouseEvent<HTMLAnchorElement>, surface: string) {
    event.preventDefault();
    navigateSurface(surface);
  }

  function startNewChat(project: ProjectModel | null = null) {
    setSelectedProject(project);
    setSelectedChatId(null);
    setTaskGoal("");
    setTaskConstraints("");
    setTaskSuccess("");
    setTaskError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function startSiteChat(project: ProjectModel | null = selectedProject) {
    setSelectedProject(project);
    setSelectedChatId(null);
    setTaskGoal("Build a website. Start by asking me about its audience, purpose, required behavior, and information, then build and test a reviewable version before publishing it.");
    setTaskConstraints("");
    setTaskSuccess("");
    setTaskError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function startCodeReview() {
    if (!selectedProject?.repository.is_git) {
      setCodeReviewError("Choose a project backed by a Git repository before starting a review.");
      return;
    }
    const projectName = selectedProject.root.split(/[\\/]/).filter(Boolean).at(-1) ?? selectedProject.root;
    const branch = selectedProject.repository.branch ?? "the current branch";
    const scope = codeReviewScope === "branch"
      ? `the changes on ${branch} compared with ${codeReviewBase.trim() || "the base branch"}`
      : codeReviewScope === "uncommitted"
        ? "all staged, unstaged, and untracked changes"
        : codeReviewScope === "commit"
          ? `the current commit${selectedProject.repository.head ? ` ${selectedProject.repository.head}` : ""}`
          : codeReviewInstructions.trim() || "the code changes I describe next";
    setSelectedChatId(null);
    setTaskGoal(`${CODE_REVIEW_GOAL_PREFIX} in ${projectName}: review ${scope}. Prioritize correctness, regressions, security, missing tests, and repository instructions. Report concise, actionable findings with file and line references, ordered by severity. Do not modify the working tree during the review.`);
    setTaskConstraints("");
    setTaskSuccess("");
    setTaskError(null);
    setCodeReviewError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function continueSiteChat(site: Site) {
    const project = projects.find((candidate) => candidate.project_id === site.project_id) ?? null;
    setSelectedProject(project);
    setSelectedChatId(null);
    setTaskGoal(`Refine the website ${site.display_name} at ${site.public_url}. Inspect the existing project, show me a reviewable version, and do not publish changes until they are approved.`);
    setTaskConstraints("");
    setTaskSuccess("");
    setTaskError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function openChat(taskId: string) {
    setSelectedChatId(taskId);
    navigateSurface("threads");
  }

  function togglePinnedTask(taskId: string) {
    setPinnedTaskIds((current) => current.includes(taskId)
      ? current.filter((candidate) => candidate !== taskId)
      : [taskId, ...current]);
  }

  useEffect(() => {
    let active = true;
    getLocalServiceStatus()
      .then((status) => {
        if (active) setLocalService(status);
      })
      .catch((error) => {
        if (active) setLocalService({
          ready: false,
          launched: false,
          service: "mcg-daemon",
          version: "unknown",
          detail: errorMessage(error)
        });
      });
    getUpdateConfiguration()
      .then((configuration) => {
        if (active) setUpdateConfiguration(configuration);
      })
      .catch((error) => {
        if (active) setUpdateError(errorMessage(error));
      });
    getUserSettings()
      .then((settings) => {
        if (active) {
          setUserSettings(settings);
          setAdvanced(settings.advanced_mode);
        }
      })
      .catch((error) => {
        if (active) setSettingsError(errorMessage(error));
      });
    loadUserSettingsHistory()
      .then((response) => {
        if (active) setSettingsHistory(response.revisions);
      })
      .catch((error) => {
        if (active) setSettingsError(errorMessage(error));
      });
    listBackups()
      .then((response) => {
        if (active) setBackups(response.backups);
      })
      .catch((error) => {
        if (active) setBackupError(errorMessage(error));
      });
    listMcpConnections()
      .then((response) => {
        if (active) setMcpConnections(response.connections);
      })
      .catch((error) => {
        if (active) setMcpError(errorMessage(error));
      });
    listPlugins()
      .then((response) => {
        if (active) setPlugins(response.plugins);
      })
      .catch((error) => {
        if (active) setPluginError(errorMessage(error));
      });
    listProviders()
      .then((response) => {
        if (active) {
          setProviders(response.providers);
          const subscription = response.providers.find(({ connection }) =>
            connection.kind === "open_ai"
            && (connection.auth_mode === "chat_gpt_browser_oauth" || connection.auth_mode === "chat_gpt_device_code")
          );
          if (subscription) setProviderStatus(`${subscription.connection.display_name} connected`);
        }
      })
      .catch((error) => {
        if (active) setProviderError(errorMessage(error));
      });
    providerProxyUsage()
      .then((response) => {
        if (active) setProxyUsage(response);
      })
      .catch((error) => {
        if (active) setProviderError(errorMessage(error));
      });
    providerProxyConversations()
      .then((response) => {
        if (active) setProxyConversations(response.conversations);
      })
      .catch((error) => {
        if (active) setProviderError(errorMessage(error));
      });
    listTasks()
      .then((response) => {
        if (active) setTasks(response.tasks);
      })
      .catch((error) => {
        if (active) setTaskError(errorMessage(error));
      });
    listProjects()
      .then((response) => {
        if (active) setProjects(response.projects);
      })
      .catch((error) => {
        if (active) setProjectError(errorMessage(error));
      });
    listOrganizations()
      .then((response) => {
        if (active) {
          setOrganizations(response.organizations);
          setOrganizationBaseId((current) => current || response.organizations[0]?.organization_id || "");
        }
      })
      .catch((error) => {
        if (active) setOrganizationError(errorMessage(error));
      });
    listOrganizationPublishers()
      .then((publishers) => {
        if (active) setOrganizationPublishers(publishers);
      })
      .catch((error) => {
        if (active) setOrganizationShareError(errorMessage(error));
      });
    listOrganizationShareReviews()
      .then((reviews) => {
        if (active) setOrganizationShareReviews(reviews);
      })
      .catch((error) => {
        if (active) setOrganizationShareError(errorMessage(error));
      });
    listApprovals()
      .then((response) => {
        if (active) setApprovals(response.approvals);
      })
      .catch((error) => {
        if (active) setTaskError(errorMessage(error));
      });
    listSchedules()
      .then((response) => {
        if (active) setSchedules(response.schedules);
      })
      .catch((error) => {
        if (active) setScheduleError(errorMessage(error));
      });
    listSchedulerNotifications()
      .then((response) => {
        if (active) setScheduleNotifications(response.notifications);
      })
      .catch((error) => {
        if (active) setScheduleError(errorMessage(error));
      });
    loadPlatformCapabilities()
      .then((snapshot) => {
        if (active) setPlatformCapabilities(snapshot);
      })
      .catch((error) => {
        if (active) setPlatformError(errorMessage(error));
      });
    listPlatformSessions()
      .then((response) => {
        if (active) setPlatformSessions(response.sessions);
      })
      .catch((error) => {
        if (active) setPlatformError(errorMessage(error));
      });
    listSites()
      .then(async (response) => {
        if (active) setSites(response.sites);
        const monitors = await Promise.all(response.sites.map(async (site) => {
          const monitor = await getSiteMonitor(site.id).catch(() => undefined);
          return [site.id, monitor] as const;
        }));
        if (active) setSiteMonitors(Object.fromEntries(monitors));
      })
      .catch((error) => {
        if (active) setSiteError(errorMessage(error));
      });
    listSshHosts()
      .then((response) => { if (active) setSshHosts(response.hosts); })
      .catch((error) => { if (active) setSshError(errorMessage(error)); });
    listSshWorkflows()
      .then((response) => { if (active) setSshWorkflows(response.workflows); })
      .catch((error) => { if (active) setSshError(errorMessage(error)); });
    listSupervisionObservations()
      .then((response) => { if (active) setObservations(response.observations); })
      .catch((error) => { if (active) setExperimentError(errorMessage(error)); });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    setMemoryError(null);
    setTopologyPlanError(null);
    setReviewedTopologyError(null);
    setReviewedTopologyApprovals({});
    setMemoryHistory({});
    setMemoryTransferSource(null);
    setMemoryTransferPreview(null);
    setMemoryTransferTarget("");
    setMemoryTransferReason("");
    setMemoryTransferId(crypto.randomUUID());
    if (!selectedProject) {
      setMemoryClaims([]);
      setMemoryTransfers([]);
      setManagedMemory("");
      setTopologyPlan(null);
      setReviewedTopologyPlans([]);
      return () => { active = false; };
    }
    setTopologyPlan(null);
    Promise.all([
      listProjectMemory(selectedProject.project_id),
      loadManagedProjectMemory(selectedProject.project_id),
      listProjectMemoryTransfers(selectedProject.project_id),
      listReviewedTopologyPlans(selectedProject.project_id)
    ])
      .then(([claims, managed, transfers, reviewedPlans]) => {
        if (active) {
          setMemoryClaims(claims.claims);
          setManagedMemory(managed.markdown);
          setMemoryTransfers(transfers.transfers);
          setReviewedTopologyPlans(reviewedPlans.plans);
        }
      })
      .catch((error) => {
        if (active) setMemoryError(errorMessage(error));
      });
    projectTopologyPlan(selectedProject.project_id)
      .then((plan) => {
        if (active) {
          setTopologyPlan(plan);
          setReviewedTopologyStage(String(plan.stages.at(-1)?.index ?? 0));
        }
      })
      .catch((error) => { if (active) setTopologyPlanError(errorMessage(error)); });
    return () => { active = false; };
  }, [selectedProject]);

  async function saveUserSettings(next: UserSettings, reason: string) {
    if (!userSettings) return;
    setSettingsBusy(true);
    setSettingsError(null);
    try {
      const saved = await updateUserSettings({
        expected_revision: userSettings.revision,
        advanced_mode: next.advanced_mode,
        autonomy: next.autonomy,
        shadow_experiments: next.shadow_experiments,
        reason
      });
      setUserSettings(saved);
      setAdvanced(saved.advanced_mode);
      setSettingsHistory((current) => [...current, { settings: saved, reason, changed_at: saved.updated_at }]);
    } catch (error) {
      setSettingsError(errorMessage(error));
      const current = await getUserSettings().catch(() => null);
      if (current) {
        setUserSettings(current);
        setAdvanced(current.advanced_mode);
      }
    } finally {
      setSettingsBusy(false);
    }
  }

  async function toggleAdvancedMode() {
    if (!userSettings || settingsBusy) return;
    const restoreFocus = document.activeElement === modeButtonRef.current;
    const nextAdvanced = !advanced;
    if (!nextAdvanced && scheduleKind !== "interval") {
      setScheduleKind("interval");
      setScheduleDetail("3600");
      setScheduleSecondary("30");
    }
    if (!nextAdvanced) {
      setBackupPassphrase("");
      setBackupConfirmation("");
    }
    await saveUserSettings(
      { ...userSettings, advanced_mode: nextAdvanced },
      nextAdvanced ? "Enabled advanced interface" : "Enabled simple interface"
    );
    if (restoreFocus) {
      window.requestAnimationFrame(() => modeButtonRef.current?.focus());
    }
  }

  async function makeBackup() {
    setBackupBusy(true);
    setBackupError(null);
    try {
      const backup = await createBackup(crypto.randomUUID());
      setBackups((current) => [...current.filter((item) => item.backup_id !== backup.backup_id), backup]);
    } catch (error) {
      setBackupError(errorMessage(error));
    } finally {
      setBackupBusy(false);
    }
  }

  async function makeEncryptedBackup() {
    setBackupError(null);
    if (backupPassphrase !== backupConfirmation) {
      setBackupError("Encrypted backup passphrases do not match.");
      return;
    }
    setBackupBusy(true);
    try {
      const backup = await createEncryptedBackup(crypto.randomUUID(), backupPassphrase);
      setBackups((current) => [...current.filter((item) => item.backup_id !== backup.backup_id), backup]);
      setBackupPassphrase("");
      setBackupConfirmation("");
    } catch (error) {
      setBackupError(errorMessage(error));
    } finally {
      setBackupBusy(false);
    }
  }

  async function checkBackup(backupId: string) {
    setBackupBusy(true);
    setBackupError(null);
    try {
      const backup = await verifyBackup(backupId);
      setBackups((current) => current.map((item) => item.backup_id === backupId ? backup : item));
    } catch (error) {
      setBackupError(errorMessage(error));
    } finally {
      setBackupBusy(false);
    }
  }

  async function checkSystem() {
    setDiagnosticsBusy(true);
    setDiagnosticsError(null);
    try {
      setDiagnostics(await runDiagnostics());
    } catch (error) {
      setDiagnosticsError(errorMessage(error));
    } finally {
      setDiagnosticsBusy(false);
    }
  }

  async function checkUpdates() {
    setUpdateBusy(true);
    setUpdateError(null);
    setUpdateReview(false);
    try {
      setUpdateCheck(await checkForUpdate());
    } catch (error) {
      setUpdateError(errorMessage(error));
    } finally {
      setUpdateBusy(false);
    }
  }

  async function applyUpdate() {
    if (!updateCheck?.available_version) return;
    setUpdateBusy(true);
    setUpdateError(null);
    try {
      await installUpdate(updateCheck.available_version, crypto.randomUUID());
    } catch (error) {
      setUpdateError(errorMessage(error));
      setUpdateReview(false);
      setUpdateBusy(false);
    }
  }

  async function startLogin(method: OpenAiOAuthMethod) {
    setBusy(true);
    setProviderError(null);
    try {
      const started = await startOpenAiOAuth(method);
      setLogin(started);
      setProviderStatus("Waiting for authorization");
      try {
        await openExternalUrl(started.verification_url);
      } catch (error) {
        const retryLabel = started.user_code ? "Open device authorization page" : "Open sign-in page";
        setProviderError(`${errorMessage(error)}. Use “${retryLabel}” below.`);
      }
    } catch (error) {
      setProviderError(errorMessage(error));
      setProviderStatus("Connection failed");
    } finally {
      setBusy(false);
    }
  }

  async function openVerification(url: string) {
    try {
      await openExternalUrl(url);
      setProviderError(null);
    } catch (error) {
      setProviderError(errorMessage(error));
    }
  }

  async function finishLogin() {
    if (!login) return;
    setBusy(true);
    setProviderError(null);
    try {
      const completed = await completeOpenAiOAuth(login.session_id);
      setProviderStatus(`${completed.connection.display_name} connected`);
      setProviders((current) => [
        { connection: completed.connection, catalog: completed.catalog },
        ...current.filter((item) => item.connection.id !== completed.connection.id)
      ]);
      setLogin(null);
    } catch (error) {
      setProviderError(errorMessage(error));
      setProviderStatus("Connection failed");
    } finally {
      setBusy(false);
    }
  }

  async function cancelLogin() {
    if (!login) return;
    setBusy(true);
    try {
      await cancelOpenAiOAuth(login.session_id);
      setLogin(null);
      setProviderStatus("Not connected");
      setProviderError(null);
    } catch (error) {
      setProviderError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  function generatedProviderId(): string {
    const source = (providerId || providerName || "provider").toLowerCase();
    const slug = source.replace(/[^a-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "");
    return (slug || "provider").slice(0, 128);
  }

  async function addProvider() {
    if (!providerName.trim() || !providerUrl.trim()) {
      setProviderError("Provider name and base URL are required.");
      return;
    }
    if (providerAuth !== "none" && !providerCredential) {
      setProviderError("Enter the credential for this provider.");
      return;
    }
    setBusy(true);
    setProviderError(null);
    try {
      const mutation = await configureProvider(
        {
          contract_version: 1,
          id: generatedProviderId(),
          display_name: providerName.trim(),
          kind: providerKind,
          base_url: providerUrl.trim(),
          auth_mode: providerAuth,
          organization: advanced && providerOrganization.trim() ? providerOrganization.trim() : undefined,
          project: advanced && providerProject.trim() ? providerProject.trim() : undefined
        },
        providerAuth === "none" ? undefined : providerCredential
      );
      setProviders((current) => [
        { connection: mutation.connection, catalog: mutation.catalog },
        ...current.filter((item) => item.connection.id !== mutation.connection.id)
      ]);
      setProviderStatus(`${mutation.connection.display_name} connected`);
      setProviderName("");
      setProviderId("");
      setProviderUrl("");
      setProviderCredential("");
      setProviderOrganization("");
      setProviderProject("");
      setProviderSetup(false);
    } catch (error) {
      setProviderError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function refreshProvider(connectionId: string) {
    setBusy(true);
    setProviderError(null);
    try {
      const catalog = await discoverProvider(connectionId);
      setProviders((current) => current.map((item) =>
        item.connection.id === connectionId ? { ...item, catalog } : item
      ));
    } catch (error) {
      setProviderError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeProvider(item: ProviderListItem) {
    if (!window.confirm(`Disconnect ${item.connection.display_name}? Its protected credentials will be removed from this computer.`)) return;
    setBusy(true);
    setProviderError(null);
    try {
      await disconnectProvider(item.connection.id);
      setProviders((current) => current.filter((provider) => provider.connection.id !== item.connection.id));
      setProviderStatus(`${item.connection.display_name} disconnected`);
    } catch (error) {
      setProviderError(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function logOutOfOpenAi() {
    const oauthConnections = providers.filter(({ connection }) =>
      connection.kind === "open_ai"
      && (connection.auth_mode === "chat_gpt_browser_oauth" || connection.auth_mode === "chat_gpt_device_code")
    );
    if (oauthConnections.length === 0) return;
    setBusy(true);
    setProviderError(null);
    const disconnected = new Set<string>();
    try {
      for (const item of oauthConnections) {
        await disconnectProvider(item.connection.id);
        disconnected.add(item.connection.id);
      }
      setProviderStatus("Signed out of ChatGPT subscription");
      setOwnerMenuOpen(false);
    } catch (error) {
      setProviderError(errorMessage(error));
      setProviderStatus("Sign out failed");
    } finally {
      if (disconnected.size > 0) setProviders((current) => current.filter(({ connection }) => !disconnected.has(connection.id)));
      setBusy(false);
    }
  }

  function generatedMcpId(): string {
    const source = (mcpId || mcpName || "server").toLowerCase();
    const slug = source.replace(/[^a-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "");
    return (slug || "server").slice(0, 128);
  }

  async function addMcpServer() {
    const stdio = advanced && mcpTransport === "stdio";
    if (!mcpName.trim() || (stdio ? !mcpCommand.trim() : !mcpUrl.trim())) {
      setMcpError(stdio ? "Server name and executable are required." : "Server name and URL are required.");
      return;
    }
    if (!stdio && advanced && mcpAuth === "bearer_token" && !mcpCredential) {
      setMcpError("Enter the bearer token for this server.");
      return;
    }
    setMcpBusy(true);
    setMcpError(null);
    try {
      const mutation = await configureMcpConnection(
        {
          contract_version: 1,
          id: generatedMcpId(),
          display_name: mcpName.trim(),
          transport: stdio
            ? {
                kind: "stdio",
                command: mcpCommand.trim(),
                args: mcpArgs.split("\n").map((argument) => argument.trim()).filter(Boolean)
              }
            : { kind: "streamable_http", url: mcpUrl.trim() },
          auth_mode: stdio ? "none" : advanced ? mcpAuth : "none",
          protocol_mode: advanced ? mcpProtocol : "auto",
          enabled: true
        },
        !stdio && advanced && mcpAuth === "bearer_token" ? mcpCredential : undefined
      );
      setMcpConnections((connections) => [
        ...connections.filter((item) => item.connection.id !== mutation.connection.id),
        { connection: mutation.connection, catalog: mutation.catalog }
      ]);
      setMcpName("");
      setMcpId("");
      setMcpUrl("");
      setMcpCommand("");
      setMcpArgs("");
      setMcpCredential("");
    } catch (error) {
      setMcpError(errorMessage(error));
    } finally {
      setMcpBusy(false);
    }
  }

  async function refreshMcpServer(connectionId: string) {
    setMcpBusy(true);
    setMcpError(null);
    try {
      const catalog = await discoverMcpConnection(connectionId);
      setMcpConnections((connections) =>
        connections.map((item) =>
          item.connection.id === connectionId ? { ...item, catalog } : item
        )
      );
    } catch (error) {
      setMcpError(errorMessage(error));
    } finally {
      setMcpBusy(false);
    }
  }

  async function refreshPlugins() {
    const response = await listPlugins();
    setPlugins(response.plugins);
  }

  async function importPlugin() {
    if (!pluginManifestFile || !pluginModuleFile) {
      setPluginError("Choose both a plugin manifest and a WebAssembly module.");
      return;
    }
    if (pluginManifestFile.size > 64 * 1024 || pluginModuleFile.size > 64 * 1024 * 1024) {
      setPluginError("The manifest must be at most 64 KiB and the module at most 64 MiB.");
      return;
    }
    setPluginBusy(true);
    setPluginError(null);
    try {
      const manifest = JSON.parse(await pluginManifestFile.text()) as PluginManifest;
      if (manifest.schema_version !== 1 || manifest.class !== "sandboxed_wasi" || !manifest.id || !manifest.display_name || !manifest.version) {
        throw new Error("The manifest is not a supported sandboxed WASI plugin manifest.");
      }
      const moduleBytes = new Uint8Array(await pluginModuleFile.arrayBuffer());
      await installLocalPlugin({
        revision: {
          schema_version: 1,
          id: crypto.randomUUID(),
          manifest: { ...manifest, requested_capabilities: manifest.requested_capabilities ?? [] },
          module_sha256: await sha256Hex(moduleBytes),
          module_size_bytes: moduleBytes.byteLength,
          source: { kind: "local_import" },
          signature: { state: "unsigned" },
          installed_at: new Date().toISOString()
        },
        module_base64: bytesToBase64(moduleBytes)
      });
      await refreshPlugins();
      setPluginManifestFile(null);
      setPluginModuleFile(null);
    } catch (error) {
      setPluginError(errorMessage(error));
    } finally {
      setPluginBusy(false);
    }
  }

  async function importRegistryPlugin() {
    if (!pluginRegistryPackageFile) {
      setPluginError("Choose a signed registry package request.");
      return;
    }
    if (pluginRegistryPackageFile.size > 90 * 1024 * 1024) {
      setPluginError("The signed registry package request must be at most 90 MiB.");
      return;
    }
    setPluginBusy(true);
    setPluginError(null);
    try {
      const request = JSON.parse(await pluginRegistryPackageFile.text()) as InstallRegistryPluginRequest;
      await installRegistryPlugin(request);
      await refreshPlugins();
      setPluginRegistryPackageFile(null);
    } catch (error) {
      setPluginError(errorMessage(error));
    } finally {
      setPluginBusy(false);
    }
  }

  function draftPluginScope(pluginId: string, field: string): string {
    return pluginScopeDrafts[pluginId]?.[field] ?? "";
  }

  function updatePluginScope(pluginId: string, field: string, value: string) {
    setPluginScopeDrafts((current) => ({
      ...current,
      [pluginId]: { ...current[pluginId], [field]: value }
    }));
  }

  function requestedPluginGrants(plugin: PluginDescriptor): PluginCapabilityGrant[] {
    return plugin.active_revision.manifest.requested_capabilities.map((capability) => {
      let scopes: PluginCapabilityGrant["scopes"];
      if (capability === "filesystem_read" || capability === "filesystem_write") {
        scopes = [{ kind: "task_project" }];
      } else if (capability === "network") {
        const origin = draftPluginScope(plugin.installation.plugin_id, "network").trim();
        if (!origin) throw new Error("Enter the exact network origin this plugin may contact.");
        scopes = [{ kind: "network_origin", origin }];
      } else if (capability === "process") {
        const executable = draftPluginScope(plugin.installation.plugin_id, "process").trim();
        if (!executable) throw new Error("Enter the exact executable this plugin may run.");
        scopes = [{ kind: "executable", executable }];
      } else {
        const reference = draftPluginScope(plugin.installation.plugin_id, "credentials").trim();
        const separator = reference.indexOf(":");
        if (separator <= 0 || separator === reference.length - 1) {
          throw new Error("Enter a credential reference as provider:key.");
        }
        scopes = [{ kind: "credential", provider: reference.slice(0, separator), key: reference.slice(separator + 1) }];
      }
      return { schema_version: 1, capability, scopes };
    });
  }

  async function grantPluginAccess(plugin: PluginDescriptor) {
    setPluginBusy(true);
    setPluginError(null);
    try {
      await setPluginGrants(plugin.installation.plugin_id, {
        expected_revision: plugin.installation.revision,
        grants: requestedPluginGrants(plugin)
      });
      await refreshPlugins();
    } catch (error) {
      setPluginError(errorMessage(error));
    } finally {
      setPluginBusy(false);
    }
  }

  async function togglePlugin(plugin: PluginDescriptor) {
    setPluginBusy(true);
    setPluginError(null);
    try {
      await setPluginEnabled(
        plugin.installation.plugin_id,
        plugin.installation.revision,
        !plugin.installation.enabled
      );
      await refreshPlugins();
    } catch (error) {
      setPluginError(errorMessage(error));
    } finally {
      setPluginBusy(false);
    }
  }

  function usePluginInNewChat(plugin: PluginDescriptor) {
    if (!plugin.installation.enabled) {
      setPluginError("Enable this plugin before using it in a new chat.");
      return;
    }
    setSelectedChatId(null);
    setTaskGoal(`Use @${plugin.active_revision.manifest.display_name} to `);
    setTaskConstraints("");
    setTaskSuccess("");
    setTaskError(null);
    setPluginError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  async function runPlugin(plugin: PluginDescriptor) {
    setPluginBusy(true);
    setPluginError(null);
    try {
      const input = JSON.parse(pluginInputDrafts[plugin.installation.plugin_id] || "{}");
      const mutation = await invokePlugin(plugin.installation.plugin_id, crypto.randomUUID(), input);
      setPluginInvocations((current) => ({ ...current, [plugin.installation.plugin_id]: mutation }));
    } catch (error) {
      setPluginError(errorMessage(error));
    } finally {
      setPluginBusy(false);
    }
  }

  async function refreshPluginHistory(plugin: PluginDescriptor) {
    const invocation = pluginInvocations[plugin.installation.plugin_id]?.invocation;
    if (!invocation) return;
    setPluginBusy(true);
    setPluginError(null);
    try {
      const response = await getPluginInvocation(plugin.installation.plugin_id, invocation.id);
      setPluginHistoryCounts((current) => ({ ...current, [plugin.installation.plugin_id]: response.history.length }));
    } catch (error) {
      setPluginError(errorMessage(error));
    } finally {
      setPluginBusy(false);
    }
  }

  function replaceTask(task: TaskEnvelope) {
    setTasks((current) => [
      task,
      ...current.filter((candidate) => candidate.id !== task.id)
    ]);
  }

  async function addTask() {
    if (!taskGoal.trim()) {
      setTaskError("Describe what you want MCG to accomplish.");
      return;
    }
    setTaskBusy(true);
    setTaskError(null);
    try {
      const maxCost = Number(taskMaxCost);
      const created = await createTask({
        goal: taskGoal.trim(),
        project_id: selectedProject?.project_id,
        constraints: advanced
          ? taskConstraints.split("\n").map((value) => value.trim()).filter(Boolean)
          : [],
        success_conditions: advanced
          ? taskSuccess.split("\n").map((value) => value.trim()).filter(Boolean)
          : [],
        risk: advanced ? taskRisk : undefined,
        budget: advanced && taskMaxCost.trim() && Number.isFinite(maxCost)
          ? { max_cost_usd: maxCost }
          : undefined
      });
      replaceTask(created.task);
      setTaskGoal("");
      setTaskConstraints("");
      setTaskSuccess("");
      setTaskMaxCost("");
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function startTask(taskId: string) {
    setTaskBusy(true);
    setTaskError(null);
    setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Planning and starting governed work..." }));
    try {
      const launched = await launchTask(taskId);
      replaceTask(launched.task);
      setTaskRunIds((runs) => ({ ...runs, [taskId]: launched.run.run.id }));
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: launched.execution.detail }));
    } catch (error) {
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Launch failed." }));
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function retryBlockedTask(taskId: string) {
    setTaskBusy(true);
    setTaskError(null);
    setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Retrying incomplete governed work..." }));
    try {
      const retried = await retryTask(taskId);
      replaceTask(retried.task);
      setTaskRunIds((runs) => ({ ...runs, [taskId]: retried.run.run.id }));
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: retried.execution.detail }));
    } catch (error) {
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Retry failed." }));
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function approveAndResume(taskId: string, approvalId: string) {
    setTaskBusy(true);
    setTaskError(null);
    setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Applying approval and resuming..." }));
    try {
      await approveRequest(approvalId);
      const resumed = await resumeTask(taskId);
      replaceTask(resumed.task);
      setTaskRunIds((runs) => ({ ...runs, [taskId]: resumed.run.run.id }));
      setApprovals((current) => current.filter((approval) => approval.id !== approvalId));
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: resumed.execution.detail }));
    } catch (error) {
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Approval continuation failed." }));
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function denyApprovalRequest(taskId: string, approvalId: string) {
    setTaskBusy(true);
    setTaskError(null);
    setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Denying request and revoking waiting authority..." }));
    try {
      await denyRequest(approvalId);
      const current = await listTasks();
      const settled = current.tasks.find((task) => task.id === taskId);
      if (!settled) throw new Error("Denied task disappeared from durable task listing");
      replaceTask(settled);
      setApprovals((approvals) => approvals.filter((approval) => approval.id !== approvalId));
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Request denied; task is blocked and can be retried." }));
    } catch (error) {
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Approval denial failed." }));
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function changeTaskState(taskId: string, state: "cancelled") {
    setTaskBusy(true);
    setTaskError(null);
    try {
      replaceTask(await transitionTask(taskId, state));
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function toggleTaskHistory(taskId: string) {
    if (taskHistory[taskId]) {
      setTaskHistory((history) => ({ ...history, [taskId]: undefined }));
      setTaskAudit((audit) => ({ ...audit, [taskId]: undefined }));
      return;
    }
    setTaskBusy(true);
    setTaskError(null);
    try {
      const [events, audit] = await Promise.all([loadTaskEvents(taskId), loadTaskAudit(taskId)]);
      setTaskHistory((history) => ({ ...history, [taskId]: events }));
      setTaskAudit((current) => ({ ...current, [taskId]: audit }));
      const durableRunId = events.events
        .slice()
        .reverse()
        .find((event) => event.payload?.run?.id)?.payload?.run?.id;
      if (durableRunId) setTaskRunIds((runs) => ({ ...runs, [taskId]: durableRunId }));
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function verifyTaskReplay(taskId: string, runId: string) {
    setReplayBusyTask(taskId);
    setTaskError(null);
    try {
      const report = await replayAgentRun(runId);
      setTaskReplays((replays) => ({ ...replays, [taskId]: report }));
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setReplayBusyTask(null);
    }
  }

  function taskEvidence(task: TaskEnvelope) {
    const history = taskHistory[task.id];
    const audit = taskAudit[task.id];
    return history && audit
      ? <TaskEvidence
          history={history}
          audit={audit}
          advanced={advanced}
          state={task.state}
          replay={taskReplays[task.id]}
          replayBusy={replayBusyTask === task.id}
          onReplay={taskRunIds[task.id] ? () => verifyTaskReplay(task.id, taskRunIds[task.id]!) : undefined}
        />
      : null;
  }

  async function refreshProjectMemory(project: ProjectModel) {
    const [claims, managed, transfers] = await Promise.all([
      listProjectMemory(project.project_id),
      loadManagedProjectMemory(project.project_id),
      listProjectMemoryTransfers(project.project_id)
    ]);
    setMemoryClaims(claims.claims);
    setManagedMemory(managed.markdown);
    setMemoryTransfers(transfers.transfers);
  }

  async function addProjectMemory() {
    if (!selectedProject) {
      setMemoryError("Select a project before adding memory.");
      return;
    }
    if (!memoryKey.trim()) {
      setMemoryError("Name the fact you want MCG to remember.");
      return;
    }
    setMemoryBusy(true);
    setMemoryError(null);
    try {
      const confidence = Number(memoryConfidence);
      if (!Number.isFinite(confidence) || confidence < 0 || confidence > 1) {
        throw new Error("Memory confidence must be between zero and one.");
      }
      await createProjectMemory(selectedProject.project_id, {
        namespace: advanced ? memoryNamespace.trim() || "project" : "project",
        key: memoryKey.trim(),
        value: memoryValue(memoryDraft),
        confidence,
        provenance: desktopMemoryProvenance,
        fresh_until: advanced && memoryFreshUntil ? new Date(memoryFreshUntil).toISOString() : undefined
      });
      setMemoryKey("");
      setMemoryDraft("");
      setMemoryFreshUntil("");
      await refreshProjectMemory(selectedProject);
    } catch (error) {
      setMemoryError(errorMessage(error));
    } finally {
      setMemoryBusy(false);
    }
  }

  async function correctMemory(claim: MemoryClaim) {
    if (!selectedProject) return;
    setMemoryBusy(true);
    setMemoryError(null);
    try {
      await correctProjectMemory(selectedProject.project_id, claim.id, {
        value: memoryValue(memoryCorrections[claim.id] ?? JSON.stringify(claim.value)),
        confidence: claim.confidence,
        provenance: desktopMemoryProvenance,
        fresh_until: claim.fresh_until
      });
      setMemoryCorrections((current) => ({ ...current, [claim.id]: "" }));
      await refreshProjectMemory(selectedProject);
    } catch (error) {
      setMemoryError(errorMessage(error));
    } finally {
      setMemoryBusy(false);
    }
  }

  async function transitionMemory(claim: MemoryClaim, action: MemoryLifecycleAction) {
    if (!selectedProject) return;
    setMemoryBusy(true);
    setMemoryError(null);
    try {
      await transitionProjectMemory(selectedProject.project_id, claim.id, action);
      await refreshProjectMemory(selectedProject);
    } catch (error) {
      setMemoryError(errorMessage(error));
    } finally {
      setMemoryBusy(false);
    }
  }

  async function inspectMemoryHistory(claim: MemoryClaim) {
    if (!selectedProject) return;
    setMemoryBusy(true);
    setMemoryError(null);
    try {
      const history = await loadProjectMemoryHistory(
        selectedProject.project_id,
        claim.namespace,
        claim.key
      );
      setMemoryHistory((current) => ({ ...current, [claim.id]: history }));
    } catch (error) {
      setMemoryError(errorMessage(error));
    } finally {
      setMemoryBusy(false);
    }
  }

  async function beginMemoryTransfer(claim: MemoryClaim) {
    if (!selectedProject) return;
    setMemoryBusy(true);
    setMemoryError(null);
    try {
      const preview = await reviewProjectMemoryTransfer(selectedProject.project_id, claim.id);
      setMemoryTransferSource(claim);
      setMemoryTransferPreview(preview);
      setMemoryTransferTarget("");
      setMemoryTransferReason("");
      setMemoryTransferId(crypto.randomUUID());
    } catch (error) {
      setMemoryError(errorMessage(error));
    } finally {
      setMemoryBusy(false);
    }
  }

  async function confirmMemoryTransfer() {
    if (!selectedProject || !memoryTransferSource || !memoryTransferPreview) return;
    if (!memoryTransferTarget) {
      setMemoryError("Choose a different project for this fact.");
      return;
    }
    if (!memoryTransferReason.trim()) {
      setMemoryError("Explain why this fact should also apply to the target project.");
      return;
    }
    setMemoryBusy(true);
    setMemoryError(null);
    try {
      await transferProjectMemory(selectedProject.project_id, {
        transfer_id: memoryTransferId,
        source_claim_id: memoryTransferSource.id,
        source_claim_sha256: memoryTransferPreview.source_claim_sha256,
        target_project_id: memoryTransferTarget,
        reason: memoryTransferReason.trim()
      });
      setMemoryTransferSource(null);
      setMemoryTransferPreview(null);
      setMemoryTransferTarget("");
      setMemoryTransferReason("");
      setMemoryTransferId(crypto.randomUUID());
      await refreshProjectMemory(selectedProject);
    } catch (error) {
      setMemoryError(errorMessage(error));
    } finally {
      setMemoryBusy(false);
    }
  }

  async function chooseProject(surface: "task" | "projects") {
    setTaskBusy(true);
    setProjectError(null);
    setProjectErrorSurface(surface);
    try {
      const root = await pickProjectDirectory();
      if (!root) return;
      const existingProject = projects.find((project) => projectRootKey(project.root) === projectRootKey(root));
      const mutation = existingProject
        ? await discoverProject(root, existingProject.project_id)
        : await discoverProject(root);
      setSelectedProject(mutation.project);
      setProjects((current) => [
        mutation.project,
        ...current.filter((project) => project.project_id !== mutation.project.project_id)
      ]);
      if (surface === "projects" && !advanced) {
        setSelectedChatId(null);
        navigateSurface("tasks");
        window.requestAnimationFrame(() => goalInputRef.current?.focus());
      }
    } catch (error) {
      setProjectError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  async function addGlobalFolder() {
    setGlobalFolderBusy(true);
    setGlobalFolderError(null);
    try {
      const root = await pickProjectDirectory();
      if (!root) return;
      setGlobalFolders((current) => current.some((folder) => projectRootKey(folder) === projectRootKey(root))
        ? current
        : [...current, root]);
    } catch (error) {
      setGlobalFolderError(errorMessage(error));
    } finally {
      setGlobalFolderBusy(false);
    }
  }

  function replaceReviewedTopologyPlan(plan: ReviewedTopologyPlan) {
    setReviewedTopologyPlans((current) => [
      plan,
      ...current.filter((candidate) => candidate.id !== plan.id)
    ]);
  }

  async function reviewTopologyExecution() {
    if (!selectedProject || !topologyPlan || topologyPlan.status !== "ready") return;
    const command = selectedProject.commands[Number(reviewedTopologyCommand)];
    const stageIndex = Number(reviewedTopologyStage);
    if (!command || !topologyPlan.stages.some((stage) => stage.index === stageIndex)) {
      setReviewedTopologyError("Choose one discovered command and one available topology stage.");
      return;
    }
    if (!reviewedTopologyReason.trim()) {
      setReviewedTopologyError("Explain why this command should run for this reviewed plan.");
      return;
    }
    setReviewedTopologyBusy(true);
    setReviewedTopologyError(null);
    try {
      const projectName = selectedProject.root.split(/[\\/]/).filter(Boolean).at(-1) ?? selectedProject.root;
      const mutation = await createReviewedTopologyPlan(selectedProject.project_id, {
        id: reviewedTopologyPlanId,
        project_snapshot_id: topologyPlan.project_snapshot_id,
        topology_sha256: topologyPlan.sha256,
        display_name: `${command.name} · ${projectName}`,
        reason: reviewedTopologyReason.trim(),
        steps: [{
          id: reviewedTopologyStepId,
          stage_index: stageIndex,
          name: `Run ${command.name}`,
          command_name: command.name,
          command: command.command,
          timeout_seconds: 1800
        }]
      });
      replaceReviewedTopologyPlan(mutation.plan);
      setReviewedTopologyPlanId(crypto.randomUUID());
      setReviewedTopologyStepId(crypto.randomUUID());
      setReviewedTopologyReason("");
    } catch (error) {
      setReviewedTopologyError(reviewedTopologyErrorMessage(error));
    } finally {
      setReviewedTopologyBusy(false);
    }
  }

  async function executeReviewedTopologyPlan(planId: string) {
    if (!selectedProject) return;
    setReviewedTopologyBusy(true);
    setReviewedTopologyError(null);
    try {
      const mutation = await runReviewedTopologyPlan(selectedProject.project_id, planId);
      replaceReviewedTopologyPlan(mutation.plan);
      setReviewedTopologyApprovals((current) => ({
        ...current,
        [planId]: mutation.approval_id ?? undefined
      }));
      if (mutation.approval_id) {
        const response = await listApprovals().catch(() => null);
        if (response) setApprovals(response.approvals);
      }
    } catch (error) {
      setReviewedTopologyError(errorMessage(error));
    } finally {
      setReviewedTopologyBusy(false);
    }
  }

  async function approveReviewedTopologyPlan(planId: string) {
    const approvalId = reviewedTopologyApprovals[planId];
    if (!approvalId) return;
    setReviewedTopologyBusy(true);
    setReviewedTopologyError(null);
    try {
      await approveRequest(approvalId);
      setApprovals((current) => current.filter((approval) => approval.id !== approvalId));
      setReviewedTopologyApprovals((current) => ({ ...current, [planId]: undefined }));
    } catch (error) {
      setReviewedTopologyError(errorMessage(error));
      setReviewedTopologyBusy(false);
      return;
    }
    setReviewedTopologyBusy(false);
    await executeReviewedTopologyPlan(planId);
  }

  async function stopReviewedTopologyPlan(planId: string) {
    if (!selectedProject) return;
    setReviewedTopologyBusy(true);
    setReviewedTopologyError(null);
    try {
      const mutation = await cancelReviewedTopologyPlan(selectedProject.project_id, planId);
      replaceReviewedTopologyPlan(mutation.plan);
      setReviewedTopologyApprovals((current) => ({ ...current, [planId]: undefined }));
    } catch (error) {
      setReviewedTopologyError(errorMessage(error));
    } finally {
      setReviewedTopologyBusy(false);
    }
  }

  function configuredScheduleTrigger(): ScheduleTrigger {
    if (scheduleKind === "once") {
      const at = new Date(scheduleDetail);
      if (Number.isNaN(at.getTime())) throw new Error("Choose a valid date and time.");
      return { kind: "once", at: at.toISOString() };
    }
    if (scheduleKind === "interval") {
      return { kind: "interval", seconds: positiveWholeNumber(scheduleDetail, "Interval") };
    }
    if (scheduleKind === "cron") {
      return { kind: "cron", expression: scheduleDetail };
    }
    if (scheduleKind === "file_watch") {
      return { kind: "file_watch", relative_path: scheduleDetail, poll_seconds: positiveWholeNumber(scheduleSecondary, "Poll interval") };
    }
    if (scheduleKind === "event") {
      return { kind: "event", event_kind: scheduleDetail };
    }
    return {
      kind: "heartbeat",
      idle_seconds: positiveWholeNumber(scheduleDetail, "Idle heartbeat"),
      busy_seconds: positiveWholeNumber(scheduleSecondary, "Busy heartbeat")
    };
  }

  function replaceSchedule(schedule: ScheduleDefinition) {
    setSchedules((current) => [schedule, ...current.filter((item) => item.id !== schedule.id)]);
  }

  async function addSchedule() {
    if (!selectedProject) {
      setScheduleError("Select a project before scheduling work.");
      return;
    }
    if (!scheduleName.trim() || !scheduleGoal.trim()) {
      setScheduleError("Name the schedule and describe its goal.");
      return;
    }
    setScheduleBusy(true);
    setScheduleError(null);
    try {
      const maxCost = scheduleMaxCost.trim() ? Number(scheduleMaxCost) : undefined;
      if (maxCost !== undefined && (!Number.isFinite(maxCost) || maxCost < 0)) {
        throw new Error("Cost limit must be zero or a positive dollar amount.");
      }
      const schedule = await createSchedule({
        schedule_id: scheduleId,
        name: scheduleName.trim(),
        project_id: selectedProject.project_id,
        goal: scheduleGoal.trim(),
        constraints: [],
        success_conditions: [],
        risk: scheduleRisk,
        budget: {
          max_tokens: positiveWholeNumber(scheduleMaxTokens, "Token limit"),
          max_cost_usd: maxCost,
          max_duration_seconds: positiveWholeNumber(scheduleMaxDuration, "Duration limit")
        },
        trigger: configuredScheduleTrigger(),
        context_token_budget: 8192,
        requested_tools: [],
        estimated_output_tokens: 8192,
        max_model_calls: positiveWholeNumber(scheduleModelCalls, "Model call limit")
      });
      replaceSchedule(schedule);
      setScheduleId(freshScheduleId());
      setScheduleName("");
      setScheduleGoal("");
    } catch (error) {
      setScheduleError(errorMessage(error));
    } finally {
      setScheduleBusy(false);
    }
  }

  async function changeScheduleState(scheduleId: string, state: "enabled" | "paused" | "completed") {
    setScheduleBusy(true);
    setScheduleError(null);
    try {
      replaceSchedule(await setScheduleState(scheduleId, state));
    } catch (error) {
      setScheduleError(errorMessage(error));
    } finally {
      setScheduleBusy(false);
    }
  }

  async function toggleScheduleHistory(scheduleId: string) {
    if (scheduleHistory[scheduleId]) {
      setScheduleHistory((current) => ({ ...current, [scheduleId]: undefined }));
      return;
    }
    setScheduleBusy(true);
    try {
      const history = await loadScheduleHistory(scheduleId);
      setScheduleHistory((current) => ({ ...current, [scheduleId]: history }));
    } catch (error) {
      setScheduleError(errorMessage(error));
    } finally {
      setScheduleBusy(false);
    }
  }

  async function requestScheduleWake(scheduleId: string) {
    setScheduleBusy(true);
    try {
      await wakeSchedule(scheduleId, "Owner requested immediate attention from the desktop");
      const [scheduleResponse, notificationResponse] = await Promise.all([
        listSchedules(),
        listSchedulerNotifications()
      ]);
      setSchedules(scheduleResponse.schedules);
      setScheduleNotifications(notificationResponse.notifications);
    } catch (error) {
      setScheduleError(errorMessage(error));
    } finally {
      setScheduleBusy(false);
    }
  }

  async function acknowledgeNotification(notificationId: string) {
    try {
      await acknowledgeSchedulerNotification(notificationId);
      setScheduleNotifications((current) => current.filter((item) => item.id !== notificationId));
    } catch (error) {
      setScheduleError(errorMessage(error));
    }
  }

  async function acknowledgeAllNotifications() {
    if (scheduleNotifications.length === 0) return;
    setScheduleBusy(true);
    setScheduleError(null);
    try {
      await Promise.all(scheduleNotifications.map((notification) => acknowledgeSchedulerNotification(notification.id)));
      setScheduleNotifications([]);
    } catch (error) {
      setScheduleError(errorMessage(error));
    } finally {
      setScheduleBusy(false);
    }
  }

  function startScheduleChat() {
    if (!selectedProject) {
      setScheduleError("Choose a project before setting up scheduled work.");
      return;
    }
    const projectName = selectedProject.root.split(/[\\/]/).filter(Boolean).at(-1) ?? selectedProject.root;
    const requestedWork = scheduleSetupPrompt.trim() || "Help me decide what recurring work would be useful for this project.";
    const destination = scheduleDestination === "chat"
      ? "Return to this setup chat on each run so the work keeps its conversation context."
      : "Start a separate chat for each run so every result is independent and appears in Scheduled.";
    setSelectedChatId(null);
    setTaskGoal(`Set up scheduled work for ${projectName}. Requested work: ${requestedWork} ${destination} First explain the unattended security boundary, then interview me for the exact cadence, stopping conditions, and permissions. Help me test and refine the prompt before enabling it. Use the narrowest access that can succeed. If this build cannot create the final schedule from chat yet, give me a complete reviewable schedule proposal and state that limitation clearly.`);
    setTaskConstraints("");
    setTaskSuccess("");
    setTaskError(null);
    setScheduleError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function scheduleTriggerLabel(trigger: ScheduleTrigger): string {
    if (trigger.kind === "once") return new Date(trigger.at).toLocaleString();
    if (trigger.kind === "interval") {
      if (trigger.seconds < 60) return `Every ${trigger.seconds} seconds`;
      if (trigger.seconds % 3600 === 0) return `Every ${trigger.seconds / 3600} ${trigger.seconds === 3600 ? "hour" : "hours"}`;
      return `Every ${trigger.seconds / 60} minutes`;
    }
    if (trigger.kind === "cron") return `Cron ${trigger.expression} UTC`;
    if (trigger.kind === "file_watch") return `Watch ${trigger.relative_path}`;
    if (trigger.kind === "event") return `On ${trigger.event_kind.replaceAll("_", " ")}`;
    return `Heartbeat ${trigger.busy_seconds}s busy / ${trigger.idle_seconds}s idle`;
  }

  function replacePlatformSession(session: PlatformSession) {
    setPlatformSessions((current) => [session, ...current.filter((item) => item.id !== session.id)]);
  }

  async function reservePlatformSession() {
    if (!platformName.trim()) {
      setPlatformError("Name the session so it is easy to recognize.");
      return;
    }
    const allowedOrigins = platformOrigins.split(/\r?\n/).map((value) => value.trim()).filter(Boolean);
    if (platformKind === "browser" && allowedOrigins.length === 0) {
      setPlatformError("Add at least one allowed site before reserving a browser session.");
      return;
    }
    const regionValues = Object.values(platformComputerRegion);
    const regionRequested = platformKind === "computer_use" && regionValues.some((value) => value.trim() !== "");
    const parsedRegion = regionValues.map((value) => Number(value));
    if (regionRequested && (
      regionValues.some((value) => value.trim() === "")
      || parsedRegion.some((value) => !Number.isSafeInteger(value))
      || parsedRegion[0] < 0
      || parsedRegion[1] < 0
      || parsedRegion[2] <= 0
      || parsedRegion[3] <= 0
    )) {
      setPlatformError("Enter all four region values as whole numbers; X and Y may be zero, and width and height must be positive.");
      return;
    }
    const computerTarget = regionRequested ? {
      region: {
        x: parsedRegion[0],
        y: parsedRegion[1],
        width: parsedRegion[2],
        height: parsedRegion[3]
      }
    } : {};
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const session = await requestPlatformSession({
        session_id: platformSessionId,
        kind: platformKind,
        project_id: selectedProject?.project_id,
        display_name: platformName.trim(),
        allowed_origins: platformKind === "browser"
          ? allowedOrigins
          : [],
        computer_capabilities: platformKind === "computer_use"
          ? (platformComputerScreenOnly ? ["screen_read"] : ["screen_read", "mouse", "keyboard"])
          : [],
        computer_target: platformKind === "computer_use" ? computerTarget : undefined,
        lifetime_seconds: 3600
      });
      replacePlatformSession(session);
      setPlatformName("");
      setPlatformOrigins("");
      setPlatformComputerRegion({ x: "", y: "", width: "", height: "" });
      setPlatformSessionId(freshPlatformSessionId());
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function closePlatform(sessionId: string) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      replacePlatformSession(await closePlatformSession(sessionId, "Local owner closed the session from the desktop"));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function attachPlatformBrowser(sessionId: string, visible: boolean) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const result = await attachBrowserSession(sessionId, visible ? "visible" : "headless");
      replacePlatformSession(result.session);
      setPlatformBrowsers((current) => ({ ...current, [sessionId]: result.browser }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function navigatePlatformBrowser(sessionId: string) {
    const url = platformBrowserUrls[sessionId]?.trim();
    if (!url) {
      setPlatformError("Enter a URL inside this session's allowed sites.");
      return;
    }
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const browser = await navigateBrowserSession(sessionId, url);
      setPlatformBrowsers((current) => ({ ...current, [sessionId]: browser }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function handoffPlatformBrowser(sessionId: string) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const result = await handoffBrowserSession(sessionId);
      replacePlatformSession(result.session);
      setPlatformBrowsers((current) => ({ ...current, [sessionId]: result.browser }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function reclaimPlatformBrowser(sessionId: string) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const result = await reclaimBrowserSession(sessionId);
      replacePlatformSession(result.session);
      setPlatformBrowsers((current) => ({ ...current, [sessionId]: result.browser }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function attachPlatformComputer(sessionId: string) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const result = await attachComputerSession(sessionId);
      replacePlatformSession(result.session);
      const frame = await captureComputerSession(sessionId);
      setPlatformComputerFrames((current) => ({ ...current, [sessionId]: frame }));
      setPlatformComputerBatchIds((current) => ({ ...current, [sessionId]: undefined }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function capturePlatformComputer(sessionId: string) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const frame = await captureComputerSession(sessionId);
      setPlatformComputerFrames((current) => ({ ...current, [sessionId]: frame }));
      setPlatformComputerBatchIds((current) => ({ ...current, [sessionId]: undefined }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function actOnPlatformComputer(sessionId: string, includeText: boolean) {
    const frame = platformComputerFrames[sessionId];
    const coordinates = platformComputerCoordinates[sessionId] ?? { x: "0", y: "0" };
    const x = Number(coordinates.x);
    const y = Number(coordinates.y);
    if (!frame || !Number.isInteger(x) || !Number.isInteger(y) || x < 0 || y < 0) {
      setPlatformError("Capture a current frame and enter whole-number coordinates inside it.");
      return;
    }
    const text = platformComputerText[sessionId] ?? "";
    if (includeText && !text) {
      setPlatformError("Enter text to type after the in-region click.");
      return;
    }
    setPlatformBusy(true);
    setPlatformError(null);
    const batchId = platformComputerBatchIds[sessionId] ?? freshPlatformSessionId();
    setPlatformComputerBatchIds((current) => ({ ...current, [sessionId]: batchId }));
    try {
      const mutation = await applyComputerActions(sessionId, {
        batch_id: batchId,
        expected_frame_sha256: frame.sha256,
        actions: [
          { type: "click", x, y, button: "left" },
          ...(includeText ? [{ type: "type_text" as const, text }] : [])
        ]
      });
      const nextFrame = mutation.result?.frame ?? await captureComputerSession(sessionId);
      setPlatformComputerFrames((current) => ({ ...current, [sessionId]: nextFrame }));
      setPlatformComputerBatchIds((current) => ({ ...current, [sessionId]: undefined }));
      if (includeText) setPlatformComputerText((current) => ({ ...current, [sessionId]: "" }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function handoffPlatformComputer(sessionId: string) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const result = await handoffComputerSession(sessionId);
      replacePlatformSession(result.session);
      setPlatformComputerFrames((current) => ({ ...current, [sessionId]: undefined }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function reclaimPlatformComputer(sessionId: string) {
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const result = await reclaimComputerSession(sessionId);
      replacePlatformSession(result.session);
      setPlatformComputerFrames((current) => ({ ...current, [sessionId]: undefined }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function togglePlatformHistory(sessionId: string) {
    if (platformHistory[sessionId]) {
      setPlatformHistory((current) => ({ ...current, [sessionId]: undefined }));
      return;
    }
    setPlatformBusy(true);
    try {
      const history = await loadPlatformSessionHistory(sessionId);
      setPlatformHistory((current) => ({ ...current, [sessionId]: history }));
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  async function addSite() {
    if (!selectedProject || !siteName.trim() || !sitePublicUrl.trim() || !siteHealthUrl.trim() || !siteProvider.trim()) {
      setSiteError("Choose a project and enter the Site name, public URL, health URL, and hosting provider.");
      return;
    }
    setSiteBusy(true);
    setSiteError(null);
    try {
      const site = await registerSite({
        site_id: siteRegistrationId,
        project_id: selectedProject.project_id,
        display_name: siteName.trim(),
        environment: "production",
        public_url: sitePublicUrl.trim(),
        health_url: siteHealthUrl.trim(),
        hosting_provider: siteProvider.trim(),
        deployment_command: siteDeployExecutable.trim() ? {
          executable: siteDeployExecutable.trim(),
          arguments: siteDeployArguments.split(/\r?\n/).map((value) => value.trim()).filter(Boolean),
          working_directory: ""
        } : undefined,
        state: "active"
      });
      setSites((current) => [site, ...current.filter((item) => item.id !== site.id)]);
      setSiteRegistrationId(crypto.randomUUID());
      setSiteName("");
      setSitePublicUrl("");
      setSiteHealthUrl("");
      setSiteProvider("");
      setSiteDeployExecutable("");
      setSiteDeployArguments("");
    } catch (error) {
      setSiteError(errorMessage(error));
    } finally {
      setSiteBusy(false);
    }
  }

  async function addSshHost() {
    if (!sshName.trim() || !sshHostname.trim() || !sshUsername.trim() || !sshKnownHosts.trim() || !sshFingerprint.trim()) {
      setSshError("Enter the name, host, user, pinned known_hosts line, and verified SHA256 fingerprint.");
      return;
    }
    setSshBusy(true); setSshError(null);
    try {
      const host = await registerSshHost({
        host_id: sshHostId, display_name: sshName.trim(), hostname: sshHostname.trim(), port: 22,
        username: sshUsername.trim(), platform: "posix", authentication: "agent",
        known_hosts_entry: sshKnownHosts.trim(), host_key_sha256: sshFingerprint.trim(),
        capabilities: sshCapabilities.split(/[\s,]+/).map((value) => value.trim().toLowerCase()).filter(Boolean), state: "active"
      });
      setSshHosts((current) => [host, ...current.filter((item) => item.id !== host.id)]);
      setSshHostId(crypto.randomUUID()); setSshName(""); setSshHostname(""); setSshUsername("");
      setSshKnownHosts(""); setSshFingerprint(""); setSshCapabilities("");
    } catch (error) { setSshError(errorMessage(error)); } finally { setSshBusy(false); }
  }

  async function executeSsh(host: SshHost, operation?: SshOperationKind) {
    setSshBusy(true); setSshError(null);
    try {
      const selected = operation ?? JSON.parse(sshOperationJson[host.id] || "") as SshOperationKind;
      await runSshOperation(host.id, { operation_id: crypto.randomUUID(), operation: selected, timeout_seconds: 60 });
      const history = await loadSshOperations(host.id);
      setSshOperations((current) => ({ ...current, [host.id]: history }));
    } catch (error) { setSshError(errorMessage(error)); } finally { setSshBusy(false); }
  }

  function toggleSshWorkflowHost(hostId: string) {
    setSshWorkflowHosts((current) => current.includes(hostId)
      ? current.filter((id) => id !== hostId)
      : [...current, hostId]);
  }

  async function addSshWorkflow() {
    if (!sshWorkflowName.trim()) {
      setSshError("Give the workflow a name.");
      return;
    }
    let steps: SshWorkflowStepRequest[];
    try {
      if (advanced && sshWorkflowStepsJson.trim()) {
        steps = JSON.parse(sshWorkflowStepsJson) as SshWorkflowStepRequest[];
      } else {
        const selected = sshWorkflowHosts
          .map((id) => sshHosts.find((host) => host.id === id))
          .filter((host): host is SshHost => host !== undefined && host.state === "active");
        if (selected.length === 0) {
          setSshError("Select at least one active host for the workflow.");
          return;
        }
        let previous: string | undefined;
        steps = selected.map((host) => {
          const id = crypto.randomUUID();
          const step = {
            id,
            name: `Inspect ${host.display_name}`,
            host_id: host.id,
            host_revision: host.revision,
            required_capabilities: [],
            dependencies: previous ? [previous] : [],
            operation: { kind: "inspect" } as const,
            timeout_seconds: 60
          };
          previous = id;
          return step;
        });
      }
    } catch {
      setSshError("Advanced workflow steps must be a valid JSON array.");
      return;
    }
    setSshBusy(true); setSshError(null);
    try {
      const mutation = await createSshWorkflow({
        id: sshWorkflowId,
        display_name: sshWorkflowName.trim(),
        steps
      });
      setSshWorkflows((current) => [mutation.workflow, ...current.filter((item) => item.id !== mutation.workflow.id)]);
      setSshWorkflowId(crypto.randomUUID()); setSshWorkflowName(""); setSshWorkflowHosts([]); setSshWorkflowStepsJson("");
    } catch (error) { setSshError(errorMessage(error)); } finally { setSshBusy(false); }
  }

  async function mutateSshWorkflow(workflow: SshWorkflow, action: "run" | "cancel") {
    setSshBusy(true); setSshError(null);
    try {
      const mutation = action === "run"
        ? await runSshWorkflow(workflow.id)
        : await cancelSshWorkflow(workflow.id);
      setSshWorkflows((current) => current.map((item) => item.id === mutation.workflow.id ? mutation.workflow : item));
    } catch (error) { setSshError(errorMessage(error)); } finally { setSshBusy(false); }
  }

  async function inspectSite(siteId: string) {
    setSiteBusy(true);
    setSiteError(null);
    try {
      const [health, deployments] = await Promise.all([
        loadSiteHealth(siteId),
        loadSiteDeployments(siteId)
      ]);
      setSiteHealth((current) => ({ ...current, [siteId]: health }));
      setSiteDeployments((current) => ({ ...current, [siteId]: deployments }));
    } catch (error) {
      setSiteError(errorMessage(error));
    } finally {
      setSiteBusy(false);
    }
  }

  async function runSiteHealth(siteId: string) {
    setSiteBusy(true);
    setSiteError(null);
    try {
      const mutation = await checkSiteHealth(siteId, crypto.randomUUID());
      setSiteHealth((current) => ({
        ...current,
        [siteId]: { checks: [mutation.check, ...(current[siteId]?.checks ?? []).filter((check) => check.id !== mutation.check.id)] }
      }));
    } catch (error) {
      setSiteError(errorMessage(error));
    } finally {
      setSiteBusy(false);
    }
  }

  async function enableSiteMonitoring(siteId: string) {
    setSiteBusy(true);
    setSiteError(null);
    try {
      const intervalSeconds = positiveWholeNumber(
        siteMonitorIntervals[siteId] ?? "300",
        "Monitor interval"
      );
      const mutation = await createSiteMonitor(siteId, {
        schedule_id: crypto.randomUUID(),
        interval_seconds: intervalSeconds,
        expected_status: 200
      });
      setSiteMonitors((current) => ({ ...current, [siteId]: mutation.monitor }));
    } catch (error) {
      setSiteError(errorMessage(error));
    } finally {
      setSiteBusy(false);
    }
  }

  async function runSiteDeployment(siteId: string) {
    const deploymentId = siteDeploymentIds[siteId] ?? crypto.randomUUID();
    setSiteDeploymentIds((current) => ({ ...current, [siteId]: deploymentId }));
    setSiteBusy(true);
    setSiteError(null);
    try {
      const mutation = await deploySite(siteId, deploymentId);
      setSiteDeployments((current) => ({
        ...current,
        [siteId]: { deployments: [mutation.deployment, ...(current[siteId]?.deployments ?? []).filter((deployment) => deployment.id !== mutation.deployment.id)] }
      }));
      setSiteDeploymentIds((current) => ({ ...current, [siteId]: undefined }));
    } catch (error) {
      setSiteError(errorMessage(error));
      const response = await listApprovals().catch(() => null);
      if (response) setApprovals(response.approvals);
    } finally {
      setSiteBusy(false);
    }
  }

  async function approveSiteDeployment(siteId: string, approvalId: string) {
    setSiteBusy(true);
    setSiteError(null);
    try {
      await approveRequest(approvalId);
      setApprovals((current) => current.filter((approval) => approval.id !== approvalId));
    } catch (error) {
      setSiteError(errorMessage(error));
      setSiteBusy(false);
      return;
    }
    setSiteBusy(false);
    await runSiteDeployment(siteId);
  }

  async function changeSiteState(site: Site, state: Site["state"]) {
    setSiteBusy(true);
    setSiteError(null);
    try {
      const mutation = await updateSite(site.id, {
        expected_revision: site.revision,
        display_name: site.display_name,
        environment: site.environment,
        public_url: site.public_url,
        health_url: site.health_url,
        hosting_provider: site.hosting_provider,
        deployment_command: site.deployment_command,
        credential_reference: site.credential_reference,
        state,
        reason: state === "archived" ? "Archived by local owner" : "Monitoring state changed by local owner"
      });
      setSites((current) => current.map((item) => item.id === site.id ? mutation.site : item));
    } catch (error) {
      setSiteError(errorMessage(error));
    } finally {
      setSiteBusy(false);
    }
  }

  async function refreshOrganizations(preferredId?: string) {
    const response = await listOrganizations();
    setOrganizations(response.organizations);
    const next = preferredId
      ?? (response.organizations.some((item) => item.organization_id === organizationBaseId)
        ? organizationBaseId
        : response.organizations[0]?.organization_id);
    setOrganizationBaseId(next ?? "");
    setOrganizationCompareLeft((current) => current || response.organizations[0]?.revision_id || "");
    setOrganizationCompareRight((current) => current || response.organizations[1]?.revision_id || response.organizations[0]?.revision_id || "");
  }

  async function addOrganization() {
    setOrganizationBusy(true);
    setOrganizationError(null);
    try {
      const mutation = await createOrganization(organizationName);
      setOrganizationName("");
      await refreshOrganizations(mutation.organization.organization_id);
    } catch (error) {
      setOrganizationError(errorMessage(error));
    } finally {
      setOrganizationBusy(false);
    }
  }

  async function addInheritedOrganization() {
    if (!organizationBaseId) {
      setOrganizationError("Choose an organization to inherit from.");
      return;
    }
    if (!organizationName.trim()) {
      setOrganizationError("Enter a name for the inherited organization.");
      return;
    }
    setOrganizationBusy(true);
    setOrganizationError(null);
    try {
      const mutation = await inheritOrganization(organizationBaseId, organizationName);
      setOrganizationName("");
      await refreshOrganizations(mutation.organization.organization_id);
    } catch (error) {
      setOrganizationError(errorMessage(error));
    } finally {
      setOrganizationBusy(false);
    }
  }

  async function duplicateOrganization(organization: OrganizationGenome) {
    setOrganizationBusy(true);
    setOrganizationError(null);
    const importId = organizationDuplicateIds[organization.organization_id] ?? crypto.randomUUID();
    setOrganizationDuplicateIds((current) => ({ ...current, [organization.organization_id]: importId }));
    try {
      const portable = await exportOrganization(organization.revision_id);
      const mutation = await importOrganization(portable, importId, `${organization.name} copy`);
      setOrganizationDuplicateIds((current) => {
        const next = { ...current };
        delete next[organization.organization_id];
        return next;
      });
      await refreshOrganizations(mutation.organization.organization_id);
    } catch (error) {
      setOrganizationError(errorMessage(error));
    } finally {
      setOrganizationBusy(false);
    }
  }

  async function showPortableOrganization(organization: OrganizationGenome) {
    setOrganizationBusy(true);
    setOrganizationError(null);
    try {
      const portable = await exportOrganization(organization.revision_id);
      setPortableOrganizationJson(JSON.stringify(portable, null, 2));
    } catch (error) {
      setOrganizationError(errorMessage(error));
    } finally {
      setOrganizationBusy(false);
    }
  }

  async function applyPortableOrganization() {
    setOrganizationBusy(true);
    setOrganizationError(null);
    try {
      const portable = JSON.parse(portableOrganizationJson) as PortableOrganizationGenome;
      const mutation = await importOrganization(portable, organizationImportId);
      setOrganizationImportId(crypto.randomUUID());
      await refreshOrganizations(mutation.organization.organization_id);
    } catch (error) {
      setOrganizationError(errorMessage(error));
    } finally {
      setOrganizationBusy(false);
    }
  }

  async function inspectOrganizationHistory(organization: OrganizationGenome) {
    setOrganizationBusy(true);
    setOrganizationError(null);
    try {
      const response = await loadOrganizationHistory(organization.organization_id);
      setOrganizationHistory((current) => ({
        ...current,
        [organization.organization_id]: response.revisions
      }));
    } catch (error) {
      setOrganizationError(errorMessage(error));
    } finally {
      setOrganizationBusy(false);
    }
  }

  async function compareOrganizationRevisions() {
    if (!organizationCompareLeft || !organizationCompareRight) {
      setOrganizationError("Choose two organization revisions to compare.");
      return;
    }
    setOrganizationBusy(true);
    setOrganizationError(null);
    try {
      setOrganizationComparison(await compareOrganizations(organizationCompareLeft, organizationCompareRight));
    } catch (error) {
      setOrganizationError(errorMessage(error));
    } finally {
      setOrganizationBusy(false);
    }
  }

  async function refreshOrganizationSharing() {
    const [publishers, reviews] = await Promise.all([
      listOrganizationPublishers(),
      listOrganizationShareReviews()
    ]);
    setOrganizationPublishers(publishers);
    setOrganizationShareReviews(reviews);
  }

  async function saveOrganizationPublisher() {
    const registries = publisherRegistries.split(/[\s,]+/).filter(Boolean);
    if (!publisherId.trim() || !publisherDisplayName.trim() || !publisherPublicKey.trim() || registries.length === 0 || !publisherReason.trim()) {
      setOrganizationShareError("Enter the publisher identity, name, Ed25519 public key, HTTPS registry scope, and owner reason.");
      return;
    }
    setOrganizationShareBusy(true);
    setOrganizationShareError(null);
    try {
      const existing = organizationPublishers.find((publisher) => publisher.id === publisherId.trim());
      await trustOrganizationPublisher({
        id: publisherId.trim(),
        display_name: publisherDisplayName.trim(),
        public_key_base64: publisherPublicKey.trim(),
        registries,
        expected_revision: existing?.revision,
        reason: publisherReason.trim()
      });
      setPublisherReason("");
      await refreshOrganizationSharing();
    } catch (error) {
      setOrganizationShareError(errorMessage(error));
    } finally {
      setOrganizationShareBusy(false);
    }
  }

  async function revokeSharedPublisher(publisher: TrustedOrganizationPublisher) {
    if (!publisherReason.trim()) {
      setOrganizationShareError("Enter an owner reason before revoking publisher trust.");
      return;
    }
    setOrganizationShareBusy(true);
    setOrganizationShareError(null);
    try {
      await revokeOrganizationPublisher(publisher.id, publisher.revision, publisherReason);
      setPublisherReason("");
      await refreshOrganizationSharing();
    } catch (error) {
      setOrganizationShareError(errorMessage(error));
    } finally {
      setOrganizationShareBusy(false);
    }
  }

  async function reviewOrganizationShare() {
    if (!organizationShareJson.trim()) {
      setOrganizationShareError("Paste one signed organization package to verify.");
      return;
    }
    setOrganizationShareBusy(true);
    setOrganizationShareError(null);
    try {
      const organizationPackage = JSON.parse(organizationShareJson) as OrganizationSharePackage;
      await reviewSharedOrganization(organizationShareReviewId, organizationPackage);
      setOrganizationShareReviewId(crypto.randomUUID());
      setOrganizationShareJson("");
      await refreshOrganizationSharing();
    } catch (error) {
      setOrganizationShareError(errorMessage(error));
    } finally {
      setOrganizationShareBusy(false);
    }
  }

  async function importOrganizationShare(review: OrganizationShareReview) {
    const reason = organizationShareReasons[review.id]?.trim() ?? "";
    if (!reason) {
      setOrganizationShareError("Enter an owner reason before importing the verified organization.");
      return;
    }
    const importId = organizationShareImportIds[review.id] ?? crypto.randomUUID();
    setOrganizationShareImportIds((current) => ({ ...current, [review.id]: importId }));
    setOrganizationShareBusy(true);
    setOrganizationShareError(null);
    try {
      const mutation = await importSharedOrganization(
        review.id,
        importId,
        reason,
        organizationShareNames[review.id]
      );
      await Promise.all([
        refreshOrganizationSharing(),
        refreshOrganizations(mutation.organization.organization_id)
      ]);
    } catch (error) {
      setOrganizationShareError(errorMessage(error));
    } finally {
      setOrganizationShareBusy(false);
    }
  }

  async function rejectOrganizationShare(review: OrganizationShareReview) {
    const reason = organizationShareReasons[review.id]?.trim() ?? "";
    if (!reason) {
      setOrganizationShareError("Enter an owner reason before rejecting the organization share.");
      return;
    }
    setOrganizationShareBusy(true);
    setOrganizationShareError(null);
    try {
      await rejectSharedOrganization(review.id, reason);
      await refreshOrganizationSharing();
    } catch (error) {
      setOrganizationShareError(errorMessage(error));
    } finally {
      setOrganizationShareBusy(false);
    }
  }

  async function refreshExperiments() {
    if (!experimentOrganizationId.trim()) {
      setExperimentError("Enter an organization ID to load its experiments.");
      return;
    }
    setExperimentBusy(true);
    setExperimentError(null);
    try {
      const response = await listExperiments(experimentOrganizationId.trim());
      setExperiments(response.experiments);
      const evidence = await Promise.all(response.experiments.map(async (experiment) => {
        const detail = await loadExperimentEvidence(experiment.id);
        return [experiment.id, {
          shadow: detail.shadow.receipts.length,
          benchmarks: detail.benchmarks.benchmarks.length,
          decisions: detail.decisions.decisions.length
        }] as const;
      }));
      setExperimentEvidence(Object.fromEntries(evidence));
    } catch (error) {
      setExperimentError(errorMessage(error));
    } finally {
      setExperimentBusy(false);
    }
  }

  async function submitExperimentJson() {
    setExperimentBusy(true);
    setExperimentError(null);
    try {
      const value = JSON.parse(experimentJson) as Record<string, unknown>;
      if (experimentAction === "candidate") await recordCandidateRevision(value);
      if (experimentAction === "observation") await recordSupervisionObservation(value as unknown as SupervisionObservation);
      if (experimentAction === "experiment") await createExperiment(value as unknown as ExperimentDefinition);
      if (experimentAction === "shadow") await recordShadowExecution(experimentTargetId.trim(), value);
      if (experimentAction === "benchmark") await recordExperimentBenchmark(experimentTargetId.trim(), value);
      if (experimentAction === "decision") await decideExperiment(experimentTargetId.trim(), value);
      setExperimentJson("");
      const response = await listSupervisionObservations();
      setObservations(response.observations);
      if (experimentOrganizationId.trim()) await refreshExperiments();
    } catch (error) {
      setExperimentError(errorMessage(error));
    } finally {
      setExperimentBusy(false);
    }
  }

  async function advanceExperiment(experiment: ExperimentDefinition, next: ExperimentState) {
    setExperimentBusy(true);
    setExperimentError(null);
    try {
      await transitionExperiment(experiment.id, experiment.state, next);
      await refreshExperiments();
    } catch (error) {
      setExperimentError(errorMessage(error));
    } finally {
      setExperimentBusy(false);
    }
  }

  const organizationRevisionChoices = Array.from(
    new Map(
      organizations
        .flatMap((organization) => organizationHistory[organization.organization_id] ?? [organization])
        .map((revision) => [revision.revision_id, revision])
    ).values()
  );

  const normalizedChatSearch = chatSearch.trim().toLocaleLowerCase();
  const matchingTasks = tasks.filter((task) => !normalizedChatSearch || task.goal.toLocaleLowerCase().includes(normalizedChatSearch));
  const pinnedTasks = matchingTasks.filter((task) => pinnedTaskIds.includes(task.id));
  const visibleTasks = activeSurface === "threads" && selectedChatId
    ? tasks.filter((task) => task.id === selectedChatId)
      : tasks;
  const directoryPlugins = plugins.filter((plugin) => {
    const manifest = plugin.active_revision.manifest;
    const matchesSearch = `${manifest.display_name} ${manifest.id}`.toLowerCase().includes(pluginSearch.trim().toLowerCase());
    if (!matchesSearch) return false;
    if (pluginDirectoryTab === "installed") return true;
    if (pluginDirectoryTab === "personal") return plugin.active_revision.source.kind === "local_import";
    if (pluginDirectoryTab === "workspace") return plugin.active_revision.source.kind === "registry";
    return false;
  });
  const openAiOAuthConnections = providers.filter(({ connection }) =>
    connection.kind === "open_ai"
    && (connection.auth_mode === "chat_gpt_browser_oauth" || connection.auth_mode === "chat_gpt_device_code")
  );
  const signedInWithOpenAi = openAiOAuthConnections.length > 0;
  const settingsGroups: Array<{ label: string; items: Array<{ id: SettingsTab; label: string; glyph: string }> }> = [
    { label: "Personal", items: [
      { id: "general", label: "General", glyph: "⚙" }, { id: "import", label: "Import", glyph: "⇩" },
      { id: "profile", label: "Profile", glyph: "◎" }, { id: "appearance", label: "Appearance", glyph: "☼" },
      { id: "voice", label: "Voice", glyph: "♩" }, { id: "configuration", label: "Configuration", glyph: "⬡" },
      { id: "personalization", label: "Personalization", glyph: "◴" }, { id: "shortcuts", label: "Keyboard shortcuts", glyph: "⌨" },
      { id: "usage", label: "Usage & billing", glyph: "◔" }, { id: "account", label: "Account", glyph: "◉" }
    ] },
    { label: "Integrations", items: [
      { id: "plugins", label: "Plugins", glyph: "◉" }, { id: "browser", label: "Browser", glyph: "▣" },
      { id: "computer", label: "Computer use", glyph: "⌁" }
    ] },
    { label: "Coding", items: [
      { id: "hooks", label: "Hooks", glyph: "⌘" }, { id: "connections", label: "Connections", glyph: "◎" },
      { id: "git", label: "Git", glyph: "⑂" }, { id: "environments", label: "Environments", glyph: "▭" },
      { id: "worktrees", label: "Worktrees", glyph: "⇆" }
    ] },
    { label: "Archived", items: [{ id: "archived", label: "Archived chats", glyph: "▤" }] }
  ];
  const normalizedSettingsSearch = settingsSearch.trim().toLocaleLowerCase();
  const activeSurfaceTitle = activeSurface === "tasks"
    ? "New chat"
    : activeSurface === "threads"
      ? tasks.find((task) => task.id === selectedChatId)?.goal ?? "Chats"
      : activeSurface === "mcp"
        ? "Tools"
        : activeSurface === "ssh"
          ? "SSH"
          : activeSurface === "pull-requests"
            ? "Pull requests"
            : activeSurface.charAt(0).toUpperCase() + activeSurface.slice(1);

  return (
    <div className={`desktop-frame${sidebarOpen ? "" : " sidebar-collapsed"}`}>
      <header className="desktop-menubar" aria-label="Application menu">
        <button type="button" className="chrome-button sidebar-toggle" aria-label={sidebarOpen ? "Hide sidebar" : "Show sidebar"} aria-pressed={sidebarOpen} onClick={() => setSidebarOpen((current) => !current)}><span aria-hidden="true">◧</span></button>
        <button type="button" className="chrome-button" aria-label="Back" onClick={() => window.history.back()}><span aria-hidden="true">←</span></button>
        <button type="button" className="chrome-button" aria-label="Forward" onClick={() => window.history.forward()}><span aria-hidden="true">→</span></button>
        <div className="desktop-menu-list">
          {["File", "Edit", "View", "Help"].map((menu) => <div className="desktop-menu" key={menu}>
            <button type="button" aria-expanded={desktopMenu === menu} onClick={() => setDesktopMenu((current) => current === menu ? null : menu)}>{menu}</button>
            <div className={`desktop-menu-popover desktop-menu-${menu.toLocaleLowerCase()}${desktopMenu === menu ? " open" : ""}`} aria-label={`${menu} menu`}>
              {menu === "File" && <>
                <button type="button" onClick={() => void openNewWindow()}><span>New Window</span></button>
                <button type="button" onClick={() => startNewChat()}><span>New Chat</span><span className="menu-shortcut">Ctrl+N</span></button>
                <button type="button" disabled title="Temporary chats are not available yet"><span>New Temporary Chat</span><span className="menu-shortcut">Ctrl+Shift+N</span></button>
                <hr />
                <button type="button" disabled={globalFolderBusy} onClick={() => void addGlobalFolder()}><span>{globalFolderBusy ? "Opening Folder…" : "Open Folder…"}</span><span className="menu-shortcut">Ctrl+O</span></button>
                <hr />
                <button type="button" onClick={() => void closeCurrentWindow()}><span>Close</span><span className="menu-shortcut">Ctrl+W</span></button>
                <hr />
                <button type="button" disabled={!signedInWithOpenAi || busy} title={signedInWithOpenAi ? undefined : "No OpenAI subscription session is connected"} onClick={() => void logOutOfOpenAi()}><span>Log Out</span></button>
                <button type="button" onClick={() => void closeCurrentWindow(true)}><span>Quit MCG</span><span className="menu-shortcut">Ctrl+Q</span></button>
              </>}
              {menu === "Edit" && <>
                <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => runEditCommand("undo")}><span>Undo</span><span className="menu-shortcut">Ctrl+Z</span></button>
                <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => runEditCommand("redo")}><span>Redo</span><span className="menu-shortcut">Ctrl+Y</span></button>
                <hr />
                <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => runEditCommand("cut")}><span>Cut</span><span className="menu-shortcut">Ctrl+X</span></button>
                <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => runEditCommand("copy")}><span>Copy</span><span className="menu-shortcut">Ctrl+C</span></button>
                <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => runEditCommand("paste")}><span>Paste</span><span className="menu-shortcut">Ctrl+V</span></button>
                <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => runEditCommand("delete")}><span>Delete</span></button>
                <hr />
                <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => runEditCommand("selectAll")}><span>Select All</span><span className="menu-shortcut">Ctrl+A</span></button>
                <hr />
                <button type="button" onClick={() => openSettings("general")}><span>Settings…</span><span className="menu-shortcut">Ctrl+,</span></button>
              </>}
              {menu === "View" && <>
                <button type="button" onClick={() => { setSidebarOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Sidebar</span><span className="menu-shortcut">Ctrl+B</span></button>
                <button type="button" disabled title="The bottom panel is not available yet"><span>Toggle Bottom Panel</span><span className="menu-shortcut">Ctrl+J</span></button>
                <button type="button" onClick={() => { setPinnedSummaryOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Pinned Summary</span></button>
                <button type="button" disabled title="The integrated terminal is not available yet"><span>Open Terminal</span><span className="menu-shortcut">Ctrl+`</span></button>
                <button type="button" onClick={() => { setProjectTreeOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle File Tree</span><span className="menu-shortcut">Ctrl+Shift+E</span></button>
                <button type="button" disabled title="The review panel is not available yet"><span>Toggle Review Panel</span><span className="menu-shortcut">Alt+Ctrl+B</span></button>
                <hr />
                <div className="desktop-menu-submenu">
                  <button type="button" aria-haspopup="menu"><span>Browser</span><span className="menu-arrow" aria-hidden="true">›</span></button>
                  <div role="menu" aria-label="Browser submenu"><button type="button" role="menuitem" onClick={() => openSettings("browser")}><span>Browser Settings</span></button><button type="button" role="menuitem" onClick={() => navigateSurface("platform")}><span>Open Browser Workspace</span></button></div>
                </div>
                <hr />
                <button type="button" onClick={() => { setSidebarOpen(true); setDesktopMenu(null); window.requestAnimationFrame(() => chatSearchRef.current?.focus()); }}><span>Find</span><span className="menu-shortcut">Ctrl+F</span></button>
                <hr />
                <button type="button" disabled={tasks.length === 0} onClick={() => openAdjacentChat(-1)}><span>Previous Chat</span><span className="menu-shortcut">Ctrl+Shift+[</span></button>
                <button type="button" disabled={tasks.length === 0} onClick={() => openAdjacentChat(1)}><span>Next Chat</span><span className="menu-shortcut">Ctrl+Shift+]</span></button>
                <button type="button" onClick={() => { setDesktopMenu(null); window.history.back(); }}><span>Back</span><span className="menu-shortcut">Ctrl+[</span></button>
                <button type="button" onClick={() => { setDesktopMenu(null); window.history.forward(); }}><span>Forward</span><span className="menu-shortcut">Ctrl+]</span></button>
                <hr />
                <button type="button" onClick={() => setAppZoom(zoomLevel + 0.1)}><span>Zoom In</span><span className="menu-shortcut">Ctrl+Shift+=</span></button>
                <button type="button" onClick={() => setAppZoom(zoomLevel - 0.1)}><span>Zoom Out</span><span className="menu-shortcut">Ctrl+-</span></button>
                <button type="button" onClick={() => setAppZoom(1)}><span>Actual Size</span><span className="menu-shortcut">Ctrl+0</span></button>
                <hr />
                <button type="button" onClick={() => void toggleFullScreen()}><span>Toggle Full Screen</span><span className="menu-shortcut">F11</span></button>
              </>}
              {menu === "Help" && <>
                <button type="button" disabled title="MCG documentation is being prepared"><span>Documentation</span></button>
                <button type="button" onClick={() => openSettings("shortcuts")}><span>Keyboard Shortcuts</span><span className="menu-shortcut">Ctrl+/</span></button>
                <button type="button" disabled title="Release notes are not available yet"><span>What's New</span></button>
                <hr />
                <button type="button" onClick={() => openSettings("configuration")}><span>Troubleshooting</span></button>
                <button type="button" onClick={() => openSettings("configuration")}><span>System Status</span></button>
                <button type="button" disabled title="A feedback destination is not configured"><span>Send Feedback</span></button>
                <hr />
                <button type="button" onClick={() => navigateSurface("threads")}><span>Task Manager</span></button>
                <button type="button" disabled title="Performance tracing is not available yet"><span>Start Performance Trace</span></button>
                <hr />
                <button type="button" disabled={updateBusy} onClick={() => { openSettings("configuration"); void checkUpdates(); }}><span>{updateBusy ? "Checking for Updates…" : "Check for Updates…"}</span></button>
                <button type="button" onClick={() => openSettings("configuration")}><span>About MCG</span></button>
              </>}
            </div>
          </div>)}
        </div>
      </header>
      <div
        className={`app-shell ${advanced ? "advanced-mode" : "simple-mode"}`}
        data-active-surface={activeSurface}
      >
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="sidebar-heading">
          <div className="brand" aria-label="MetaCognitive Governor"><span>MCG</span></div>
          <label className="chat-search"><span aria-hidden="true">⌕</span><span className="sr-only">Search chats</span><input ref={chatSearchRef} aria-label="Search chats" value={chatSearch} onChange={(event) => setChatSearch(event.target.value)} placeholder="Search" /></label>
        </div>
        <nav className="sidebar-shortcuts" aria-label="Workspace pages">
          <button className={activeSurface === "tasks" ? "nav-item active" : "nav-item"} type="button" onClick={() => startNewChat()}><span className="shortcut-glyph" aria-hidden="true">✎</span>New chat</button>
          {[(["pull-requests", "Pull requests", "⑂"]), (["sites", "Sites", "▦"]), (["scheduler", "Scheduled", "◷"]), (["plugins", "Plugins", "◉"])].map(([surface, label, glyph]) => <a className={`nav-item${activeSurface === surface ? " active" : ""}`} href={`#${surface}`} aria-current={activeSurface === surface ? "page" : undefined} onClick={(event) => openSurface(event, surface)} key={surface}><span className="shortcut-glyph" aria-hidden="true">{glyph}</span>{label}</a>)}
        </nav>
        <div className="sidebar-library">
          {pinnedSummaryOpen && <section className="sidebar-section" aria-label="Pinned chats"><h2>Pinned</h2>{pinnedTasks.length === 0 ? <p>No pinned chats</p> : pinnedTasks.map((task) => <button className={selectedChatId === task.id ? "chat-link active" : "chat-link"} type="button" onClick={() => openChat(task.id)} key={task.id}>{task.goal}</button>)}</section>}
          {projectTreeOpen && <section className="sidebar-section project-tree" aria-label="Projects"><h2>Projects</h2>{projects.length === 0 && <p>No projects</p>}{projects.map((project) => {
            const name = project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root;
            const projectTasks = matchingTasks.filter((task) => task.project_id === project.project_id);
            return <div className="project-tree-item" key={project.project_id}><div className="project-tree-heading"><button className="project-tree-name" type="button" onClick={() => startNewChat(project)}><span aria-hidden="true">▱</span>{name}</button><button className="project-new-chat" type="button" aria-label={`New chat in ${name}`} title={`New chat in ${name}`} onClick={() => startNewChat(project)}><span aria-hidden="true">＋</span></button></div>{projectTasks.length === 0 ? <p>No chats</p> : projectTasks.map((task) => <button className={selectedChatId === task.id ? "chat-link active" : "chat-link"} type="button" onClick={() => openChat(task.id)} key={task.id}>{task.goal}</button>)}</div>;
          })}{matchingTasks.filter((task) => !task.project_id).length > 0 && <div className="project-tree-item"><button type="button" onClick={() => navigateSurface("threads")}><span aria-hidden="true">▱</span>General</button>{matchingTasks.filter((task) => !task.project_id).map((task) => <button className={selectedChatId === task.id ? "chat-link active" : "chat-link"} type="button" onClick={() => openChat(task.id)} key={task.id}>{task.goal}</button>)}</div>}</section>}
        </div>
        <div className="sidebar-account-area">
          <button className="owner-menu-button" type="button" aria-expanded={ownerMenuOpen} onClick={() => setOwnerMenuOpen((current) => !current)}><span className="owner-avatar" aria-hidden="true">{signedInWithOpenAi ? "O" : "M"}</span><span><strong>{signedInWithOpenAi ? "OpenAI account" : "Local owner"}</strong><small>{signedInWithOpenAi ? "ChatGPT subscription" : "Not signed in"}</small></span><span aria-hidden="true">⋯</span></button>
          <div className={`owner-menu${ownerMenuOpen ? " open" : ""}`} aria-label="Owner menu">
            <div className="owner-menu-identity"><span className="owner-avatar" aria-hidden="true">{signedInWithOpenAi ? "O" : "M"}</span><span><strong>{signedInWithOpenAi ? "OpenAI account" : "Local owner"}</strong><small>{signedInWithOpenAi ? "Signed in with ChatGPT" : "MCG workspace"}</small></span></div>
            <button type="button" aria-label="Usage" onClick={() => { setSettingsTab("usage"); navigateSurface("settings"); }}><span aria-hidden="true">◔</span><span>Usage</span><small>{proxyUsage ? `${proxyUsage.usage.total_tokens.toLocaleString()} tokens` : "View"}</small></button>
            <button type="button" aria-label="Settings" onClick={() => { setSettingsTab("general"); navigateSurface("settings"); }}><span aria-hidden="true">⚙</span><span>Settings</span><small>Ctrl+,</small></button>
            {signedInWithOpenAi
              ? <button className="owner-menu-session" type="button" disabled={busy} onClick={() => void logOutOfOpenAi()}><span aria-hidden="true">↪</span><span>{busy ? "Logging out…" : "Log out"}</span></button>
              : <button className="owner-menu-session" type="button" disabled={busy || Boolean(login)} onClick={() => { setOwnerMenuOpen(false); setSettingsTab("account"); navigateSurface("settings"); void startLogin("browser"); }}><span aria-hidden="true">↪</span><span>{busy ? "Opening sign-in…" : "Sign in with ChatGPT"}</span></button>}
          </div>
        </div>
      </aside>

      <main className="workspace" id="tasks">
        <header className="topbar">
          <div>
            <p className="eyebrow">METACOGNITIVE GOVERNOR</p>
            <h1>{activeSurfaceTitle}</h1>
            <p className="topbar-subtitle">Build, review, and run work with a local AI team.</p>
          </div>
          <span
            className={`status-pill${localService && !localService.ready ? " status-error" : ""}`}
            title={localService?.detail}
            role="status"
          >
            <i /> {localService?.ready ? `Local runtime · ${localService.version}` : localService ? "Local runtime needs attention" : "Starting local runtime…"}
          </span>
        </header>

        <section className="hero-card">
          <div>
            <p className="eyebrow">NEW TASK</p>
            <h2>What would you like to build?</h2>
            <p className="muted">
              Open a folder or describe a goal. MCG will choose the smallest useful
              organization, work within your permissions, and verify the result.
            </p>
          </div>
          <div className="composer">
            <label htmlFor="goal">Goal</label>
            <textarea
              ref={goalInputRef}
              id="goal"
              value={taskGoal}
              onChange={(event) => setTaskGoal(event.target.value)}
              placeholder="Build and test a responsive website..."
            />
            {advanced && <div className="advanced-fields task-advanced">
              <label>
                Constraints (one per line)
                <textarea value={taskConstraints} onChange={(event) => setTaskConstraints(event.target.value)} />
              </label>
              <label>
                Success conditions (one per line)
                <textarea
                  value={taskSuccess}
                  onChange={(event) => setTaskSuccess(event.target.value)}
                  placeholder={"FILE EXISTS dist/output.json\nSERVICE HEALTHY http://127.0.0.1:3000/health\nACCESSIBILITY CHECK http://127.0.0.1:3000/"}
                />
              </label>
              <label>
                Risk
                <select value={taskRisk} onChange={(event) => setTaskRisk(event.target.value as RiskLevel)}>
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                  <option value="critical">Critical</option>
                </select>
              </label>
              <label>
                Maximum cost (USD, optional)
                <input type="number" min="0" step="0.01" value={taskMaxCost} onChange={(event) => setTaskMaxCost(event.target.value)} />
              </label>
            </div>}
            <div className="composer-actions">
              {projects.length > 0 && <label className="project-select">
                <span className="sr-only">Project</span>
                <select
                  aria-label="Project"
                  value={selectedProject?.project_id ?? ""}
                  onChange={(event) => setSelectedProject(
                    projects.find((project) => project.project_id === event.target.value) ?? null
                  )}
                >
                  <option value="">None</option>
                  {projects.map((project) => <option value={project.project_id} key={project.project_id}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                </select>
              </label>}
              <button className="secondary icon-button folder-button" type="button" aria-label="Open folder" title="Create a project from a folder" disabled={taskBusy} onClick={() => chooseProject("task")}><span aria-hidden="true">▱</span></button>
              <button className="primary" type="button" disabled={taskBusy} onClick={addTask}>
                {taskBusy ? "Creating..." : "Create task"}
              </button>
            </div>
            {taskError && <p className="error-message" role="alert">{taskError}</p>}
            {projectError && projectErrorSurface === "task" && <p className="error-message" role="alert">{projectError}</p>}
          </div>
        </section>

        <section className="panel task-panel" aria-label="Tasks">
          <div className="section-heading">
            <div>
              <p className="eyebrow">DURABLE WORK</p>
              <h2>Tasks</h2>
            </div>
            <span className="count-badge">{visibleTasks.length} total</span>
          </div>
          <div className="task-list">
            {visibleTasks.length === 0 ? <p className="empty-state">No chats yet.</p> : visibleTasks.map((task) => (
              <article className="task-card" key={task.id}>
                <div className="task-card-main">
                  <span className={`task-state state-${task.state}`}>{task.state.replaceAll("_", " ")}</span>
                  <h3>{task.goal}</h3>
                  {task.project_id && <span className="task-project">{projects.find((project) => project.project_id === task.project_id)?.root ?? "Linked project"}</span>}
                  {advanced && <code>{task.id}</code>}
                  {taskLaunch[task.id] && <p className="task-progress" role="status">{taskLaunch[task.id]}</p>}
                  {task.state === "waiting_for_approval" && approvals.find((approval) => approval.task_id === task.id) && <div className="approval-summary">
                    <strong>Permission needed</strong>
                    <span>{approvals.find((approval) => approval.task_id === task.id)?.reason}</span>
                    {advanced && <code>{approvals.find((approval) => approval.task_id === task.id)?.resource}</code>}
                  </div>}
                  {taskEvidence(task)}
                </div>
                <div className="task-actions">
                  {task.state === "pending" && <button className="primary" type="button" disabled={taskBusy || !task.project_id} title={task.project_id ? undefined : "Select a project before starting"} onClick={() => startTask(task.id)}>Start task</button>}
                  {task.state === "blocked" && <button className="primary" type="button" disabled={taskBusy} onClick={() => retryBlockedTask(task.id)}>Retry</button>}
                  {task.state === "waiting_for_approval" && approvals.find((approval) => approval.task_id === task.id) && <button className="primary" type="button" disabled={taskBusy} onClick={() => approveAndResume(task.id, approvals.find((approval) => approval.task_id === task.id)!.id)}>Approve and continue</button>}
                  {task.state === "waiting_for_approval" && approvals.find((approval) => approval.task_id === task.id) && <button className="secondary" type="button" disabled={taskBusy} onClick={() => denyApprovalRequest(task.id, approvals.find((approval) => approval.task_id === task.id)!.id)}>Deny</button>}
                  {!(["completed", "failed", "cancelled"] as string[]).includes(task.state) && <button className="secondary" type="button" disabled={taskBusy} onClick={() => changeTaskState(task.id, "cancelled")}>Cancel</button>}
                  <button className="secondary" type="button" aria-pressed={pinnedTaskIds.includes(task.id)} onClick={() => togglePinnedTask(task.id)}>{pinnedTaskIds.includes(task.id) ? "Unpin" : "Pin"}</button>
                  <button className="secondary" type="button" disabled={taskBusy} onClick={() => toggleTaskHistory(task.id)}>{taskHistory[task.id] ? "Hide evidence" : "Evidence"}</button>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section className="panel pull-request-panel" id="pull-requests" aria-label="Pull requests">
          <div className="section-heading">
            <div>
              <p className="eyebrow">CODE REVIEW</p>
              <h2>Pull requests</h2>
              <p className="muted">Review a branch, local changes, or a commit without changing the repository.</p>
            </div>
            <span className="count-badge">{tasks.filter((task) => task.goal.startsWith(CODE_REVIEW_GOAL_PREFIX)).length} reviews</span>
          </div>
          <div className="code-review-layout">
            <div className="code-review-start-card">
              <div className="code-review-intro">
                <h3>Review code changes</h3>
                <p>Get prioritized, line-specific findings before deciding what to fix, stage, commit, or push.</p>
                <span className="read-only-note">Review is read-only. It will not modify files.</span>
              </div>
              <label>
                <span>Repository</span>
                <select value={selectedProject?.repository.is_git ? selectedProject.project_id : ""} onChange={(event) => {
                  setSelectedProject(projects.find((project) => project.project_id === event.target.value) ?? null);
                  setCodeReviewError(null);
                }}>
                  <option value="">Choose a Git project</option>
                  {projects.filter((project) => project.repository.is_git).map((project) => <option key={project.project_id} value={project.project_id}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                </select>
              </label>
              <label>
                <span>Review scope</span>
                <select value={codeReviewScope} onChange={(event) => setCodeReviewScope(event.target.value as CodeReviewScope)}>
                  <option value="branch">Branch changes</option>
                  <option value="uncommitted">Uncommitted changes</option>
                  <option value="commit">Current commit</option>
                  <option value="custom">Custom review</option>
                </select>
              </label>
              {codeReviewScope === "branch" && <label>
                <span>Base branch</span>
                <input value={codeReviewBase} onChange={(event) => setCodeReviewBase(event.target.value)} placeholder="main" />
              </label>}
              {codeReviewScope === "custom" && <label className="code-review-custom">
                <span>What should be reviewed?</span>
                <textarea value={codeReviewInstructions} onChange={(event) => setCodeReviewInstructions(event.target.value)} placeholder="Describe the range and any review focus" />
              </label>}
              <button className="primary" type="button" disabled={!selectedProject?.repository.is_git} onClick={startCodeReview}>Start review</button>
            </div>
            {codeReviewError && <p className="error-message" role="alert">{codeReviewError}</p>}
            <div className="code-review-library" aria-label="Code reviews">
              {tasks.filter((task) => task.goal.startsWith(CODE_REVIEW_GOAL_PREFIX)).length === 0
                ? <div className="site-empty-state"><strong>No code reviews yet</strong><span>Choose a Git project and review scope to begin.</span></div>
                : tasks.filter((task) => task.goal.startsWith(CODE_REVIEW_GOAL_PREFIX)).map((task) => <article className="task-card" key={task.id}>
                  <div className="task-card-main">
                    <span className={`task-state state-${task.state}`}>{task.state.replaceAll("_", " ")}</span>
                    <h3>{task.goal}</h3>
                    <small>{projects.find((project) => project.project_id === task.project_id)?.root ?? "Repository unavailable"}</small>
                  </div>
                  <button className="secondary" type="button" onClick={() => openChat(task.id)}>Open review</button>
                </article>)}
            </div>
          </div>
          {advanced && <div className="governed-approvals" aria-label="Governed approvals">
            <div className="section-heading">
              <div><p className="eyebrow">ADVANCED</p><h3>Governed approvals</h3><p className="muted">Runtime permission requests are separate from pull-request reviews.</p></div>
              <span className="count-badge">{tasks.filter((task) => task.state === "waiting_for_approval").length} pending</span>
            </div>
            {tasks.filter((task) => task.state === "waiting_for_approval").length === 0 ? <p className="empty-state">No runtime approvals need attention.</p> : <div className="task-list">{tasks.filter((task) => task.state === "waiting_for_approval").map((task) => <article className="task-card" key={task.id}><div className="task-card-main"><span className="task-state state-waiting_for_approval">permission</span><h3>{task.goal}</h3><small>{projects.find((project) => project.project_id === task.project_id)?.root ?? "General workspace"}</small></div><button className="secondary" type="button" onClick={() => openChat(task.id)}>Open chat</button></article>)}</div>}
          </div>}
          <p className="integration-note">GitHub pull request titles, comments, checks, and changed-file sync will appear here after the GitHub integration is connected. Local Git review works now.</p>
        </section>

        <section className="grid" id="projects">
          <article className="panel project-panel">
            <div className="section-heading">
              <div>
                <p className="eyebrow">PROJECTS</p>
                <h2>{advanced ? "Your workspaces" : "Choose a project"}</h2>
                {!advanced && <p className="muted">Start a chat with an AI team that can work inside the selected repository.</p>}
              </div>
              <button className="secondary" type="button" disabled={taskBusy} onClick={() => chooseProject("projects")}>Add project folder</button>
            </div>
            {projects.length === 0 ? <p className="empty-state">Open a folder to discover its language, build, tests, repository, and instructions.</p> : <div className="project-list">
              {projects.map((project) => {
                const name = project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root;
                const selected = selectedProject?.project_id === project.project_id;
                return <article className={`project-card${selected ? " selected" : ""}`} key={project.project_id}>
                  <div>
                    <h3>{name}</h3>
                    {advanced && <p>{project.languages.length > 0 ? project.languages.join(", ") : "Language not detected"}</p>}
                    {advanced && <div className="project-tags">
                      {project.build_systems.map((system) => <span key={`build-${system}`}>{system}</span>)}
                      {project.test_systems.map((system) => <span key={`test-${system}`}>{system}</span>)}
                      {project.repository.is_git && <span>{project.repository.dirty ? "Git · changes" : "Git · clean"}</span>}
                    </div>}
                    {advanced && <><code>{project.root}</code><small>{project.repository.branch ? `Branch ${project.repository.branch} · ` : ""}{project.instructions.conflicts.length} instruction conflicts</small></>}
                  </div>
                  {advanced ? <button className={selected ? "project-selected" : "secondary"} type="button" disabled={selected} onClick={() => setSelectedProject(project)}>{selected ? "Selected" : "Use project"}</button> : <button className="primary" type="button" onClick={() => startNewChat(project)}>Start chat</button>}
                </article>;
              })}
            </div>}
            {projectError && projectErrorSurface === "projects" && <p className="error-message" role="alert">{projectError}</p>}
            {advanced && selectedProject && <div className="project-intelligence" aria-label="Project intelligence and memory">
              <div className="section-heading compact-heading">
                <div>
                  <h3>Topology plan</h3>
                  <p className="muted">Read-only stages come only from explicit manifest dependencies.</p>
                </div>
                {topologyPlan && <span className="count-badge">{topologyPlan.status === "ready" ? `${topologyPlan.stages.length} stages` : topologyPlan.status.replaceAll("_", " ")}</span>}
              </div>
              {topologyPlanError ? <p className="error-message" role="alert">The topology plan could not be loaded: {topologyPlanError}</p> : !topologyPlan ? <p className="empty-state">Loading the current topology plan…</p> : topologyPlan.status === "empty" ? <p className="empty-state">No explicit deployment dependencies were discovered.</p> : topologyPlan.status === "blocked_by_cycle" ? <div className="error-message" role="alert">Dependency cycle detected. MCG will not present partial execution stages.{advanced && topologyPlan.unresolved_nodes.length > 0 && <small>{topologyPlan.unresolved_nodes.map((node) => node.label).join(", ")}</small>}</div> : <ol className="topology-plan" aria-label="Project topology plan">
                {topologyPlan.stages.map((stage) => <li key={stage.index}><strong>Stage {stage.index + 1}</strong><ul>{stage.nodes.map((node) => <li key={node.id}>{node.label}{advanced && <small>{node.kind} · {node.source}</small>}</li>)}</ul></li>)}
              </ol>}
              <div className="section-heading compact-heading">
                <div>
                  <h3>Reviewed execution</h3>
                  <p className="muted">Review an exact discovered command first. Running it later still follows your autonomy and approval settings.</p>
                </div>
                <span className="count-badge">{reviewedTopologyPlans.length} plans</span>
              </div>
              {topologyPlan?.status === "ready" && selectedProject.commands.length > 0 ? <div className="memory-composer reviewed-topology-composer" aria-label="Review topology execution plan">
                <label>Discovered command<select aria-label="Reviewed topology command" value={reviewedTopologyCommand} onChange={(event) => setReviewedTopologyCommand(event.target.value)}>
                  {selectedProject.commands.map((command, index) => <option value={String(index)} key={`${command.name}-${command.command}-${index}`}>{command.name} · {command.command}</option>)}
                </select></label>
                <label>Run after stage<select aria-label="Reviewed topology stage" value={reviewedTopologyStage} onChange={(event) => setReviewedTopologyStage(event.target.value)}>
                  {topologyPlan.stages.map((stage) => <option value={String(stage.index)} key={stage.index}>Stage {stage.index + 1} · {stage.nodes.map((node) => node.label).join(", ")}</option>)}
                </select></label>
                <label>Why should this run?<textarea aria-label="Reviewed topology reason" value={reviewedTopologyReason} onChange={(event) => setReviewedTopologyReason(event.target.value)} maxLength={1024} placeholder="Verify the project after its declared dependencies are ready." /></label>
                {advanced && <div className="reviewed-topology-metadata" aria-label="Reviewed topology authority">
                  <small><span>Snapshot</span><code>{topologyPlan.project_snapshot_id}</code></small>
                  <small><span>Topology SHA-256</span><code>{topologyPlan.sha256}</code></small>
                  <small><span>Retry identity</span><code>{reviewedTopologyPlanId}</code></small>
                </div>}
                <button className="primary" type="button" disabled={reviewedTopologyBusy || !reviewedTopologyReason.trim()} onClick={reviewTopologyExecution}>{reviewedTopologyBusy ? "Working…" : "Review this plan"}</button>
              </div> : <p className="empty-state">A ready explicit topology and a manifest-discovered command are required before MCG can create an execution plan.</p>}
              {reviewedTopologyError && <p className="error-message" role="alert">{reviewedTopologyError}</p>}
              {reviewedTopologyPlans.length > 0 && <div className="memory-list" aria-label="Reviewed topology plans">
                {reviewedTopologyPlans.map((plan) => <article className="memory-card reviewed-topology-card" key={plan.id}>
                  <div className="reviewed-plan-content">
                    <div className="reviewed-plan-heading"><strong>{plan.display_name}</strong><span className={`task-state state-${plan.state}`}>{plan.state}</span></div>
                    <p>{plan.reason}</p>
                    <div className="reviewed-plan-metadata">
                      <small>{plan.steps.length} command{plan.steps.length === 1 ? "" : "s"} · reviewed {new Date(plan.created_at).toLocaleString()}</small>
                      {plan.steps.map((step) => <small key={step.id}><strong>{step.command}</strong><span>Stage {step.stage_index + 1} · {step.state}{step.detail ? ` · ${step.detail}` : ""}</span></small>)}
                    </div>
                    {advanced && <div className="reviewed-topology-metadata reviewed-plan-authority" aria-label={`Authority for ${plan.display_name}`}>
                      <small><span>Plan identity</span><code>{plan.id}</code></small>
                      <small><span>Definition SHA-256</span><code>{plan.definition_hash}</code></small>
                      <small><span>Snapshot</span><code>{plan.project_snapshot_id}</code></small>
                      <small><span>Topology SHA-256</span><code>{plan.topology_sha256}</code></small>
                      {plan.steps.flatMap((step) => step.log_artifact_sha256).map((digest) => <small key={digest}><span>Log artifact</span><code>{digest}</code></small>)}
                    </div>}
                  </div>
                  <div className="memory-actions">
                    {(plan.state === "reviewed" || (plan.state === "waiting" && !reviewedTopologyApprovals[plan.id])) && <button className="primary" type="button" disabled={reviewedTopologyBusy} onClick={() => executeReviewedTopologyPlan(plan.id)}>{plan.state === "waiting" ? "Continue approval" : "Run reviewed plan"}</button>}
                    {plan.state === "waiting" && reviewedTopologyApprovals[plan.id] && <button className="primary" type="button" disabled={reviewedTopologyBusy} onClick={() => approveReviewedTopologyPlan(plan.id)}>Approve and run</button>}
                    {(["reviewed", "waiting"] as string[]).includes(plan.state) && <button className="secondary" type="button" disabled={reviewedTopologyBusy} onClick={() => stopReviewedTopologyPlan(plan.id)}>Cancel</button>}
                  </div>
                </article>)}
              </div>}
              <div className="section-heading compact-heading">
                <div>
                  <h3>What MCG remembers</h3>
                  <p className="muted">Important project facts stay local, versioned, and tied to their source.</p>
                </div>
                <span className="count-badge">{memoryClaims.length} current</span>
              </div>
              <div className="memory-composer">
                {advanced && <label>Namespace<input aria-label="Memory namespace" value={memoryNamespace} onChange={(event) => setMemoryNamespace(event.target.value)} /></label>}
                <label>Fact name<input aria-label="Memory fact name" value={memoryKey} onChange={(event) => setMemoryKey(event.target.value)} placeholder="package manager" /></label>
                <label>What should MCG remember?<textarea aria-label="Memory value" value={memoryDraft} onChange={(event) => setMemoryDraft(event.target.value)} placeholder="This project uses pnpm." /></label>
                {advanced && <label>Confidence<input aria-label="Memory confidence" type="number" min="0" max="1" step="0.05" value={memoryConfidence} onChange={(event) => setMemoryConfidence(event.target.value)} /></label>}
                {advanced && <label>Fresh until (optional)<input aria-label="Memory fresh until" type="datetime-local" value={memoryFreshUntil} onChange={(event) => setMemoryFreshUntil(event.target.value)} /><small>After this time, MCG automatically marks the fact stale and retains its history.</small></label>}
                <button className="primary" type="button" disabled={memoryBusy || !memoryKey.trim() || !memoryDraft.trim()} onClick={addProjectMemory}>Remember this</button>
              </div>
              {memoryError && <p className="error-message" role="alert">{memoryError}</p>}
              {memoryClaims.length === 0 ? <p className="empty-state">No managed project facts yet.</p> : <div className="memory-list">
                {memoryClaims.map((claim) => <article className="memory-card" key={claim.id}>
                  <div>
                    <strong>{claim.key}</strong>
                    <span className={`task-state state-${claim.state}`}>{claim.state}</span>
                    <p>{memoryText(claim.value)}</p>
                    <small>{claim.namespace} · confidence {Math.round(claim.confidence * 100)}% · {claim.provenance.source}</small>
                  </div>
                  <div className="memory-actions">
                    {advanced && <textarea aria-label={`Correction for ${claim.key}`} value={memoryCorrections[claim.id] ?? ""} onChange={(event) => setMemoryCorrections((current) => ({ ...current, [claim.id]: event.target.value }))} placeholder={memoryText(claim.value)} />}
                    {advanced && <button className="secondary" type="button" disabled={memoryBusy || !(memoryCorrections[claim.id] ?? "").trim()} onClick={() => correctMemory(claim)}>Correct</button>}
                    {advanced && !(["superseded", "retracted"] as string[]).includes(claim.state) && <button className="secondary" type="button" disabled={memoryBusy} onClick={() => transitionMemory(claim, "reinforce")}>Reinforce</button>}
                    {advanced && (["new", "working"] as string[]).includes(claim.state) && <button className="secondary" type="button" disabled={memoryBusy} onClick={() => transitionMemory(claim, "decay")}>Mark stale</button>}
                    {advanced && !(["superseded", "retracted"] as string[]).includes(claim.state) && <button className="link-button" type="button" disabled={memoryBusy} onClick={() => transitionMemory(claim, "retract")}>Retract</button>}
                    {claim.state === "durable" && (!claim.fresh_until || Date.parse(claim.fresh_until) > Date.now()) && projects.some((project) => project.project_id !== claim.project_id) && <button className="secondary" type="button" disabled={memoryBusy} onClick={() => beginMemoryTransfer(claim)}>Share with another project</button>}
                    {advanced && <button className="secondary" type="button" disabled={memoryBusy} onClick={() => inspectMemoryHistory(claim)}>History</button>}
                  </div>
                  {advanced && memoryHistory[claim.id] && <ol className="memory-history">
                    {memoryHistory[claim.id]!.claims.map((version) => <li key={version.id}><strong>{version.state}</strong> · {new Date(version.observed_at).toLocaleString()} · {memoryText(version.value)}</li>)}
                  </ol>}
                </article>)}
              </div>}
              {memoryTransferSource && memoryTransferPreview && <div className="memory-composer" aria-label="Review project memory transfer">
                <strong>Share “{memoryTransferSource.key}” with another project</strong>
                <p className="muted">MCG will copy this exact fact once. Later changes stay separate in each project.</p>
                <label>Target project<select aria-label="Memory transfer target project" value={memoryTransferTarget} onChange={(event) => setMemoryTransferTarget(event.target.value)}>
                  <option value="">Choose a project</option>
                  {projects.filter((project) => project.project_id !== selectedProject.project_id).map((project) => <option value={project.project_id} key={project.project_id}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                </select></label>
                <label>Why does this fact apply there?<textarea aria-label="Memory transfer reason" value={memoryTransferReason} onChange={(event) => setMemoryTransferReason(event.target.value)} maxLength={512} placeholder="Both projects use the same deployment convention." /></label>
                {advanced && <><small>Source SHA-256 <code>{memoryTransferPreview.source_claim_sha256}</code></small><small>Retry identity <code>{memoryTransferId}</code></small></>}
                <div className="composer-actions">
                  <button className="secondary" type="button" disabled={memoryBusy} onClick={() => { setMemoryTransferSource(null); setMemoryTransferPreview(null); }}>Cancel</button>
                  <button className="primary" type="button" disabled={memoryBusy || !memoryTransferTarget || !memoryTransferReason.trim()} onClick={confirmMemoryTransfer}>Copy reviewed fact</button>
                </div>
              </div>}
              {advanced && memoryTransfers.length > 0 && <details className="advanced-details">
                <summary>Cross-project transfer history ({memoryTransfers.length})</summary>
                <ol className="memory-history">
                  {memoryTransfers.map((transfer) => <li key={transfer.id}><strong>{transfer.source_project_id === selectedProject.project_id ? "Sent" : "Received"}</strong> · {new Date(transfer.created_at).toLocaleString()} · {transfer.reason}<small><code>{transfer.id}</code> · SHA-256 {transfer.source_claim_sha256}</small></li>)}
                </ol>
              </details>}
              {advanced && <details className="advanced-details">
                <summary>World model and managed MEMORY.md</summary>
                <p>{selectedProject.world?.nodes.length ?? 0} nodes · {selectedProject.world?.relationships.length ?? 0} relationships · {selectedProject.commands?.length ?? 0} discovered commands</p>
                <ul className="world-model" aria-label="Project world model">
                  {(selectedProject.world?.nodes ?? []).map((node) => <li key={node.id}><strong>{node.kind}</strong> {node.label}<small>{node.source}</small></li>)}
                  {(selectedProject.world?.relationships ?? []).map((edge, index) => <li key={`${edge.from}-${edge.relation}-${edge.to}-${index}`}><code>{worldNodeLabel(selectedProject, edge.from)} → {edge.relation.replaceAll("_", " ")} → {worldNodeLabel(selectedProject, edge.to)}</code><small>{edge.source}</small></li>)}
                </ul>
                <pre className="managed-memory">{managedMemory}</pre>
              </details>}
            </div>}
          </article>

          <article className="panel runtime-panel">
            {advanced ? (
              <>
                <p className="eyebrow">ADVANCED</p>
                <h2>Runtime status</h2>
                <dl>
                  <div><dt>Daemon API</dt><dd>Authenticated local loopback</dd></div>
                  <div><dt>Startup</dt><dd>{localService?.detail ?? "Checking packaged service…"}</dd></div>
                  <div><dt>Autonomy</dt><dd>Autonomous</dd></div>
                  <div><dt>Organization</dt><dd>Foundation</dd></div>
                  <div><dt>Provider</dt><dd>{providerStatus}</dd></div>
                </dl>
              </>
            ) : (
              <>
                <p className="eyebrow">SIMPLE MODE</p>
                <h2>Complexity stays out of the way</h2>
                <p className="muted">System details are hidden in simple mode.</p>
              </>
            )}
          </article>
        </section>

        <section className="panel scheduler-panel" id="improvement" aria-label="Measured improvement">
          <div className="section-heading">
            <div>
              <p className="eyebrow">IMPROVEMENT</p>
              <h2>Measured changes, safe rollback</h2>
              <p className="muted">Candidates stay inactive until benchmark evidence supports promotion. Shadow work can evaluate, but cannot change a real task.</p>
            </div>
            <span className="count-badge">{experiments.filter((item) => item.state === "running").length} running</span>
          </div>
          <div className="scheduler-layout" aria-label="Agent organizations">
            <div className="schedule-composer">
              <h3>Agent organizations</h3>
              <p className="muted">Create, duplicate, or import a safe organization before specializing it through measured experiments.</p>
              <label>
                New organization name
                <input value={organizationName} onChange={(event) => setOrganizationName(event.target.value)} placeholder="Website specialists" />
              </label>
              <button className="secondary" type="button" disabled={organizationBusy} onClick={addOrganization}>
                {organizationBusy ? "Saving..." : "Create default organization"}
              </button>
              {advanced && <div className="advanced-fields">
                <label>
                  Inherit current revision from
                  <select value={organizationBaseId} onChange={(event) => setOrganizationBaseId(event.target.value)}>
                    <option value="">Choose a base organization</option>
                    {organizations.map((organization) => <option key={organization.organization_id} value={organization.organization_id}>{organization.name} · revision {organization.revision}</option>)}
                  </select>
                </label>
                <button className="secondary" type="button" disabled={organizationBusy || !organizationBaseId || !organizationName.trim()} onClick={addInheritedOrganization}>Create inherited organization</button>
                <small>Inheritance pins the exact base revision. Later changes to the base cannot silently alter this organization.</small>
                <label>Portable organization JSON<textarea aria-label="Portable organization JSON" value={portableOrganizationJson} onChange={(event) => setPortableOrganizationJson(event.target.value)} placeholder="Export a revision or paste a portable MCG organization" /></label>
                <button className="secondary" type="button" disabled={organizationBusy || !portableOrganizationJson.trim()} onClick={applyPortableOrganization}>Import as new organization</button>
                <small>Retry identity: <code>{organizationImportId}</code>. It changes only after a confirmed import.</small>
                <div className="inline-fields">
                  <label>Compare from<select value={organizationCompareLeft} onChange={(event) => setOrganizationCompareLeft(event.target.value)}><option value="">Choose revision</option>{organizationRevisionChoices.map((organization) => <option key={`left-${organization.revision_id}`} value={organization.revision_id}>{organization.name} · r{organization.revision}</option>)}</select></label>
                  <label>Compare to<select value={organizationCompareRight} onChange={(event) => setOrganizationCompareRight(event.target.value)}><option value="">Choose revision</option>{organizationRevisionChoices.map((organization) => <option key={`right-${organization.revision_id}`} value={organization.revision_id}>{organization.name} · r{organization.revision}</option>)}</select></label>
                </div>
                <button className="secondary" type="button" disabled={organizationBusy || !organizationCompareLeft || !organizationCompareRight} onClick={compareOrganizationRevisions}>Compare portable definitions</button>
                {organizationComparison && <p className="muted" role="status">{organizationComparison.equivalent ? "Definitions are equivalent." : `Different: ${organizationComparison.roles_added.length + organizationComparison.roles_removed.length + organizationComparison.roles_changed.length} role changes, ${organizationComparison.relationships_added.length + organizationComparison.relationships_removed.length} relationship changes.`}</p>}
              </div>}
              {organizationError && <p className="error-message" role="alert">{organizationError}</p>}
            </div>
            <div className="schedule-list">
              {organizations.length === 0 ? <p className="empty-state">No reusable organizations yet.</p> : organizations.map((organization) => <article className="schedule-card organization-card" key={organization.organization_id}>
                <div className="organization-summary">
                  <strong>{organization.name}</strong>
                  <span>{organization.roles.length} roles · revision {organization.revision}</span>
                  <small>{organization.inherited_from_revision_id ? "Inherited from a pinned revision" : "Root organization"}</small>
                  {advanced && <><code>{organization.organization_id}</code><code>revision {organization.revision_id}</code>{organization.inherited_from_revision_id && <code>base {organization.inherited_from_revision_id}</code>}</>}
                </div>
                <div className="organization-actions">
                  <button className="secondary" type="button" disabled={organizationBusy} onClick={() => duplicateOrganization(organization)}>Duplicate</button>
                  {advanced && <button className="secondary" type="button" disabled={organizationBusy} onClick={() => showPortableOrganization(organization)}>Show portable JSON</button>}
                  {advanced && <button className="secondary" type="button" disabled={organizationBusy} onClick={() => inspectOrganizationHistory(organization)}>Revision history</button>}
                  <button className="secondary" type="button" onClick={() => setExperimentOrganizationId(organization.organization_id)}>Use for experiments</button>
                </div>
                {advanced && organizationHistory[organization.organization_id] && <ol className="organization-history" aria-label={`${organization.name} revision history`}>{organizationHistory[organization.organization_id]!.map((revision) => <li key={revision.revision_id}>Revision {revision.revision} · <code>{revision.revision_id}</code></li>)}</ol>}
              </article>)}
            </div>
          </div>
          <div className="scheduler-layout organization-sharing" aria-label="Signed organization sharing">
            <div className="schedule-composer">
              <h3>Signed organization sharing</h3>
              <p className="muted">Verification creates a review only. Import remains a separate owner decision and never inherits publisher authority.</p>
              <label>
                Signed organization package
                <textarea aria-label="Signed organization package" value={organizationShareJson} onChange={(event) => setOrganizationShareJson(event.target.value)} placeholder="Paste a signed MCG organization package" />
              </label>
              <button className="secondary" type="button" disabled={organizationShareBusy || !organizationShareJson.trim()} onClick={reviewOrganizationShare}>
                {organizationShareBusy ? "Checking..." : "Verify for review"}
              </button>
              <small>Verification identity: <code>{organizationShareReviewId}</code>. Verification alone cannot import or activate the organization.</small>
              {advanced && <div className="advanced-fields" aria-label="Organization publisher trust controls">
                <h4>Publisher trust</h4>
                <label>Publisher identity<input value={publisherId} onChange={(event) => setPublisherId(event.target.value)} placeholder="example-publisher" /></label>
                <label>Display name<input value={publisherDisplayName} onChange={(event) => setPublisherDisplayName(event.target.value)} placeholder="Example publisher" /></label>
                <label>Ed25519 public key<input value={publisherPublicKey} onChange={(event) => setPublisherPublicKey(event.target.value)} placeholder="Canonical base64 public key" /></label>
                <label>Trusted HTTPS registries<textarea aria-label="Trusted organization registries" value={publisherRegistries} onChange={(event) => setPublisherRegistries(event.target.value)} placeholder="https://organizations.example.com" /></label>
                <label>Owner trust or revocation reason<input value={publisherReason} onChange={(event) => setPublisherReason(event.target.value)} placeholder="Why this exact authority and scope are acceptable" /></label>
                <button className="secondary" type="button" disabled={organizationShareBusy} onClick={saveOrganizationPublisher}>Trust or revise publisher</button>
                {organizationPublishers.length === 0 ? <p className="empty-state">No trusted organization publishers.</p> : organizationPublishers.map((publisher) => <article className="schedule-card organization-share-card" key={publisher.id}>
                  <strong>{publisher.display_name}</strong>
                  <span>{publisher.state} · revision {publisher.revision}</span>
                  <code>{publisher.id}</code>
                  {publisher.registries.map((registry) => <small key={registry}>{registry}</small>)}
                  <code>key {publisher.public_key_base64}</code>
                  {publisher.state === "trusted" && <button className="secondary" type="button" disabled={organizationShareBusy || !publisherReason.trim()} onClick={() => revokeSharedPublisher(publisher)}>Revoke permanently</button>}
                </article>)}
              </div>}
              {organizationShareError && <p className="error-message" role="alert">{organizationShareError}</p>}
            </div>
            <div className="schedule-list" aria-label="Organization share reviews">
              {organizationShareReviews.length === 0 ? <p className="empty-state">No signed organizations awaiting review.</p> : organizationShareReviews.map((review) => <article className="schedule-card organization-share-card" key={review.id}>
                <div className="organization-summary">
                  <strong>{review.package.portable.definition.name}</strong>
                  <span>{review.evidence.publisher} · {review.state}</span>
                  <small>{review.evidence.registry}/{review.evidence.package}</small>
                  <small>Publisher key digest</small>
                  <code>{review.evidence.publisher_key_sha256}</code>
                  <small>Verified package digest</small>
                  <code>{review.evidence.package_sha256}</code>
                  {advanced && <><code>review {review.id}</code><code>portable {review.evidence.portable_sha256}</code><code>signature {review.evidence.signature_sha256}</code></>}
                </div>
                {review.state === "pending" ? <div className="advanced-fields organization-share-decision">
                  <label>Imported name (optional)<input value={organizationShareNames[review.id] ?? ""} onChange={(event) => setOrganizationShareNames((current) => ({ ...current, [review.id]: event.target.value }))} placeholder={review.package.portable.definition.name} /></label>
                  <label>Owner decision reason<input value={organizationShareReasons[review.id] ?? ""} onChange={(event) => setOrganizationShareReasons((current) => ({ ...current, [review.id]: event.target.value }))} placeholder="Why this verified organization should or should not be imported" /></label>
                  <div className="organization-actions">
                    <button className="primary" type="button" disabled={organizationShareBusy || !(organizationShareReasons[review.id]?.trim())} onClick={() => importOrganizationShare(review)}>Import verified organization</button>
                    <button className="secondary" type="button" disabled={organizationShareBusy || !(organizationShareReasons[review.id]?.trim())} onClick={() => rejectOrganizationShare(review)}>Reject share</button>
                  </div>
                  {advanced && <small>Import retry identity: <code>{organizationShareImportIds[review.id] ?? "created on first import attempt"}</code></small>}
                </div> : <p className="muted">{review.decision_reason}{review.organization_id ? ` · local organization ${review.organization_id}` : ""}</p>}
              </article>)}
            </div>
          </div>
          <div className="scheduler-layout">
            <div className="schedule-composer">
              <label>Organization for experiments<select value={experimentOrganizationId} onChange={(event) => setExperimentOrganizationId(event.target.value)}><option value="">Choose an organization</option>{organizations.map((organization) => <option key={organization.organization_id} value={organization.organization_id}>{organization.name}</option>)}</select></label>
              <button className="primary" type="button" disabled={experimentBusy} onClick={refreshExperiments}>{experimentBusy ? "Loading..." : "Load experiments"}</button>
              <div className="evidence-overview">
                <div><span>Observations</span><strong>{observations.length}</strong></div>
                <div><span>Experiments</span><strong>{experiments.length}</strong></div>
                <div><span>Promoted</span><strong>{experiments.filter((item) => item.state === "promoted").length}</strong></div>
                <div><span>Reverted</span><strong>{experiments.filter((item) => item.state === "reverted").length}</strong></div>
              </div>
              {observations.slice(-3).reverse().map((observation) => <article className="schedule-card" key={observation.id}>
                <div><strong>{observation.signal.replaceAll("_", " ")}</strong><span>{observation.loop_kind} loop · {(observation.severity_basis_points / 100).toFixed(0)}%</span></div>
                <p>{observation.summary}</p>
              </article>)}
              {advanced && <div className="advanced-fields">
                <label>Advanced action<select value={experimentAction} onChange={(event) => setExperimentAction(event.target.value as typeof experimentAction)}>
                  <option value="observation">Record observation</option>
                  <option value="candidate">Record candidate revision</option>
                  <option value="experiment">Create experiment</option>
                  <option value="shadow">Record shadow receipt</option>
                  <option value="benchmark">Record benchmark</option>
                  <option value="decision">Promote, reject, or revert</option>
                </select></label>
                {(experimentAction === "shadow" || experimentAction === "benchmark" || experimentAction === "decision") && <label>Experiment ID<input value={experimentTargetId} onChange={(event) => setExperimentTargetId(event.target.value)} placeholder="Experiment UUID" /></label>}
                <label>Versioned JSON<textarea aria-label="Measured improvement JSON" value={experimentJson} onChange={(event) => setExperimentJson(event.target.value)} placeholder="Paste the complete versioned contract" /></label>
                <button className="secondary" type="button" disabled={experimentBusy || !experimentJson.trim()} onClick={submitExperimentJson}>Apply measured action</button>
              </div>}
              {experimentError && <p className="error-message" role="alert">{experimentError}</p>}
            </div>
            <div className="schedule-list">
              {experiments.length === 0 ? <p className="empty-state">Load an organization to inspect its measured experiments.</p> : experiments.map((experiment) => {
                const evidence = experimentEvidence[experiment.id] ?? { shadow: 0, benchmarks: 0, decisions: 0 };
                const next = experiment.state === "draft" ? "ready" : experiment.state === "ready" ? "running" : experiment.state === "running" ? "awaiting_decision" : null;
                return <article className="schedule-card experiment-card" key={experiment.id}>
                  <div><strong>{experiment.hypothesis}</strong><span>{experiment.mode} · {experiment.state.replaceAll("_", " ")}</span></div>
                  <p>{experiment.task_classes.join(", ")}</p>
                  <small>{evidence.shadow} shadow · {evidence.benchmarks}/{experiment.required_benchmarks} benchmarks · {evidence.decisions} decisions</small>
                  <small>Active baseline remains protected until an atomic measured decision.</small>
                  {advanced && <><code>{experiment.id}</code><code>candidate {experiment.candidate_revision_id}</code>{next && <button className="secondary" type="button" disabled={experimentBusy} onClick={() => advanceExperiment(experiment, next)}>Move to {next.replaceAll("_", " ")}</button>}</>}
                </article>;
              })}
            </div>
          </div>
        </section>

        <section className="panel scheduler-panel" id="sites" aria-label="Sites">
          {!advanced ? <>
            <div className="section-heading sites-heading">
              <div>
                <p className="eyebrow">SITES</p>
                <h2>Sites</h2>
                <p className="muted">Create a website from an idea or an existing project, review it, and keep refining it in chat.</p>
              </div>
              <span className="count-badge">{sites.filter((site) => site.state !== "archived").length}</span>
            </div>
            <div className="sites-simple-layout">
              <div className="site-start-card">
                <div>
                  <h3>Build a new site</h3>
                  <p>Describe what you want in chat. You can start fresh or use one of your project folders.</p>
                </div>
                <label>
                  <span>Starting point</span>
                  <select value={selectedProject?.project_id ?? ""} onChange={(event) => setSelectedProject(projects.find((project) => project.project_id === event.target.value) ?? null)}>
                    <option value="">Start from an idea</option>
                    {projects.map((project) => <option key={project.project_id} value={project.project_id}>Project: {project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                  </select>
                </label>
                <button className="primary" type="button" onClick={() => startSiteChat()}>New site</button>
              </div>
              <div className="site-library" aria-label="Your sites">
                {sites.filter((site) => site.state !== "archived").length === 0
                  ? <div className="site-empty-state"><strong>No sites yet</strong><span>Build one from a prompt or an existing project.</span></div>
                  : sites.filter((site) => site.state !== "archived").map((site) => {
                    const project = projects.find((candidate) => candidate.project_id === site.project_id);
                    const deployment = siteDeployments[site.id]?.deployments[0];
                    return <article className="site-library-card" key={site.id}>
                      <div className="site-preview" aria-hidden="true">▧</div>
                      <div className="site-library-copy">
                        <h3>{site.display_name}</h3>
                        <a href={site.public_url} target="_blank" rel="noreferrer">{site.public_url}</a>
                        <small>{project ? project.root.split(/[\\/]/).filter(Boolean).at(-1) : "Project unavailable"}{deployment ? ` · ${deployment.state}` : " · Ready to refine"}</small>
                      </div>
                      <div className="site-library-actions">
                        <a className="secondary button-link" href={site.public_url} target="_blank" rel="noreferrer">Open site</a>
                        <button className="secondary" type="button" onClick={() => continueSiteChat(site)}>Continue editing</button>
                      </div>
                    </article>;
                  })}
              </div>
            </div>
          </> : <>
          <div className="section-heading">
            <div>
              <p className="eyebrow">SITES</p>
              <h2>Web properties</h2>
              <p className="muted">Register a project-backed Site, keep durable health evidence, and deploy only with explicit approval.</p>
            </div>
            <span className="count-badge">{sites.filter((site) => site.state === "active").length} active</span>
          </div>
          <div className="scheduler-layout">
            <div className="schedule-composer">
              <label>
                Site project
                <select value={selectedProject?.project_id ?? ""} onChange={(event) => setSelectedProject(projects.find((project) => project.project_id === event.target.value) ?? null)}>
                  <option value="">Choose a project</option>
                  {projects.map((project) => <option key={project.project_id} value={project.project_id}>Site: {project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                </select>
              </label>
              <label>Site name<input value={siteName} onChange={(event) => setSiteName(event.target.value)} placeholder="Production website" /></label>
              <label>Public URL<input value={sitePublicUrl} onChange={(event) => setSitePublicUrl(event.target.value)} placeholder="https://example.com" /></label>
              <label>Health URL<input value={siteHealthUrl} onChange={(event) => setSiteHealthUrl(event.target.value)} placeholder="https://example.com/health" /></label>
              <label>Hosting provider<input value={siteProvider} onChange={(event) => setSiteProvider(event.target.value)} placeholder="Cloudflare, Vercel, local..." /></label>
              {advanced && <div className="advanced-fields">
                <label>Deployment executable<input value={siteDeployExecutable} onChange={(event) => setSiteDeployExecutable(event.target.value)} placeholder="npm" /></label>
                <label>Deployment arguments (one per line)<textarea value={siteDeployArguments} onChange={(event) => setSiteDeployArguments(event.target.value)} placeholder={"run\ndeploy"} /></label>
                <code>Retry identity: {siteRegistrationId}</code>
              </div>}
              <button className="primary" type="button" disabled={siteBusy} onClick={addSite}>{siteBusy ? "Saving..." : "Register Site"}</button>
              {siteError && <p className="error-message" role="alert">{siteError}</p>}
            </div>
            <div className="schedule-list">
              {sites.length === 0 ? <p className="empty-state">No Sites registered yet.</p> : sites.map((site) => {
                const health = siteHealth[site.id]?.checks[0];
                const deployment = siteDeployments[site.id]?.deployments[0];
                const monitor = siteMonitors[site.id];
                const pendingDeployment = siteDeploymentIds[site.id]
                  ? approvals.find((approval) => approval.action === "sites.deploy")
                  : undefined;
                return <article className="schedule-card" key={site.id}>
                  <div>
                    <span className={`task-state state-${site.state}`}>{site.state.replaceAll("_", " ")}</span>
                    <h3>{site.display_name}</h3>
                    <a href={site.public_url} target="_blank" rel="noreferrer">{site.public_url}</a>
                    <small>{site.environment} · {site.hosting_provider}</small>
                    {health && <small>Health: {health.state}{health.status_code ? ` · HTTP ${health.status_code}` : ""}</small>}
                    {monitor && <small>Automatic checks: every {monitor.interval_seconds} seconds{monitor.last_state ? ` · ${monitor.last_state}` : ""}</small>}
                    {deployment && <small>Last deployment: {deployment.state} · revision {deployment.site_revision}</small>}
                    {advanced && <><code>{site.id}</code><code>revision {site.revision} · {site.definition_hash.slice(0, 12)}</code></>}
                  </div>
                  <div className="task-actions">
                    <button className="primary" type="button" disabled={siteBusy} onClick={() => runSiteHealth(site.id)}>Check health</button>
                    {!monitor && <>
                      {advanced && <label>Monitor interval (seconds)<input aria-label={`Monitor interval for ${site.display_name}`} value={siteMonitorIntervals[site.id] ?? "300"} onChange={(event) => setSiteMonitorIntervals((current) => ({ ...current, [site.id]: event.target.value }))} /></label>}
                      <button className="secondary" type="button" disabled={siteBusy} onClick={() => enableSiteMonitoring(site.id)}>Enable automatic monitoring</button>
                    </>}
                    {site.deployment_command && !pendingDeployment && <button className="secondary" type="button" disabled={siteBusy} onClick={() => runSiteDeployment(site.id)}>Deploy</button>}
                    {pendingDeployment && <button className="primary" type="button" disabled={siteBusy} onClick={() => approveSiteDeployment(site.id, pendingDeployment.id)}>Approve and deploy</button>}
                    <button className="secondary" type="button" disabled={siteBusy} onClick={() => inspectSite(site.id)}>History</button>
                    {site.state === "active" && <button className="secondary" type="button" disabled={siteBusy} onClick={() => changeSiteState(site, "monitoring_paused")}>Pause monitoring</button>}
                    {site.state === "monitoring_paused" && <button className="secondary" type="button" disabled={siteBusy} onClick={() => changeSiteState(site, "active")}>Resume monitoring</button>}
                    {site.state !== "archived" && advanced && <button className="link-button" type="button" disabled={siteBusy} onClick={() => changeSiteState(site, "archived")}>Archive</button>}
                  </div>
                  {advanced && (siteHealth[site.id] || siteDeployments[site.id]) && <div className="evidence-details">
                    <span>{siteHealth[site.id]?.checks.length ?? 0} health checks</span>
                    <span>{siteDeployments[site.id]?.deployments.length ?? 0} deployments</span>
                  </div>}
                </article>;
              })}
            </div>
          </div>
          </>}
        </section>

        <section className="panel scheduler-panel" id="ssh" aria-label="SSH hosts">
          <div className="section-heading">
            <div>
              <p className="eyebrow">SSH</p>
              <h2>Remote Linux hosts</h2>
              <p className="muted">Connect outbound with your SSH agent and an explicitly pinned host key. MCG never opens a remote control listener.</p>
            </div>
            <span className="count-badge">{sshHosts.filter((host) => host.state === "active").length} active</span>
          </div>
          <div className="scheduler-layout">
            <div className="schedule-composer">
              <label>Host name<input value={sshName} onChange={(event) => setSshName(event.target.value)} placeholder="Build VM" /></label>
              <label>Hostname or IP<input value={sshHostname} onChange={(event) => setSshHostname(event.target.value)} placeholder="192.168.50.109" /></label>
              <label>SSH username<input value={sshUsername} onChange={(event) => setSshUsername(event.target.value)} placeholder="user" /></label>
              <label>Pinned known_hosts line<textarea value={sshKnownHosts} onChange={(event) => setSshKnownHosts(event.target.value)} placeholder="host ssh-ed25519 AAAA..." /></label>
              <label>Verified host fingerprint<input value={sshFingerprint} onChange={(event) => setSshFingerprint(event.target.value)} placeholder="SHA256:..." /></label>
              {advanced && <label>Routing capabilities<input value={sshCapabilities} onChange={(event) => setSshCapabilities(event.target.value)} placeholder="os:linux, arch:x86_64, gpu:nvidia" /></label>}
              {advanced && <code>Retry identity: {sshHostId}</code>}
              <button className="primary" type="button" disabled={sshBusy} onClick={addSshHost}>{sshBusy ? "Saving..." : "Register pinned host"}</button>
              {sshError && <p className="error-message" role="alert">{sshError}</p>}
            </div>
            <div className="schedule-list">
              {sshHosts.length === 0 ? <p className="empty-state">No SSH hosts registered yet.</p> : sshHosts.map((host) => {
                const latest = sshOperations[host.id]?.operations[0];
                return <article className="schedule-card" key={host.id}>
                  <div>
                    <span className={`task-state state-${host.state}`}>{host.state}</span>
                    <h3>{host.display_name}</h3>
                    <small>{host.username}@{host.hostname}:{host.port}</small>
                    {latest && <small>Last operation: {latest.kind.kind} · {latest.state}</small>}
                    {latest && latest.state !== "succeeded" && <small role="status">{latest.detail || "The SSH operation did not complete successfully."}{latest.exit_code !== undefined ? ` · exit ${latest.exit_code}` : ""}</small>}
                    {advanced && <><code>{host.id}</code><code>revision {host.revision} · {host.host_key_sha256}</code><small>{host.capabilities.length ? `Capabilities: ${host.capabilities.join(", ")}` : "No routing capabilities declared"}</small></>}
                  </div>
                  <div className="task-actions">
                    <button className="primary" type="button" disabled={sshBusy || host.state !== "active"} onClick={() => executeSsh(host, { kind: "inspect" })}>Inspect host</button>
                    {advanced && <>
                      <label>Typed operation JSON<textarea aria-label={`Typed SSH operation for ${host.display_name}`} value={sshOperationJson[host.id] ?? ""} onChange={(event) => setSshOperationJson((current) => ({ ...current, [host.id]: event.target.value }))} placeholder={'{"kind":"execute","executable":"uname","arguments":["-a"]}'} /></label>
                      <button className="secondary" type="button" disabled={sshBusy || !sshOperationJson[host.id]?.trim()} onClick={() => executeSsh(host)}>Run typed operation</button>
                    </>}
                  </div>
                  {advanced && sshOperations[host.id] && <div className="evidence-details">
                    <span>{sshOperations[host.id]?.operations.length} retained operations</span>
                    {latest && <span>{latest.artifact_sha256.length} evidence artifacts · operation {latest.id}</span>}
                  </div>}
                </article>;
              })}
            </div>
          </div>
          <div className="workflow-area" role="region" aria-label="SSH workflows">
            <div className="section-heading compact-heading">
              <div>
                <p className="eyebrow">MULTI-HOST WORKFLOWS</p>
                <h3>Coordinate remote work safely</h3>
                <p className="muted">Select hosts in execution order. Each step is pinned to the current host revision and survives daemon restarts.</p>
              </div>
              <span className="count-badge">{sshWorkflows.length} retained</span>
            </div>
            <div className="scheduler-layout">
              <div className="schedule-composer">
                <label>Workflow name<input value={sshWorkflowName} onChange={(event) => setSshWorkflowName(event.target.value)} placeholder="Build, test, and inspect" /></label>
                <fieldset>
                  <legend>Hosts in workflow order</legend>
                  {sshHosts.filter((host) => host.state === "active").map((host) => <label className="checkbox-row" key={host.id}>
                    <input type="checkbox" checked={sshWorkflowHosts.includes(host.id)} onChange={() => toggleSshWorkflowHost(host.id)} />
                    {host.display_name} <small>revision {host.revision}</small>
                  </label>)}
                  {sshHosts.every((host) => host.state !== "active") && <p className="empty-state">Register an active SSH host first.</p>}
                </fieldset>
                {advanced && <label>Advanced step graph JSON<textarea value={sshWorkflowStepsJson} onChange={(event) => setSshWorkflowStepsJson(event.target.value)} placeholder={'[{"id":"UUID","name":"GPU test","required_capabilities":["os:linux","gpu:nvidia"],"dependencies":[],"operation":{"kind":"execute","executable":"cargo","arguments":["test"]},"timeout_seconds":600}]'} /></label>}
                {advanced && <code>Workflow retry identity: {sshWorkflowId}</code>}
                <button className="primary" type="button" disabled={sshBusy} onClick={addSshWorkflow}>Create durable workflow</button>
              </div>
              <div className="schedule-list">
                {sshWorkflows.length === 0 ? <p className="empty-state">No multi-host workflows yet.</p> : sshWorkflows.map((workflow) => <article className="schedule-card" key={workflow.id}>
                  <div>
                    <span className={`task-state state-${workflow.state}`}>{workflow.state}</span>
                    <h3>{workflow.display_name}</h3>
                    <small>{workflow.steps.length} dependency-ordered step{workflow.steps.length === 1 ? "" : "s"}</small>
                    {workflow.steps.map((step) => <small key={step.id}>{step.name} · {step.state}{step.detail ? ` · ${step.detail}` : ""}</small>)}
                    {advanced && <><code>{workflow.id}</code><code>{workflow.definition_hash}</code></>}
                  </div>
                  <div className="task-actions">
                    {!['succeeded', 'failed', 'uncertain', 'cancelled'].includes(workflow.state) && <button className="primary" type="button" disabled={sshBusy} onClick={() => mutateSshWorkflow(workflow, "run")}>{workflow.state === "planned" ? "Run workflow" : "Resume workflow"}</button>}
                    {!['succeeded', 'failed', 'uncertain', 'cancelled'].includes(workflow.state) && <button className="secondary" type="button" disabled={sshBusy} onClick={() => mutateSshWorkflow(workflow, "cancel")}>Cancel</button>}
                  </div>
                </article>)}
              </div>
            </div>
          </div>
        </section>

        <section className="panel scheduler-panel platform-panel" id="platform" aria-label="Computer and browser sessions">
          <div className="section-heading">
            <div>
              <p className="eyebrow">COMPUTER &amp; BROWSER</p>
              <h2>Local execution sessions</h2>
              <p className="muted">Reserve durable local sessions now. MCG reports them as requested until a real executor is attached.</p>
            </div>
            <span className="count-badge">{platformSessions.filter((session) => ["requested", "active", "handed_off"].includes(session.state)).length} open</span>
          </div>
          {platformCapabilities && <div className="evidence-overview" aria-label="Platform capabilities">
            {platformCapabilities.capabilities
              .filter((capability) => advanced || ["browser_automation", "computer_use", "native_desktop"].includes(capability.kind))
              .map((capability) => <div key={capability.kind}>
                <span>{capability.kind.replaceAll("_", " ")}</span>
                <strong>{capability.availability}</strong>
                {advanced && <small>{capability.detail}</small>}
              </div>)}
          </div>}
          {advanced && platformCapabilities && <p className="muted">Host: {platformCapabilities.os} / {platformCapabilities.architecture} · observed {new Date(platformCapabilities.observed_at).toLocaleString()}</p>}
          <div className="scheduler-layout">
            <div className="schedule-composer">
              <label>
                Session type
                <select value={platformKind} onChange={(event) => setPlatformKind(event.target.value as PlatformSessionKind)}>
                  <option value="browser">Browser</option>
                  <option value="computer_use">Computer use</option>
                </select>
              </label>
              <label>
                Name
                <input value={platformName} onChange={(event) => setPlatformName(event.target.value)} placeholder="Research checkout options" />
              </label>
              {platformKind === "browser" && <label>
                Allowed sites {advanced ? "(required, one origin per line)" : "(required)"}
                <textarea value={platformOrigins} onChange={(event) => setPlatformOrigins(event.target.value)} placeholder="https://example.com" />
              </label>}
              {platformKind === "computer_use" && advanced && <label className="checkbox-row">
                <input type="checkbox" checked={platformComputerScreenOnly} onChange={(event) => setPlatformComputerScreenOnly(event.target.checked)} />
                Screen capture only (do not grant pointer or keyboard input)
              </label>}
              {platformKind === "computer_use" && advanced && <fieldset className="computer-region-fields">
                <legend>Target region (optional)</legend>
                <div className="computer-region-grid">
                  {(["x", "y", "width", "height"] as const).map((field) => <label key={field}>
                    {field[0].toUpperCase() + field.slice(1)}
                    <input
                      inputMode="numeric"
                      aria-label={`Target region ${field}`}
                      value={platformComputerRegion[field]}
                      onChange={(event) => setPlatformComputerRegion((current) => ({ ...current, [field]: event.target.value }))}
                      placeholder={field === "x" || field === "y" ? "0" : "Entire display"}
                    />
                  </label>)}
                </div>
                <small>Leave every field blank for the entire primary display. A smaller, static region is recommended before granting input because its pixels must remain unchanged between observation and action.</small>
              </fieldset>}
              {platformKind === "computer_use" && <p className="muted">
                {platformComputerScreenOnly ? "MCG can see the primary display but cannot interact." : "MCG can see and use the primary display. Every input batch must match the latest captured pixels."}
              </p>}
              <p className="muted">Reservation is local and restart-safe. It will not be labeled active until MCG has attached a verified executor.</p>
              <button className="primary" type="button" disabled={platformBusy} onClick={reservePlatformSession}>
                {platformBusy ? "Saving..." : "Reserve session"}
              </button>
              {platformError && <p className="error-message" role="alert">{platformError}</p>}
            </div>
            <div className="schedule-list">
              {platformSessions.length === 0 ? <p className="empty-state">No computer or browser sessions yet.</p> : platformSessions.map((session) => <article className="schedule-card" key={session.id}>
                <div>
                  <span className={`task-state state-${session.state}`}>{session.state.replaceAll("_", " ")}</span>
                  <h3>{session.display_name}</h3>
                  <p>{session.kind === "browser" ? "Browser session" : "Computer-use session"}</p>
                  {session.state === "requested" && <small>Waiting for a verified executor</small>}
                  {session.state === "interrupted" && <small>Interrupted at restart; authority was not silently resumed</small>}
                  {session.kind === "browser" && platformBrowsers[session.id] && <small>
                    {platformBrowsers[session.id]!.title ? `${platformBrowsers[session.id]!.title} · ` : ""}
                    {platformBrowsers[session.id]!.safe_url}
                  </small>}
                  {session.kind === "computer_use" && platformComputerFrames[session.id] && <figure className="computer-frame">
                    <img
                      src={`data:${platformComputerFrames[session.id]!.media_type};base64,${platformComputerFrames[session.id]!.png_base64}`}
                      alt={`Current governed computer frame for ${session.display_name}`}
                    />
                    <figcaption>
                      Current {platformComputerFrames[session.id]!.observation.region.width}×{platformComputerFrames[session.id]!.observation.region.height} frame · {platformComputerFrames[session.id]!.sha256.slice(0, 12)}
                    </figcaption>
                  </figure>}
                  {advanced && <>
                    <code>{session.id}</code>
                    <small>Owner: {session.owner.replaceAll("_", " ")} · recovery count {session.recovery_count} · expires {new Date(session.expires_at).toLocaleString()}</small>
                    {session.allowed_origins.map((origin) => <code key={origin}>{origin}</code>)}
                    {session.computer_capabilities.map((capability) => <code key={capability}>{capability.replaceAll("_", ".")}</code>)}
                  </>}
                  {platformHistory[session.id] && <ol className="occurrence-list">
                    {platformHistory[session.id]!.events.map((event) => <li key={event.id}>
                      <span>{event.to_state.replaceAll("_", " ")}</span>
                      <time>{new Date(event.occurred_at).toLocaleString()}</time>
                      <small>{event.reason}</small>
                    </li>)}
                  </ol>}
                </div>
                <div className="task-actions">
                  {session.kind === "browser" && session.state === "requested" && <>
                    <button className="primary" type="button" disabled={platformBusy} onClick={() => attachPlatformBrowser(session.id, false)}>Start private browser</button>
                    {advanced && <button className="secondary" type="button" disabled={platformBusy} onClick={() => attachPlatformBrowser(session.id, true)}>Start visible browser</button>}
                  </>}
                  {session.kind === "browser" && session.state === "active" && <>
                    <input
                      aria-label={`URL for ${session.display_name}`}
                      value={platformBrowserUrls[session.id] ?? ""}
                      onChange={(event) => setPlatformBrowserUrls((current) => ({ ...current, [session.id]: event.target.value }))}
                      placeholder={session.allowed_origins[0] ?? "https://example.com"}
                    />
                    <button className="primary" type="button" disabled={platformBusy} onClick={() => navigatePlatformBrowser(session.id)}>Go</button>
                    {platformBrowsers[session.id]?.mode === "visible" && <button className="secondary" type="button" disabled={platformBusy} onClick={() => handoffPlatformBrowser(session.id)}>Hand to me</button>}
                  </>}
                  {session.kind === "browser" && session.state === "handed_off" && <>
                    <small>Returning control blanks the current page before MCG can read it.</small>
                    <button className="primary" type="button" disabled={platformBusy} onClick={() => reclaimPlatformBrowser(session.id)}>Return control to MCG</button>
                  </>}
                  {session.kind === "computer_use" && session.state === "requested" && <button className="primary" type="button" disabled={platformBusy} onClick={() => attachPlatformComputer(session.id)}>Start computer session</button>}
                  {session.kind === "computer_use" && session.state === "active" && <>
                    <button className="primary" type="button" disabled={platformBusy} onClick={() => capturePlatformComputer(session.id)}>Refresh screen</button>
                    {advanced && session.computer_capabilities.includes("mouse") && <>
                      <div className="computer-coordinate-row">
                        <label>
                          X
                          <input
                            inputMode="numeric"
                            aria-label={`X coordinate for ${session.display_name}`}
                            value={platformComputerCoordinates[session.id]?.x ?? "0"}
                            onChange={(event) => setPlatformComputerCoordinates((current) => ({ ...current, [session.id]: { x: event.target.value, y: current[session.id]?.y ?? "0" } }))}
                          />
                        </label>
                        <label>
                          Y
                          <input
                            inputMode="numeric"
                            aria-label={`Y coordinate for ${session.display_name}`}
                            value={platformComputerCoordinates[session.id]?.y ?? "0"}
                            onChange={(event) => setPlatformComputerCoordinates((current) => ({ ...current, [session.id]: { x: current[session.id]?.x ?? "0", y: event.target.value } }))}
                          />
                        </label>
                      </div>
                      <button className="secondary" type="button" disabled={platformBusy || !platformComputerFrames[session.id]} onClick={() => actOnPlatformComputer(session.id, false)}>Click point</button>
                      {session.computer_capabilities.includes("keyboard") && <>
                        <input
                          aria-label={`Text for ${session.display_name}`}
                          value={platformComputerText[session.id] ?? ""}
                          onChange={(event) => setPlatformComputerText((current) => ({ ...current, [session.id]: event.target.value }))}
                          placeholder="Text to type after click"
                        />
                        <button className="secondary" type="button" disabled={platformBusy || !platformComputerFrames[session.id]} onClick={() => actOnPlatformComputer(session.id, true)}>Click and type</button>
                      </>}
                    </>}
                    <button className="secondary" type="button" disabled={platformBusy} onClick={() => handoffPlatformComputer(session.id)}>Hand control to me</button>
                  </>}
                  {session.kind === "computer_use" && session.state === "handed_off" && <>
                    <small>MCG cannot see or control this display while it is yours. Returning control does not capture your activity.</small>
                    <button className="primary" type="button" disabled={platformBusy} onClick={() => reclaimPlatformComputer(session.id)}>Return computer control to MCG</button>
                  </>}
                  <button className="secondary" type="button" disabled={platformBusy} onClick={() => togglePlatformHistory(session.id)}>{platformHistory[session.id] ? "Hide history" : "History"}</button>
                  {!(["closed", "failed"] as string[]).includes(session.state) && <button className="link-button" type="button" disabled={platformBusy} onClick={() => closePlatform(session.id)}>Close</button>}
                </div>
              </article>)}
            </div>
          </div>
        </section>

        <section className="panel scheduler-panel" id="scheduler" aria-label="Scheduler">
          {!advanced ? <>
            <div className="section-heading">
              <div>
                <p className="eyebrow">SCHEDULED</p>
                <h2>Scheduled tasks</h2>
                <p className="muted">Set up recurring work in chat, then review runs and anything that needs attention here.</p>
              </div>
              <span className="count-badge">{schedules.filter((schedule) => schedule.state === "enabled").length} active</span>
            </div>
            <div className="scheduled-simple-layout">
              <div className="scheduled-chat-card">
                <div>
                  <h3>What should MCG do on a schedule?</h3>
                  <p>Describe the work. MCG will help refine the prompt, cadence, destination, and permissions before it runs unattended.</p>
                </div>
                <div className="scheduled-field">
                  <span>Project</span>
                  <select aria-label="Scheduled project" value={selectedProject?.project_id ?? ""} onChange={(event) => {
                    setSelectedProject(projects.find((project) => project.project_id === event.target.value) ?? null);
                    setScheduleError(null);
                  }}>
                    <option value="">Choose a project</option>
                    {projects.map((project) => <option key={project.project_id} value={project.project_id}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                  </select>
                </div>
                <label className="scheduled-prompt-field">
                  <span>Scheduled work</span>
                  <textarea value={scheduleSetupPrompt} onChange={(event) => setScheduleSetupPrompt(event.target.value)} placeholder="For example: summarize important project changes every weekday morning" />
                </label>
                <label>
                  <span>Each run</span>
                  <select value={scheduleDestination} onChange={(event) => setScheduleDestination(event.target.value as "standalone" | "chat")}>
                    <option value="standalone">Starts a new chat</option>
                    <option value="chat">Returns to this chat</option>
                  </select>
                </label>
                <button className="primary" type="button" disabled={!selectedProject} onClick={startScheduleChat}>Set up in chat</button>
              </div>
              <p className="scheduled-local-note">Local project schedules need this computer on, MCG running, and the project available. They currently run in the selected project; dedicated Git worktree isolation is planned.</p>
              {scheduleError && <p className="error-message" role="alert">{scheduleError}</p>}
              <div className="scheduled-inbox-toolbar">
                <label className="scheduled-search">
                  <span className="sr-only">Search scheduled tasks</span>
                  <input aria-label="Search scheduled tasks" value={scheduleSearch} onChange={(event) => setScheduleSearch(event.target.value)} placeholder="Search scheduled tasks" />
                </label>
                <div className="scheduled-filters" aria-label="Schedule filters">
                  {SCHEDULE_FILTERS.map(([filter, label]) => <button className="secondary" type="button" aria-pressed={scheduleFilter === filter} onClick={() => setScheduleFilter(filter)} key={filter}>{label}</button>)}
                </div>
                {scheduleNotifications.length > 0 && <button className="link-button" type="button" disabled={scheduleBusy} onClick={acknowledgeAllNotifications}>Mark all as read</button>}
              </div>
              {scheduleNotifications.length > 0 && <div className="notification-list" aria-label="Scheduler notifications">
                <h3>Needs attention</h3>
                {scheduleNotifications.map((notification) => <article className={`scheduler-notification severity-${notification.severity}`} key={notification.id}>
                  <div><strong>{notification.title}</strong><p>{notification.detail}</p></div>
                  <button className="link-button" type="button" onClick={() => acknowledgeNotification(notification.id)}>Mark as read</button>
                </article>)}
              </div>}
              <div className="schedule-list" aria-label="Scheduled task inbox">
                {schedules.filter((schedule) => (scheduleFilter === "all" || schedule.state === scheduleFilter) && `${schedule.name} ${schedule.goal}`.toLowerCase().includes(scheduleSearch.trim().toLowerCase())).length === 0
                  ? <div className="site-empty-state"><strong>No scheduled tasks found</strong><span>Set up recurring work in chat or change the filters.</span></div>
                  : schedules.filter((schedule) => (scheduleFilter === "all" || schedule.state === scheduleFilter) && `${schedule.name} ${schedule.goal}`.toLowerCase().includes(scheduleSearch.trim().toLowerCase())).map((schedule) => <article className="schedule-card scheduled-inbox-card" key={schedule.id}>
                    <div>
                      <span className={`task-state state-${schedule.state}`}>{schedule.state === "enabled" ? "active" : schedule.state}</span>
                      <h3>{schedule.name}</h3>
                      <p>{schedule.goal}</p>
                      <small>{scheduleTriggerLabel(schedule.trigger)}{schedule.next_run_at ? ` · Next ${new Date(schedule.next_run_at).toLocaleString()}` : ""}</small>
                      <small>{projects.find((project) => project.project_id === schedule.project_id)?.root.split(/[\\/]/).filter(Boolean).at(-1) ?? "Project unavailable"}</small>
                      {scheduleHistory[schedule.id] && <ol className="occurrence-list">
                        {scheduleHistory[schedule.id]!.occurrences.map((occurrence) => <li key={occurrence.id}>
                          <span>{occurrence.state.replaceAll("_", " ")}</span>
                          <time>{new Date(occurrence.due_at).toLocaleString()}</time>
                          {occurrence.detail && <small>{occurrence.detail}</small>}
                        </li>)}
                      </ol>}
                    </div>
                    <div className="task-actions">
                      {schedule.state === "enabled" ? <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => changeScheduleState(schedule.id, "paused")}>Pause</button> : schedule.state === "paused" && <button className="primary" type="button" disabled={scheduleBusy} onClick={() => changeScheduleState(schedule.id, "enabled")}>Resume</button>}
                      <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => toggleScheduleHistory(schedule.id)}>{scheduleHistory[schedule.id] ? "Hide runs" : "Recent runs"}</button>
                    </div>
                  </article>)}
              </div>
            </div>
          </> : <>
          <div className="section-heading">
            <div>
              <p className="eyebrow">ADVANCED SCHEDULER</p>
              <h2>Scheduler administration</h2>
              <p className="muted">Create exact triggers and inspect durable restart, permission, budget, and occurrence state.</p>
            </div>
            <span className="count-badge">{schedules.filter((schedule) => schedule.state === "enabled").length} active</span>
          </div>
          <div className="scheduler-layout">
            <div className="schedule-composer">
              <label>
                Name
                <input value={scheduleName} onChange={(event) => setScheduleName(event.target.value)} placeholder="Nightly project check" />
              </label>
              <label>
                Scheduled goal
                <textarea value={scheduleGoal} onChange={(event) => setScheduleGoal(event.target.value)} placeholder="Check the project and fix verified problems" />
              </label>
              <div className="scheduler-advanced">
                <label>
                  Trigger
                  <select value={scheduleKind} onChange={(event) => {
                    const kind = event.target.value as ScheduleTrigger["kind"];
                    setScheduleKind(kind);
                    setScheduleDetail(kind === "cron" ? "0 2 * * *" : kind === "file_watch" ? "README.md" : kind === "event" ? "task_failed" : kind === "once" ? "" : "300");
                    setScheduleSecondary("30");
                  }}>
                    <option value="interval">Fixed interval</option>
                    <option value="once">One time</option>
                    <option value="cron">Cron (UTC)</option>
                    <option value="file_watch">File watch</option>
                    <option value="event">System event</option>
                    <option value="heartbeat">Adaptive heartbeat</option>
                  </select>
                </label>
                <label>
                  {scheduleKind === "cron" ? "Five-field cron" : scheduleKind === "file_watch" ? "Project-relative file" : scheduleKind === "event" ? "Event kind" : scheduleKind === "once" ? "Run at" : scheduleKind === "heartbeat" ? "Idle seconds" : "Interval seconds"}
                  <input
                    type={scheduleKind === "once" ? "datetime-local" : "text"}
                    list={scheduleKind === "event" ? "scheduler-event-kinds" : undefined}
                    value={scheduleDetail}
                    onChange={(event) => setScheduleDetail(event.target.value)}
                  />
                  {scheduleKind === "event" && <datalist id="scheduler-event-kinds">
                    {SCHEDULE_EVENT_KINDS.map((kind) => <option key={kind} value={kind} />)}
                  </datalist>}
                </label>
                {(scheduleKind === "file_watch" || scheduleKind === "heartbeat") && <label>
                  {scheduleKind === "file_watch" ? "Poll seconds" : "Busy seconds"}
                  <input type="number" min="1" value={scheduleSecondary} onChange={(event) => setScheduleSecondary(event.target.value)} />
                </label>}
                <label>
                  Risk
                  <select value={scheduleRisk} onChange={(event) => setScheduleRisk(event.target.value as RiskLevel)}>
                    <option value="low">Low</option>
                    <option value="medium">Medium</option>
                    <option value="high">High</option>
                    <option value="critical">Critical</option>
                  </select>
                </label>
                <label>
                  Token limit
                  <input type="number" min="1" value={scheduleMaxTokens} onChange={(event) => setScheduleMaxTokens(event.target.value)} />
                </label>
                <label>
                  Cost limit (USD, optional)
                  <input type="number" min="0" step="0.01" value={scheduleMaxCost} onChange={(event) => setScheduleMaxCost(event.target.value)} placeholder="No dollar limit" />
                </label>
                <label>
                  Duration limit (seconds)
                  <input type="number" min="1" value={scheduleMaxDuration} onChange={(event) => setScheduleMaxDuration(event.target.value)} />
                </label>
                <label>
                  Model call limit
                  <input type="number" min="1" value={scheduleModelCalls} onChange={(event) => setScheduleModelCalls(event.target.value)} />
                </label>
              </div>
              <div className="composer-actions">
                <span className="muted">{selectedProject ? `Project: ${selectedProject.root.split(/[\\/]/).filter(Boolean).at(-1)}` : "Select a project above"}</span>
                <button className="primary" type="button" disabled={scheduleBusy || !selectedProject} onClick={addSchedule}>
                  {scheduleBusy ? "Saving..." : "Create schedule"}
                </button>
              </div>
              {scheduleError && <p className="error-message" role="alert">{scheduleError}</p>}
            </div>
            <div className="scheduler-overview">
              {scheduleNotifications.length > 0 && <div className="notification-list" aria-label="Scheduler notifications">
                <h3>Needs attention</h3>
                {scheduleNotifications.map((notification) => <article className={`scheduler-notification severity-${notification.severity}`} key={notification.id}>
                  <div><strong>{notification.title}</strong><p>{notification.detail}</p></div>
                  <button className="link-button" type="button" onClick={() => acknowledgeNotification(notification.id)}>Dismiss</button>
                </article>)}
              </div>}
              <div className="schedule-list">
                {schedules.length === 0 ? <p className="empty-state">No scheduled work yet.</p> : schedules.map((schedule) => <article className="schedule-card" key={schedule.id}>
                  <div>
                    <span className={`task-state state-${schedule.state}`}>{schedule.state}</span>
                    <h3>{schedule.name}</h3>
                    <p>{scheduleTriggerLabel(schedule.trigger)}</p>
                    {schedule.next_run_at && <small>Next: {new Date(schedule.next_run_at).toLocaleString()}</small>}
                    <code>{schedule.id}</code><small>{schedule.goal}</small>
                    {scheduleHistory[schedule.id] && <ol className="occurrence-list">
                      {scheduleHistory[schedule.id]!.occurrences.map((occurrence) => <li key={occurrence.id}>
                        <span>{occurrence.state.replaceAll("_", " ")}</span>
                        <time>{new Date(occurrence.due_at).toLocaleString()}</time>
                        {occurrence.detail && <small>{occurrence.detail}</small>}
                      </li>)}
                    </ol>}
                  </div>
                  <div className="task-actions">
                    {schedule.state === "enabled" ? <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => changeScheduleState(schedule.id, "paused")}>Pause</button> : schedule.state === "paused" && <button className="primary" type="button" disabled={scheduleBusy} onClick={() => changeScheduleState(schedule.id, "enabled")}>Resume</button>}
                    {schedule.state !== "completed" && <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => requestScheduleWake(schedule.id)}>Wake now</button>}
                    <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => toggleScheduleHistory(schedule.id)}>{scheduleHistory[schedule.id] ? "Hide history" : "History"}</button>
                    {schedule.state !== "completed" && <button className="link-button" type="button" disabled={scheduleBusy} onClick={() => changeScheduleState(schedule.id, "completed")}>Remove</button>}
                  </div>
                </article>)}
              </div>
            </div>
          </div>
          <p className="integration-note">Advanced RFC 5545 recurrence rules, multi-project schedules, dedicated worktrees, and explicit model/reasoning selection remain product work. Existing schedules retain their durable local execution and restart-recovery contract.</p>
          </>}
        </section>

        <section className="panel provider-panel" id="providers">
          <div>
            <p className="eyebrow">MODEL PROVIDERS</p>
            <h2>Connect your OpenAI subscription</h2>
            <p className="muted">
              Sign in with your ChatGPT subscription. This connection never silently
              switches to API-key billing.
            </p>
          </div>
          <div className="provider-actions">
            {!login ? (
              <>
                <button className="primary" type="button" disabled={busy} onClick={() => startLogin("browser")}>
                  {busy ? "Opening sign-in..." : "Continue with OpenAI"}
                </button>
                <p className="form-note">Uses browser sign-in with your ChatGPT subscription. No CLI pairing code.</p>
                {advanced && <details className="device-login">
                  <summary>Can't open a browser?</summary>
                  <p>Use a one-time device code only for a headless or remote computer. This method requires entering a separate pairing code.</p>
                  <button className="secondary" type="button" disabled={busy} onClick={() => startLogin("device_code")}>
                    Use a device code instead
                  </button>
                </details>}
              </>
            ) : (
              <div className="oauth-card" role="status">
                <p>{login.user_code
                  ? "Enter this one-time code on the device authorization page."
                  : "Finish signing in with OpenAI in your browser, then return here."}</p>
                <button className="link-button" type="button" onClick={() => openVerification(login.verification_url)}>
                  {login.user_code ? "Open device authorization page" : "Open sign-in page"}
                </button>
                {login.user_code && <p className="device-code">Code: <strong>{login.user_code}</strong></p>}
                <div className="composer-actions">
                  <button className="secondary" type="button" disabled={busy} onClick={cancelLogin}>Cancel</button>
                  <button className="primary" type="button" disabled={busy} onClick={finishLogin}>
                    {busy ? "Checking..." : login.user_code ? "I've entered the code" : "Finish connection"}
                  </button>
                </div>
              </div>
            )}
            <p className="provider-state">{providerStatus}</p>
            {providerError && <p className="error-message" role="alert">{providerError}</p>}
            <button className="secondary" type="button" onClick={() => {
              if (providerSetup) setProviderCredential("");
              setProviderSetup((value) => !value);
            }}>
              {providerSetup ? "Cancel provider setup" : "Add API or local provider"}
            </button>
          </div>
          {providerSetup && <div className="provider-setup" aria-label="Add model provider">
            <label>
              Provider type
              <select value={providerKind} onChange={(event) => {
                const kind = event.target.value as ProviderKind;
                const previousDefault = providerKind === "open_ai"
                  ? "https://api.openai.com/v1"
                  : providerKind === "open_router"
                    ? "https://openrouter.ai/api/v1"
                    : "";
                const nextDefault = kind === "open_ai"
                  ? "https://api.openai.com/v1"
                  : kind === "open_router"
                    ? "https://openrouter.ai/api/v1"
                    : "";
                setProviderKind(kind);
                if (!providerUrl.trim() || providerUrl === previousDefault) {
                  setProviderUrl(nextDefault);
                }
              }}>
                <option value="open_ai_compatible">OpenAI-compatible or local</option>
                <option value="open_ai">OpenAI API</option>
                <option value="open_router">OpenRouter</option>
              </select>
            </label>
            <label>
              Provider name
              <input value={providerName} onChange={(event) => setProviderName(event.target.value)} placeholder="Local GPU" />
            </label>
            <label>
              Base URL
              <input value={providerUrl} onChange={(event) => setProviderUrl(event.target.value)} placeholder="http://127.0.0.1:1234/v1" inputMode="url" />
            </label>
            <label>
              Authentication
              <select value={providerAuth} onChange={(event) => {
                const auth = event.target.value as ProviderAuthMode;
                setProviderAuth(auth);
                if (auth === "none") setProviderCredential("");
              }}>
                <option value="api_key">API key</option>
                <option value="bearer_token">Bearer token</option>
                <option value="none">None (local/trusted endpoint)</option>
              </select>
            </label>
            {providerAuth !== "none" && <label>
              {providerAuth === "api_key" ? "API key" : "Bearer token"}
              <input type="password" autoComplete="off" value={providerCredential} onChange={(event) => setProviderCredential(event.target.value)} />
            </label>}
            {advanced && <div className="advanced-fields provider-advanced">
              <label>
                Connection id
                <input value={providerId} onChange={(event) => setProviderId(event.target.value)} placeholder="Generated from the name" />
              </label>
              <label>
                OpenAI organization (optional)
                <input value={providerOrganization} onChange={(event) => setProviderOrganization(event.target.value)} />
              </label>
              <label>
                OpenAI project (optional)
                <input value={providerProject} onChange={(event) => setProviderProject(event.target.value)} />
              </label>
            </div>}
            <p className="form-note">MCG discovers models from the endpoint before saving. Credentials go directly to the operating-system credential store.</p>
            <button className="primary" type="button" disabled={busy} onClick={addProvider}>{busy ? "Connecting…" : "Connect provider"}</button>
          </div>}
          <div className="provider-connections" aria-label="Configured model providers">
            <div className="section-heading compact-heading">
              <h3>Configured providers</h3>
              <span className="count-badge">{providers.length} connected</span>
            </div>
            {providers.length === 0 ? <p className="empty-state">No model providers connected yet.</p> : providers.map((item) => (
              <article className="provider-card" key={item.connection.id}>
                <div>
                  <h3>{item.connection.display_name}</h3>
                  <p>{item.connection.auth_mode.startsWith("chat_gpt") ? "Subscription billing" : item.connection.auth_mode === "none" ? "Endpoint-reported billing" : "Usage-based billing"} · {item.catalog?.models.length ?? 0} models</p>
                  {advanced && <><code>{item.connection.base_url}</code><small>{item.connection.kind.replaceAll("_", " ")} · {item.connection.auth_mode.replaceAll("_", " ")}</small></>}
                </div>
                <div className="card-actions">
                  <button className="secondary" type="button" disabled={busy} onClick={() => refreshProvider(item.connection.id)}>Refresh models</button>
                  <button className="secondary" type="button" disabled={busy} onClick={() => removeProvider(item)}>Disconnect</button>
                </div>
                {advanced && item.catalog && <ul className="model-list">
                  {item.catalog.models.slice(0, 12).map((model) => <li key={model.id}>
                    <span><strong>{model.display_name}</strong><code>{model.id}</code></span>
                    <small>{modelPrice(model)}</small>
                  </li>)}
                  {item.catalog.models.length > 12 && <li className="model-overflow">+ {item.catalog.models.length - 12} more models</li>}
                </ul>}
              </article>
            ))}
            <div className="section-heading compact-heading">
              <h3>Local proxy usage</h3>
              <span className="count-badge">{proxyUsage?.call_count ?? 0} calls</span>
            </div>
            {proxyUsage ? <article className="provider-card" aria-label="Provider proxy usage">
              <div>
                <h3>{proxyUsage.usage.total_tokens.toLocaleString()} tokens</h3>
                <p>
                  {proxyUsage.conversation_count} conversations Â· ${proxyUsage.known_actual_cost_usd.toFixed(6)} known cost
                  {proxyUsage.unknown_cost_call_count > 0 && ` Â· ${proxyUsage.unknown_cost_call_count} unknown-cost calls`}
                </p>
                <small>Conversation history retains usage and routing metadata, not prompts or generated text.</small>
              </div>
              {advanced && proxyConversations.length > 0 && <ul className="model-list" aria-label="Proxy conversations">
                {proxyConversations.slice(0, 12).map((conversation) => <li key={conversation.id}>
                  <span><strong>{conversation.task_id ? "Task conversation" : "Standalone conversation"}</strong><code>{conversation.id}</code></span>
                  <small>{new Date(conversation.updated_at).toLocaleString()}</small>
                </li>)}
              </ul>}
            </article> : <p className="empty-state">No local proxy usage recorded yet.</p>}
          </div>
        </section>

        <section className="panel mcp-panel" id="mcp">
          <div className="section-heading">
            <div>
              <p className="eyebrow">CONNECTED TOOLS</p>
              <h2>MCP servers</h2>
              <p className="muted">
                Add a tool server. MCG discovers its capabilities before saving it and
                keeps remote actions inside normal permissions and approvals.
              </p>
            </div>
            <span className="count-badge">{mcpConnections.length} connected</span>
          </div>

          <div className="mcp-layout">
            <div className="mcp-form" aria-label="Add MCP server">
              <label>
                Server name
                <input
                  value={mcpName}
                  onChange={(event) => setMcpName(event.target.value)}
                  placeholder="My tools"
                />
              </label>
              {(!advanced || mcpTransport === "streamable_http") && (
                <label>
                  Server URL
                  <input
                    value={mcpUrl}
                    onChange={(event) => setMcpUrl(event.target.value)}
                    placeholder="https://tools.example.com/mcp"
                    inputMode="url"
                  />
                </label>
              )}
              {advanced && (
                <div className="advanced-fields">
                  <label>
                    Transport
                    <select
                      value={mcpTransport}
                      onChange={(event) => setMcpTransport(event.target.value as McpTransportKind)}
                    >
                      <option value="streamable_http">HTTP</option>
                      <option value="stdio">Local executable (stdio)</option>
                    </select>
                  </label>
                  {mcpTransport === "stdio" && (
                    <>
                      <label>
                        Executable
                        <input
                          value={mcpCommand}
                          onChange={(event) => setMcpCommand(event.target.value)}
                          placeholder="npx"
                        />
                      </label>
                      <label>
                        Arguments (one per line)
                        <textarea
                          value={mcpArgs}
                          onChange={(event) => setMcpArgs(event.target.value)}
                          placeholder={"-y\n@modelcontextprotocol/server-filesystem\nC:\\Projects"}
                        />
                      </label>
                    </>
                  )}
                  <label>
                    Connection id
                    <input
                      value={mcpId}
                      onChange={(event) => setMcpId(event.target.value)}
                      placeholder="Generated from the name"
                    />
                  </label>
                  <label>
                    Protocol
                    <select
                      value={mcpProtocol}
                      onChange={(event) => setMcpProtocol(event.target.value as McpProtocolMode)}
                    >
                      <option value="auto">Automatic</option>
                      <option value="modern20260728">2026-07-28 only</option>
                      <option value="legacy">Legacy only</option>
                    </select>
                  </label>
                  {mcpTransport === "streamable_http" && <label>
                    Authentication
                    <select
                      value={mcpAuth}
                      onChange={(event) => setMcpAuth(event.target.value as McpAuthMode)}
                    >
                      <option value="none">None</option>
                      <option value="bearer_token">Bearer token</option>
                    </select>
                  </label>}
                  {mcpTransport === "streamable_http" && mcpAuth === "bearer_token" && (
                    <label>
                      Bearer token
                      <input
                        type="password"
                        value={mcpCredential}
                        onChange={(event) => setMcpCredential(event.target.value)}
                        autoComplete="off"
                      />
                    </label>
                  )}
                </div>
              )}
              <button className="primary" type="button" disabled={mcpBusy} onClick={addMcpServer}>
                {mcpBusy ? "Checking server…" : "Add server"}
              </button>
              {mcpError && <p className="error-message" role="alert">{mcpError}</p>}
            </div>

            <div className="connection-list" aria-label="Configured MCP servers">
              {mcpConnections.length === 0 ? (
                <p className="empty-state">No tool servers connected yet.</p>
              ) : (
                mcpConnections.map((item) => (
                  <article className="connection-card" key={item.connection.id}>
                    <div>
                      <h3>{item.connection.display_name}</h3>
                      <p>
                        {item.catalog?.tools.length ?? 0} tools · {item.catalog?.protocol_version ?? "Not discovered"}
                      </p>
                      {advanced && <code>{item.connection.id}</code>}
                    </div>
                    <button
                      className="secondary"
                      type="button"
                      disabled={mcpBusy}
                      onClick={() => refreshMcpServer(item.connection.id)}
                    >
                      Refresh
                    </button>
                  </article>
                ))
              )}
            </div>
          </div>
        </section>

        <section className="panel plugin-panel" id="plugins" aria-label="Plugins">
          {!advanced ? <>
            <div className="section-heading">
              <div>
                <p className="eyebrow">PLUGINS</p>
                <h2 id="plugins-heading">Plugin directory</h2>
                <p className="muted">Add reusable skills and connections, then use them naturally from a new chat.</p>
              </div>
              <span className="count-badge">{plugins.length} installed</span>
            </div>
            <div className="plugin-directory-toolbar">
              <label className="plugin-directory-search">
                <span className="sr-only">Search plugins</span>
                <input aria-label="Search plugins" value={pluginSearch} onChange={(event) => setPluginSearch(event.target.value)} placeholder="Search plugins" />
              </label>
              <div className="plugin-directory-tabs" aria-label="Plugin sources">
                {PLUGIN_DIRECTORY_TABS.map(([tab, label]) => <button type="button" className="secondary" aria-pressed={pluginDirectoryTab === tab} onClick={() => setPluginDirectoryTab(tab)} key={tab}>{label}</button>)}
              </div>
            </div>
            <p className="plugin-directory-note">Enabled plugins can seed a new chat with an explicit @ mention. Direct chat-to-plugin resolution, the universal public marketplace, and connector sign-in are not connected in this build yet; exact invocation remains available in Advanced mode.</p>
            {pluginError && <p className="error-message" role="alert">{pluginError}</p>}
            <div className="plugin-directory-grid" aria-label="Plugin directory results">
              {directoryPlugins.length === 0 ? <div className="site-empty-state">
                <strong>{pluginSearch.trim() ? "No matching plugins" : pluginDirectoryTab === "openai" ? "OpenAI catalog not connected" : `No ${pluginDirectoryTab} plugins`}</strong>
                <span>{pluginDirectoryTab === "installed" ? "Install a plugin from Advanced mode or change the source." : "This source will populate when marketplace synchronization is configured."}</span>
              </div> : directoryPlugins.map((plugin) => {
                const installation = plugin.installation;
                const manifest = plugin.active_revision.manifest;
                const granted = installation.capability_grants.length === manifest.requested_capabilities.length;
                const reviewingAccess = pluginAccessReviewId === installation.plugin_id;
                return <article className="plugin-directory-card" key={installation.plugin_id}>
                  <div className="plugin-directory-card-heading">
                    <span className="plugin-directory-icon" aria-hidden="true">{manifest.display_name.slice(0, 1).toUpperCase()}</span>
                    <div>
                      <h3>{manifest.display_name}</h3>
                      <p>{plugin.active_revision.source.kind === "registry" ? "Workspace marketplace" : "Personal · local"} · Version {manifest.version}</p>
                    </div>
                    <span className={`status-chip ${installation.enabled ? "status-enabled" : "status-disabled"}`}>{installation.enabled ? "On" : "Off"}</span>
                  </div>
                  <p className="plugin-directory-description">Adds a reusable sandboxed capability to MCG chats. Its requested access stays governed by the host policy.</p>
                  <div className="plugin-component-chips" aria-label={`${manifest.display_name} components`}>
                    <span>Capability</span>
                    {manifest.requested_capabilities.map((capability) => <span key={capability}>{capabilityLabel(capability)}</span>)}
                  </div>
                  <div className="composer-actions">
                    {installation.enabled ? <>
                      <button className="primary" type="button" onClick={() => usePluginInNewChat(plugin)}>Start new chat</button>
                      <button className="secondary" type="button" disabled={pluginBusy} onClick={() => togglePlugin(plugin)}>Turn off</button>
                    </> : <>
                      {!granted && <button className="secondary" type="button" onClick={() => setPluginAccessReviewId(reviewingAccess ? null : installation.plugin_id)}>{reviewingAccess ? "Close access review" : "Review access"}</button>}
                      <button className="primary" type="button" disabled={pluginBusy || !granted} onClick={() => togglePlugin(plugin)}>Turn on</button>
                    </>}
                  </div>
                  {reviewingAccess && <div className="plugin-simple-access" aria-label={`Review access for ${manifest.display_name}`}>
                    <h4>Requested access</h4>
                    <p>Review and limit what this plugin can reach before turning it on.</p>
                    <ul>{manifest.requested_capabilities.map((capability) => <li key={capability}>
                      <strong>{capabilityLabel(capability)}</strong>
                      {(capability === "filesystem_read" || capability === "filesystem_write") && <span>Current task project only</span>}
                      {capability === "network" && <input aria-label={`${manifest.display_name} network origin`} value={draftPluginScope(installation.plugin_id, "network")} onChange={(event) => updatePluginScope(installation.plugin_id, "network", event.target.value)} placeholder="https://api.example.com" />}
                      {capability === "process" && <input aria-label={`${manifest.display_name} executable`} value={draftPluginScope(installation.plugin_id, "process")} onChange={(event) => updatePluginScope(installation.plugin_id, "process", event.target.value)} placeholder="Exact executable" />}
                      {capability === "credentials" && <input aria-label={`${manifest.display_name} credential`} value={draftPluginScope(installation.plugin_id, "credentials")} onChange={(event) => updatePluginScope(installation.plugin_id, "credentials", event.target.value)} placeholder="provider:key" />}
                    </li>)}</ul>
                    <button className="primary" type="button" disabled={pluginBusy} onClick={() => grantPluginAccess(plugin)}>Approve requested access</button>
                  </div>}
                </article>;
              })}
            </div>
          </> : <>
            <div className="section-heading">
              <div>
                <p className="eyebrow">ADVANCED PLUGINS</p>
                <h2 id="plugins-heading">Plugin administration</h2>
                <p className="muted">Import bounded packages, configure exact broker scopes, invoke structured inputs, and inspect immutable evidence.</p>
              </div>
              <span className="count-badge">{plugins.length} installed</span>
            </div>
            <div className="plugin-import" aria-label="Install local plugin">
              <div>
                <h3>Install from this computer</h3>
                <p className="form-note">Local plugins are imported disabled and unsigned. MCG does not grant access automatically.</p>
              </div>
              <label>Manifest (.json)<input type="file" accept="application/json,.json" onChange={(event) => setPluginManifestFile(event.target.files?.[0] ?? null)} /></label>
              <label>Module (.wasm)<input type="file" accept="application/wasm,.wasm" onChange={(event) => setPluginModuleFile(event.target.files?.[0] ?? null)} /></label>
              <button className="primary" type="button" disabled={pluginBusy} onClick={importPlugin}>{pluginBusy ? "Working…" : "Install plugin"}</button>
            </div>
            <div className="plugin-registry-import" aria-label="Install verified registry plugin">
              <div>
                <h3>Install signed registry package</h3>
                <p className="form-note">Import the bounded package request supplied by a configured trusted registry.</p>
              </div>
              <label>Signed package request (.json)<input type="file" accept="application/json,.json" onChange={(event) => setPluginRegistryPackageFile(event.target.files?.[0] ?? null)} /></label>
              <button className="secondary" type="button" disabled={pluginBusy} onClick={importRegistryPlugin}>Verify and install</button>
            </div>
            {pluginError && <p className="error-message" role="alert">{pluginError}</p>}
            <div className="plugin-list" aria-label="Installed plugins">
            {plugins.length === 0 ? <p className="empty-state">No plugins installed yet.</p> : plugins.map((plugin) => {
              const installation = plugin.installation;
              const manifest = plugin.active_revision.manifest;
              const invocation = pluginInvocations[installation.plugin_id];
              const granted = installation.capability_grants.length === manifest.requested_capabilities.length;
              return <article className="plugin-card" key={installation.plugin_id}>
                <div className="plugin-card-heading">
                  <div>
                    <h3>{manifest.display_name}</h3>
                    <p>Version {manifest.version} · {plugin.active_revision.signature.state === "verified" ? "Verified publisher" : "Unsigned local plugin"}</p>
                  </div>
                  <span className={`status-chip ${installation.enabled ? "status-enabled" : "status-disabled"}`}>
                    {installation.enabled ? "Enabled" : "Disabled"}
                  </span>
                </div>

                <div className="plugin-permissions">
                  <h4>Requested access</h4>
                  {manifest.requested_capabilities.length === 0 ? <p className="form-note">No brokered access requested.</p> : <ul>
                    {manifest.requested_capabilities.map((capability) => <li key={capability}>
                      <strong>{capabilityLabel(capability)}</strong>
                      {(capability === "filesystem_read" || capability === "filesystem_write") && <span>Current task project only</span>}
                      {capability === "network" && <input aria-label={`${manifest.display_name} network origin`} value={draftPluginScope(installation.plugin_id, "network")} onChange={(event) => updatePluginScope(installation.plugin_id, "network", event.target.value)} placeholder="https://api.example.com" />}
                      {capability === "process" && <input aria-label={`${manifest.display_name} executable`} value={draftPluginScope(installation.plugin_id, "process")} onChange={(event) => updatePluginScope(installation.plugin_id, "process", event.target.value)} placeholder="Exact executable path or name" />}
                      {capability === "credentials" && <input aria-label={`${manifest.display_name} credential`} value={draftPluginScope(installation.plugin_id, "credentials")} onChange={(event) => updatePluginScope(installation.plugin_id, "credentials", event.target.value)} placeholder="provider:key" />}
                    </li>)}
                  </ul>}
                  <div className="composer-actions">
                    <button className="secondary" type="button" disabled={pluginBusy || installation.enabled} onClick={() => grantPluginAccess(plugin)}>
                      {granted ? "Update access" : "Grant requested access"}
                    </button>
                    <button className={installation.enabled ? "secondary" : "primary"} type="button" disabled={pluginBusy || (!granted && !installation.enabled)} onClick={() => togglePlugin(plugin)}>
                      {installation.enabled ? "Disable" : "Enable"}
                    </button>
                  </div>
                </div>

                <div className="plugin-runner">
                  <label>
                    Plugin input (JSON)
                    <textarea value={pluginInputDrafts[installation.plugin_id] ?? ""} onChange={(event) => setPluginInputDrafts((current) => ({ ...current, [installation.plugin_id]: event.target.value }))} placeholder="{}" />
                  </label>
                  <button className="primary" type="button" disabled={pluginBusy || !installation.enabled} onClick={() => runPlugin(plugin)}>Run plugin</button>
                  {invocation && <div className="plugin-result" role="status">
                    <strong>{invocation.invocation.state}</strong>
                    {invocation.result && <pre>{JSON.stringify(invocation.result.output, null, 2)}</pre>}
                    {invocation.invocation.error_code && <code>{invocation.invocation.error_code}</code>}
                  </div>}
                </div>

                <details className="advanced-details">
                  <summary>Audit details</summary>
                  <dl>
                    <div><dt>Plugin ID</dt><dd><code>{installation.plugin_id}</code></dd></div>
                    <div><dt>Revision ID</dt><dd><code>{installation.active_revision_id}</code></dd></div>
                    <div><dt>Module SHA-256</dt><dd><code>{plugin.active_revision.module_sha256}</code></dd></div>
                    <div><dt>Source</dt><dd>{plugin.active_revision.source.kind.replaceAll("_", " ")}</dd></div>
                  </dl>
                  {invocation && <button className="secondary" type="button" disabled={pluginBusy} onClick={() => refreshPluginHistory(plugin)}>
                    Refresh invocation history{pluginHistoryCounts[installation.plugin_id] !== undefined ? ` (${pluginHistoryCounts[installation.plugin_id]} records)` : ""}
                  </button>}
                </details>
              </article>;
            })}
            </div>
          </>}
        </section>

        <section className="panel settings-page" id="settings" aria-label="Settings">
          <button className="settings-back" type="button" onClick={() => navigateSurface("tasks")}><span aria-hidden="true">←</span> Back to app</button>
          <label className="settings-search"><span aria-hidden="true">⌕</span><span className="sr-only">Search settings</span><input aria-label="Search settings" value={settingsSearch} onChange={(event) => setSettingsSearch(event.target.value)} placeholder="Search settings…" /></label>
          <div className="settings-layout">
            <nav className="settings-nav" aria-label="Settings sections">
              {settingsGroups.map((group) => {
                const items = group.items.filter((item) => !normalizedSettingsSearch || item.label.toLocaleLowerCase().includes(normalizedSettingsSearch));
                return items.length > 0 && <section key={group.label}><h3>{group.label}</h3>{items.map((item) => <button className={settingsTab === item.id ? "active" : ""} type="button" aria-current={settingsTab === item.id ? "page" : undefined} onClick={() => setSettingsTab(item.id)} key={item.id}><span aria-hidden="true">{item.glyph}</span>{item.label}</button>)}</section>;
              })}
              {settingsGroups.every((group) => group.items.every((item) => normalizedSettingsSearch && !item.label.toLocaleLowerCase().includes(normalizedSettingsSearch))) && <p className="settings-no-results">No settings found.</p>}
            </nav>
            <div className="settings-content" data-settings-tab={settingsTab}>
              <div className="settings-pane" data-settings-pane="general">
                <div className="section-heading">
                  <div><h2 id="settings-heading">General</h2><p className="muted">Choose how MCG works by default.</p></div>
                  {userSettings && <span className="count-badge">Revision {userSettings.revision}</span>}
                </div>
                {userSettings ? <div className="provider-setup" aria-label="Owner settings">
            <label>
              Autonomy
              <select
                value={userSettings.autonomy}
                disabled={settingsBusy}
                onChange={(event) => setUserSettings({ ...userSettings, autonomy: event.target.value as UserSettings["autonomy"] })}
              >
                <option value="guided">Guided</option>
                <option value="balanced">Balanced</option>
                <option value="autonomous">Autonomous</option>
              </select>
            </label>
            <p className="form-note">
              {userSettings.autonomy === "guided" && "Ask before consequential local actions."}
              {userSettings.autonomy === "balanced" && "Act independently for low-risk work and ask for higher-risk actions."}
              {userSettings.autonomy === "autonomous" && "Act independently within the owner constitution and capability limits."}
            </p>
            <div className="settings-mode-row">
              <span><strong>Interface mode</strong><small>Simple mode keeps everyday choices compact. Advanced mode reveals exact runtime controls and evidence.</small></span>
              <button ref={modeButtonRef} className="secondary" type="button" aria-pressed={advanced} disabled={settingsBusy} onClick={toggleAdvancedMode}>{advanced ? "Use simple mode" : "Use advanced mode"}</button>
            </div>
            {advanced && <label className="check-row">
              <input
                type="checkbox"
                checked={userSettings.shadow_experiments}
                disabled={settingsBusy}
                onChange={(event) => setUserSettings({ ...userSettings, shadow_experiments: event.target.checked })}
              />
              Run opt-in shadow model experiments
            </label>}
            <button
              className="primary"
              type="button"
              disabled={settingsBusy}
              onClick={() => saveUserSettings(userSettings, "Updated owner settings from desktop")}
            >
              {settingsBusy ? "Saving…" : "Save settings"}
            </button>
            {settingsError && <p className="error-message" role="alert">{settingsError}</p>}
            {advanced && <div>
              <h3>Revision history</h3>
              <ul className="history-list" aria-label="Settings revision history">
                {settingsHistory.slice().reverse().slice(0, 12).map((revision) => <li key={revision.settings.revision}>
                  <strong>Revision {revision.settings.revision}</strong>
                  <span>{revision.settings.autonomy} · {revision.settings.advanced_mode ? "advanced" : "simple"} · shadow {revision.settings.shadow_experiments ? "on" : "off"}</span>
                  <small>{revision.reason}</small>
                </li>)}
              </ul>
            </div>}
                </div> : <p className="empty-state">Loading durable settings…</p>}
              </div>
              <div className="settings-pane" data-settings-pane="import">
                <div className="section-heading"><div><h2>Import</h2><p className="muted">Add folders available to every chat and manage portable backups.</p></div></div>
          <div className="provider-connections global-folder-settings" aria-label="Global folders">
            <div className="section-heading compact-heading">
              <div>
                <h3>Global folders</h3>
                <p className="muted">Folders shared with every project and general chat. Adding one here does not create a project.</p>
              </div>
              <button className="secondary" type="button" disabled={globalFolderBusy} onClick={addGlobalFolder}>{globalFolderBusy ? "Adding…" : "Add global folder"}</button>
            </div>
            {globalFolders.length === 0 ? <p className="empty-state">No global folders added.</p> : <ul className="global-folder-list">
              {globalFolders.map((folder) => <li key={projectRootKey(folder)}><span><strong>{folder.split(/[\\/]/).filter(Boolean).at(-1) ?? folder}</strong><small>{folder}</small></span><button className="link-button" type="button" onClick={() => setGlobalFolders((current) => current.filter((candidate) => projectRootKey(candidate) !== projectRootKey(folder)))}>Remove</button></li>)}
            </ul>}
            {globalFolderError && <p className="error-message" role="alert">{globalFolderError}</p>}
          </div>
              </div>
              <div className="settings-pane" data-settings-pane="configuration">
                <div className="section-heading"><div><h2>Configuration</h2><p className="muted">Updates, diagnostics, and durable application details.</p></div></div>
          <div className="provider-connections" aria-label="Application updates">
            <div className="section-heading compact-heading">
              <div>
                <h3>Application updates</h3>
                <p className="muted">MCG checks only the trusted release service embedded by the owner-controlled build and accepts only packages signed by its embedded update key.</p>
              </div>
              {updateConfiguration && <span className="count-badge">Version {updateConfiguration.current_version}</span>}
            </div>
            {!updateConfiguration && !updateError && <p className="empty-state">Loading update policy…</p>}
            {updateConfiguration?.recovery_notice && <p className="error-message" role="status">
              {updateConfiguration.recovery_notice}
            </p>}
            {updateConfiguration && !updateConfiguration.configured && <p className="empty-state">
              Updates are unavailable in this build. Install a newer owner-signed package manually; MCG will never accept an unsigned fallback.
            </p>}
            {advanced && updateConfiguration && !updateConfiguration.configured && <p className="form-note">{updateConfiguration.detail}</p>}
            {updateConfiguration?.configured && <>
              <div className="composer-actions">
                <button className="primary" type="button" disabled={updateBusy} onClick={checkUpdates}>
                  {updateBusy ? "Checking…" : "Check for updates"}
                </button>
              </div>
              {updateCheck && !updateCheck.available_version && <p role="status">{updateCheck.detail}</p>}
              {updateCheck?.available_version && <article className="provider-card update-card">
                <div>
                  <h3>MCG {updateCheck.available_version}</h3>
                  <p>{updateCheck.detail}</p>
                  {updateCheck.notes && <p className="update-notes">{updateCheck.notes}</p>}
                  {advanced && <small>
                    {updateCheck.target ?? "current platform"}
                    {updateCheck.published_at ? ` · published ${new Date(updateCheck.published_at).toLocaleString()}` : ""}
                  </small>}
                </div>
                {!updateReview && <button className="primary" type="button" disabled={updateBusy} onClick={() => setUpdateReview(true)}>Review update</button>}
              </article>}
              {updateCheck?.available_version && updateReview && <div className="provider-form" aria-label="Confirm application update">
                <h3>Install signed MCG {updateCheck.available_version}?</h3>
                <p className="form-note">MCG will download and verify the signed package, create and verify a credential-free portable backup, stop the local service cleanly, install the update, and restart. It will stop without installing if any verification fails.</p>
                <div className="composer-actions">
                  <button className="primary" type="button" disabled={updateBusy} onClick={applyUpdate}>{updateBusy ? "Preparing verified update…" : "Back up and install"}</button>
                  <button className="secondary" type="button" disabled={updateBusy} onClick={() => setUpdateReview(false)}>Cancel</button>
                </div>
              </div>}
              {advanced && updateConfiguration.trusted_origin && <p className="form-note">Trusted release origin: <code>{updateConfiguration.trusted_origin}</code></p>}
            </>}
            {updateError && <p className="error-message" role="alert">{updateError}</p>}
          </div>
          <div className="provider-connections" aria-label="System diagnostics">
            <div className="section-heading compact-heading">
              <div>
                <h3>System diagnostics</h3>
                <p className="muted">Run read-only health checks and produce a secret-free support report.</p>
              </div>
              <button className="primary" type="button" disabled={diagnosticsBusy} onClick={checkSystem}>
                {diagnosticsBusy ? "Checking…" : "Run diagnostics"}
              </button>
            </div>
            {diagnosticsError && <p className="error-message" role="alert">{diagnosticsError}</p>}
            {diagnostics && <div aria-label="Diagnostic results">
              <p><strong>Overall: {diagnostics.overall.toUpperCase()}</strong> · MCG {diagnostics.version} · {diagnostics.os}</p>
              <ul className="history-list">
                {diagnostics.checks
                  .filter((check) => advanced || check.severity !== "pass")
                  .map((check) => <li key={check.code}>
                    <strong>[{check.severity.toUpperCase()}] {check.label}</strong>
                    <span>{check.detail}</span>
                    {check.remediation && <small>{check.remediation}</small>}
                  </li>)}
              </ul>
              {!advanced && diagnostics.checks.every((check) => check.severity === "pass") && <p className="empty-state">All checks passed.</p>}
              {advanced && <p className="form-note">
                Format {diagnostics.schema_version} · {diagnostics.architecture} · {diagnostics.counts.events} events · {diagnostics.counts.tasks} tasks · {diagnostics.counts.artifacts} artifacts · {diagnostics.counts.models} models · {diagnostics.counts.provider_dispatch_completed} completed provider calls · {diagnostics.counts.provider_dispatch_failed} definite provider failures · {diagnostics.counts.provider_dispatch_started} active or indeterminate provider calls
              </p>}
            </div>}
            {!diagnostics && !diagnosticsError && <p className="empty-state">Diagnostics have not been run in this session.</p>}
          </div>
              </div>
              <div className="settings-pane" data-settings-pane="import">
          <div className="provider-connections" aria-label="Portable backups">
            <div className="section-heading compact-heading">
              <div>
                <h3>Portable backups</h3>
                <p className="muted">Preserve the database and immutable artifacts. Routine backups always exclude credentials and the local authentication token.</p>
              </div>
              <button className="primary" type="button" disabled={backupBusy} onClick={makeBackup}>
                {backupBusy ? "Working…" : "Create backup"}
              </button>
            </div>
            {backupError && <p className="error-message" role="alert">{backupError}</p>}
            {backups.length === 0 ? <p className="empty-state">No portable backups yet.</p> : backups.slice().reverse().map((backup) => (
              <article className="provider-card" key={backup.backup_id}>
                <div>
                  <h3>{backup.file_name}</h3>
                  <p>{formatBytes(backup.size_bytes)} · {new Date(backup.manifest.created_at).toLocaleString()} · {backup.manifest.secrets_included ? "credentials encrypted" : "secrets excluded"}</p>
                  {advanced && <><code>{backup.sha256}</code><small>{backup.manifest.entries.length} verified entries · format {backup.manifest.schema_version}</small></>}
                </div>
                <button className="secondary" type="button" disabled={backupBusy} onClick={() => checkBackup(backup.backup_id)}>Verify</button>
              </article>
            ))}
            {advanced && <div className="provider-form" aria-label="Encrypted credential backup">
              <h3>Encrypted credential backup</h3>
              <p className="form-note">Explicitly includes provider and MCP credentials using Argon2id and authenticated XChaCha20-Poly1305 encryption. The local daemon token is never included. Losing this passphrase makes the credentials unrecoverable.</p>
              <label>Passphrase
                <input type="password" autoComplete="new-password" value={backupPassphrase} onChange={(event) => setBackupPassphrase(event.target.value)} />
              </label>
              <label>Confirm passphrase
                <input type="password" autoComplete="new-password" value={backupConfirmation} onChange={(event) => setBackupConfirmation(event.target.value)} />
              </label>
              <button className="secondary" type="button" disabled={backupBusy || backupPassphrase.length < 12} onClick={makeEncryptedBackup}>Create encrypted backup</button>
            </div>}
            <p className="form-note">Restore is intentionally offline: stop the daemon, then run <code>mcg backup restore PATH --confirm</code>. Add <code>--restore-secrets</code> only for an encrypted credential backup. MCG retains the previous database for rollback.</p>
          </div>
              </div>

              <div className="settings-pane settings-summary-pane" data-settings-pane="profile"><h2>Profile</h2><div className="settings-card"><span className="owner-avatar" aria-hidden="true">{signedInWithOpenAi ? "O" : "M"}</span><div><strong>{signedInWithOpenAi ? "OpenAI account" : "Local owner"}</strong><p>{signedInWithOpenAi ? "Your ChatGPT subscription is connected to MCG." : "Sign in with ChatGPT to use your OpenAI subscription."}</p></div><button className="secondary" type="button" onClick={() => setSettingsTab("account")}>Manage account</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="appearance"><h2>Appearance</h2><div className="settings-card"><div><strong>Theme</strong><p>MCG currently uses its Codex-aligned dark theme. System and light themes will be added here when their visual qualification is complete.</p></div><span className="count-badge">Dark</span></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="voice"><h2>Voice</h2><div className="settings-card"><div><strong>Voice input</strong><p>Voice controls are not configured in this build. MCG will not claim microphone access until the governed voice interface is available.</p></div></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="personalization"><h2>Personalization</h2><div className="settings-card"><div><strong>How MCG works with you</strong><p>Autonomy and shadow-model preferences are durable owner controls.</p></div><button className="secondary" type="button" onClick={() => setSettingsTab("general")}>Open general settings</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="shortcuts"><h2>Keyboard shortcuts</h2><dl className="shortcut-list"><div><dt>New chat</dt><dd><kbd>Ctrl</kbd> + <kbd>N</kbd></dd></div><div><dt>Open Settings</dt><dd><kbd>Ctrl</kbd> + <kbd>,</kbd></dd></div><div><dt>Open a global folder</dt><dd><kbd>Ctrl</kbd> + <kbd>O</kbd></dd></div><div><dt>Find a chat</dt><dd><kbd>Ctrl</kbd> + <kbd>F</kbd></dd></div><div><dt>Toggle sidebar</dt><dd><kbd>Ctrl</kbd> + <kbd>B</kbd></dd></div><div><dt>Toggle project file tree</dt><dd><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>E</kbd></dd></div><div><dt>Previous or next chat</dt><dd><kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>[</kbd> / <kbd>]</kbd></dd></div></dl></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="usage"><h2>Usage &amp; billing</h2><div className="settings-card"><div><strong>{proxyUsage ? `${proxyUsage.usage.total_tokens.toLocaleString()} tokens recorded` : "Usage is loading"}</strong><p>{proxyUsage ? `${proxyUsage.call_count.toLocaleString()} calls across ${proxyUsage.conversation_count.toLocaleString()} conversations. Known API cost: $${proxyUsage.known_actual_cost_usd.toFixed(6)}.` : "MCG keeps credential-free local usage totals."}</p><small>ChatGPT subscription usage is governed by OpenAI and is not converted into API charges here. API-key providers remain separately billed.</small></div><button className="secondary" type="button" onClick={() => navigateSurface("providers")}>Provider details</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="account"><h2>Account</h2><div className="settings-card account-card"><span className="owner-avatar" aria-hidden="true">{signedInWithOpenAi ? "O" : "M"}</span><div><strong>{signedInWithOpenAi ? "Signed in with ChatGPT" : "Connect your OpenAI subscription"}</strong><p>{signedInWithOpenAi ? "MCG uses the explicit ChatGPT OAuth connection for subscription access." : "A browser window will open for OpenAI sign-in and return authorization to MCG."}</p><small>Subscription OAuth never silently falls back to API-key billing.</small></div>{signedInWithOpenAi ? <button className="secondary" type="button" disabled={busy} onClick={() => void logOutOfOpenAi()}>{busy ? "Logging out…" : "Log out"}</button> : <button className="primary" type="button" disabled={busy || Boolean(login)} onClick={() => void startLogin("browser")}>{busy ? "Opening sign-in…" : "Sign in with ChatGPT"}</button>}</div>
                {login && <div className="oauth-card" role="status"><p>Finish signing in with OpenAI in your browser, then return here.</p><button className="link-button" type="button" onClick={() => openVerification(login.verification_url)}>Open sign-in page</button><div className="composer-actions"><button className="secondary" type="button" disabled={busy} onClick={cancelLogin}>Cancel</button><button className="primary" type="button" disabled={busy} onClick={finishLogin}>{busy ? "Checking…" : "Finish connection"}</button></div></div>}
                <p className="provider-state" role="status">{providerStatus}</p>{providerError && <p className="error-message" role="alert">{providerError}</p>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="plugins"><h2>Plugins</h2><div className="settings-card"><div><strong>Plugin directory and access</strong><p>Browse installed, workspace, personal, and OpenAI plugin sources. Review access before activation.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("plugins")}>Open Plugins</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="browser"><h2>Browser</h2><div className="settings-card"><div><strong>Governed browser sessions</strong><p>Review browser authority, boundaries, and active sessions.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("platform")}>Open Browser settings</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="computer"><h2>Computer use</h2><div className="settings-card"><div><strong>Governed computer access</strong><p>Review explicit input authority and current computer sessions.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("platform")}>Open Computer use</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="hooks"><h2>Hooks</h2><div className="settings-card"><div><strong>No hooks configured</strong><p>Hook management will appear here when the typed, governed hook runtime is implemented.</p></div></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="connections"><h2>Connections</h2><div className="settings-card"><div><strong>Providers and tools</strong><p>Manage model providers, ChatGPT subscription access, MCP servers, and remote hosts.</p></div><div className="settings-card-actions"><button className="secondary" type="button" onClick={() => navigateSurface("providers")}>Providers</button><button className="secondary" type="button" onClick={() => navigateSurface("mcp")}>Tools</button><button className="secondary" type="button" onClick={() => navigateSurface("ssh")}>SSH</button></div></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="git"><h2>Git</h2><div className="settings-card"><div><strong>Repository workflows</strong><p>Projects retain repository identity; pull-request review is currently local and read-only.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("pull-requests")}>Open reviews</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="environments"><h2>Environments</h2><div className="settings-card"><div><strong>Project environments</strong><p>Environment details are derived from each attached project. Exact runtime controls remain in Advanced mode.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("projects")}>Manage projects</button></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="worktrees"><h2>Worktrees</h2><div className="settings-card"><div><strong>Isolated worktrees</strong><p>Automated isolated worktree management is not yet exposed as a consumer control.</p></div></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="archived"><h2>Archived chats</h2><div className="settings-card"><div><strong>No archived chats</strong><p>Archived chat storage and restore controls will appear here when that durable lifecycle is implemented.</p></div></div></div>
            </div>
          </div>
        </section>
      </main>
    </div>
    </div>
  );
}
