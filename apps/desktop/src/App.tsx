import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Activity,
  Archive,
  ArrowUp,
  ArrowLeft,
  ArrowRight,
  Blocks,
  BrainCircuit,
  Braces,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Cloud,
  CloudDownload,
  Command,
  Copy,
  Cpu,
  ExternalLink,
  Files,
  FolderOpen,
  Gauge,
  GitBranch,
  GitFork,
  GitPullRequest,
  Globe2,
  Hand,
  Home,
  Laptop,
  ListTree,
  ListChecks,
  LockKeyhole,
  Maximize2,
  Minus,
  PanelBottom,
  PanelLeft,
  PanelRight,
  PanelsTopLeft,
  Pause,
  Pin,
  Play,
  Plus,
  RotateCw,
  Search,
  Server,
  File,
  Folder,
  Plug,
  Share2,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  SquarePen,
  Terminal,
  Target,
  Trash2,
  Users,
  Workflow,
  X
} from "lucide-react";
import { MarkdownContent } from "./MarkdownContent";
import metaCogLogo from "./assets/mcg-logo.png";
import { scrollChainByDelta, wheelDeltaToPixels } from "./scroll-chain";

export const CHAT_FOLLOW_THRESHOLD_PX = 96;
const CHATGPT_SITES_LIBRARY_ENABLED = true;
const DEVELOPER_GOVERNANCE_WARNING = "Developer mode remains governed: protected policy, evaluator fixtures, Root authority, signing keys, secrets, direct database writes, and trust bypasses are unavailable or read-only.";

export function isChatViewportNearBottom(
  viewport: Pick<HTMLElement, "scrollHeight" | "scrollTop" | "clientHeight">,
  threshold = CHAT_FOLLOW_THRESHOLD_PX
) {
  return viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <= threshold;
}

function handleScrollChainWheel(event: WheelEvent) {
  if (event.ctrlKey || event.shiftKey || event.deltaY === 0 || !(event.target instanceof Element)) return;
  const origin = event.target.closest<HTMLElement>("[data-scroll-chain]");
  const root = event.currentTarget;
  if (!origin || !(root instanceof HTMLElement) || !root.contains(origin)) return;
  const deltaY = wheelDeltaToPixels(event.deltaY, event.deltaMode, origin.clientHeight);
  scrollChainByDelta(origin, deltaY);
  event.preventDefault();
}

function comparableSiteUrl(value: string) {
  try {
    const url = new URL(value);
    url.hash = "";
    url.search = "";
    return url.toString().replace(/\/$/, "").toLocaleLowerCase();
  } catch {
    return value.trim().replace(/\/$/, "").toLocaleLowerCase();
  }
}

function isChatGptSiteUrl(value: string) {
  try {
    const hostname = new URL(value).hostname.toLocaleLowerCase();
    return hostname === "chatgpt.site" || hostname.endsWith(".chatgpt.site");
  } catch {
    return false;
  }
}

import {
  approveRequest,
  applyCloudDiff,
  discardCloudDiffApply,
  applyComputerActions,
  attachBrowserSession,
  attachComputerSession,
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
  createConversation,
  archiveConversation,
  archiveProject,
  createTask,
  cancelAgentRun,
  deploySite,
  denyRequest,
  discoverProject,
  githubStatus,
  startGitHubLogin,
  completeGitHubLogin,
  disconnectGitHub,
  listGitHubRepositories,
  createGitHubRepository,
  linkGitHubRepository,
  discoverMcpConnection,
  discoverProvider,
  disconnectProvider,
  exportOrganization,
  handoffBrowserSession,
  handoffComputerSession,
  inheritOrganization,
  initializeProjectGit,
  importOrganization,
  importSharedOrganization,
  listMcpConnections,
  listOpenInApplications,
  listOrganizationPublishers,
  listOrganizationShareReviews,
  listOrganizations,
  listAgentRuns,
  listGlobalAgentSessions,
  listGovernorProposals,
  listManagedWorktrees,
  listCloudDiffApplies,
  listPlugins,
  listExperiments,
  listSupervisionObservations,
  listProviders,
  providerProxyConversations,
  providerProxyUsage,
  replayAgentRun,
  listApprovals,
  getApproval,
  listBackups,
  listSchedules,
  listSchedulerNotifications,
  listProjects,
  removeProject,
  listProjectMemory,
  listProjectMemoryTransfers,
  listReviewedTopologyPlans,
  projectTopologyPlan,
  prepareCloudDiffApply,
  listPlatformSessions,
  listCodexSites,
  listSites,
  listSshHosts,
  listSshWorkflows,
  listTasks,
  listConversations,
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
  getProtectedRootStatus,
  listProtectedRootWorkflows,
  getProtectedRootWorkflow,
  getProtectedRootWorkflowActivity,
  listProtectedRootActions,
  getProtectedRootAction,
  compensateProtectedRootAction,
  listProtectedRootInvocations,
  getProtectedRootInvocation,
  listProtectedRootCapabilities,
  getProtectedRootCapability,
  cancelProtectedRootWorkflow,
  getGovernorOverview,
  getGovernorControlPolicy,
  getConversation,
  updateConversationMode,
  updateConversationGoal,
  clearConversationGoal,
  answerConversationInput,
  updateConversation,
  getCodexModels,
  getCodexFeatures,
  getCodexPermissionProfiles,
  setCodexFeature,
  getCodexAccount,
  getCodexRateLimits,
  getCodexHarnessStatus,
  getCodexConfigurationStatus,
  setupWindowsSandbox,
  grantWindowsSandboxReadDirectory,
  getCodexPendingRequests,
  getCodexStableDiscovery,
  updateCodexHookState,
  reconcileOwnedCodexThread,
  forkOwnedCodexThread,
  compactOwnedCodexThread,
  watchConversationProject,
  unwatchConversationProject,
  startCodexLogin,
  updateCodexTransport,
  cancelCodexLogin,
  logoutCodexAccount,
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
  openSiteInAppBrowser,
  closeSiteInAppBrowser,
  controlSiteInAppBrowser,
  openSiteExternalUrl,
  updateSiteInAppBrowser,
  openWorkspaceIn,
  type OAuthLoginStart,
  type OpenInApplication,
  type OAuthAccountProfile,
  type CodexUsageWindow,
  type OrganizationGenome,
  type OrganizationAssignments,
  type OrganizationRevisionPreview,
  type AgentRunView,
  type GovernorOverview,
  type GovernorControlPolicy,
  type GovernorProposal,
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
  type ChatRunPreferences,
  type ApprovalMode,
  type GitHubConnectionStatus,
  type GitHubRepository,
  type Conversation,
  type ConversationActivity,
  type ConversationAgentSession,
  type GlobalAgentSessionSummary,
  type AgentMessage,
  type AgentDelegation,
  type ConversationTeamState,
  type CoordinationBatch,
  type ConversationPreview,
  type ConversationMessage,
  type ConversationTurn,
  type ConversationView,
  type ConversationInputRequest,
  type ManagedWorktree,
  type CloudDiffApply,
  type CodexModel,
  type CodexFeature,
  type CodexPermissionProfile,
  type CodexAccount,
  type CodexRateLimits,
  type CodexHarnessStatus,
  type CodexConfigurationStatus,
  type CodexPendingRequest,
  type CodexStableDiscovery,
  type CodexHookMetadata,
  type CodexHooksListEntry,
  type CodexAppInfo,
  type CodexLoginStart,
  type ReasoningEffort,
  type ExecutionSpeed,
  type TeamPreset,
  type ProxyConversation,
  type ProxyUsageSummary,
  runTerminalCommand,
  type TerminalCommandResult,
  type PlatformCapabilitySnapshot,
  type PlatformSession,
  type PlatformSessionHistoryResponse,
  type PlatformSessionKind,
  type ComputerUseFrame,
  type PersistentBrowserObservation,
  pickProjectDirectory,
  pickMentionFiles,
  pickMentionFolder,
  pickWritableFolder,
  pickSandboxReadFolder,
  searchProjectMentions,
  type ProjectModel,
  type MentionCandidate,
  type ComposerMentionInput,
  type ConversationMention,
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
  type CodexSiteInventoryItem,
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
  sendConversationMessage,
  reviewConversationDelegation,
  cancelConversationDelegation,
  retryConversationDelegation,
  applyConversationDelegation,
  discardConversationDelegation,
  startConversationPreview,
  stopConversationPreview,
  steerConversationTurn,
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
  setGlobalOrganizationAssignment,
  setProjectOrganizationAssignment,
  previewOrganizationRevision,
  publishOrganizationRevision,
  type TaskEnvelope,
  type ApprovalRequest,
  type BackupSummary,
  type DiagnosticReport,
  type LocalServiceStatus,
  type ProtectedRootStatus,
  type CanonicalWorkflowAttempt,
  type CanonicalWorkflowInspection,
  type CanonicalWorkflowInvocationActivity,
  type CanonicalActionSummary,
  type CanonicalActionInspection,
  type CanonicalSourceRef,
  type CanonicalInvocationSummary,
  type CanonicalInvocationInspection,
  type ToolAuthorizationSummary,
  type ToolAuthorizationInspection,
  type CanonicalModelRunState,
  type ActionExecutionState,
  type WorkflowState,
  type UpdateCheck,
  type UpdateConfiguration,
  type TaskEventsResponse,
  type ToolAuditResponse,
  type InterfaceComplexity,
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
  decideGovernorProposal,
  requestGovernorImplementation,
  wakeSchedule
} from "./desktop-client";
import { parseSlashCommand, slashCommandMatches, type SlashCommandName } from "./slash-commands";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function workflowStateKey(state: WorkflowState): string {
  return typeof state === "string" ? state : `waiting_${state.waiting}`;
}

function workflowStateLabel(state: WorkflowState): string {
  if (typeof state !== "string") {
    const reason = state.waiting.replaceAll("_", " ");
    return state.waiting === "reconciliation" ? "Recovering" : `Blocked · ${reason}`;
  }
  return ({
    ready: "Preparing",
    running: "Working",
    verifying: "Verifying",
    succeeded: "Completed",
    failed: "Failed",
    cancel_requested: "Stopping safely",
    cancelled: "Cancelled",
    quarantined: "Blocked · quarantined"
  } as Record<string, string>)[state] ?? state.replaceAll("_", " ");
}

function workflowCanRequestCancellation(state: WorkflowState): boolean {
  return state === "running" || state === "verifying" || typeof state === "object";
}

function actionStateLabel(state: ActionExecutionState): string {
  return ({
    proposed: "Proposed",
    authorized: "Approved and ready",
    executing: "Working",
    observed: "Checking result",
    no_effect_confirmed: "No change made",
    effect_confirmed: "Change confirmed",
    outcome_unknown: "Checking what happened",
    reconciling: "Recovering safely",
    validated: "Verified",
    committed: "Completed",
    denied: "Not allowed",
    quarantined: "Needs attention",
    compensated: "Reverted"
  } as Record<ActionExecutionState, string>)[state];
}

function invocationStateLabel(state: CanonicalModelRunState | null): string {
  if (state === null) return "Preparing";
  return ({
    planned: "Preparing",
    dispatched: "Working",
    completed: "Completed",
    failed: "Failed",
    cancelled: "Cancelled",
    outcome_unknown: "Recovering"
  } as Record<CanonicalModelRunState, string>)[state];
}

function codexFeatureStageLabel(stage: CodexFeature["stage"]): string {
  if (stage === "underDevelopment") return "Under development";
  return stage.charAt(0).toUpperCase() + stage.slice(1);
}

function chatIdForTask(task: TaskEnvelope): string {
  return task.thread_id ?? task.id;
}

type ChatMenuSource = "pinned" | "project" | "recent";
type ChatContextMenu = { chatId: string; source: ChatMenuSource; x: number; y: number } | null;
type SiteLibraryEntry = {
  key: string;
  displayName: string;
  description?: string;
  publicUrl: string;
  screenshotUrl?: string;
  localSite?: Site;
  accountSite?: CodexSiteInventoryItem;
};
type SiteContextMenu = { key: string; x: number; y: number } | null;
type SiteBrowserTab = { id: string; title: string; url: string | null };

function GovernedBrowserPreview({ sessionId, url, title }: { sessionId: string; url: string; title: string }) {
  const [loaded, setLoaded] = useState(false);
  const previewSource = "__TAURI_INTERNALS__" in window ? url : "about:blank";

  useEffect(() => setLoaded(false), [url]);

  return <div className="governed-browser-preview" data-session-id={sessionId}>
    <div className="browser-preview-status" role="status">
      <span className={loaded ? "ready" : "loading"} aria-hidden="true" />
      {loaded ? "Live preview" : "Loading preview…"}
    </div>
    <iframe
      key={url}
      title={`Browser preview · ${title}`}
      src={previewSource}
      data-preview-url={url}
      sandbox="allow-downloads allow-forms allow-modals allow-popups allow-same-origin allow-scripts"
      referrerPolicy="no-referrer"
      onLoad={() => setLoaded(true)}
    />
    <small>If a site blocks embedded viewing, use Open in to launch it in your browser.</small>
  </div>;
}

function EmbeddedSiteBrowser({ tabId, url, onError }: { tabId: string; url: string; onError: (message: string) => void }) {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let disposed = false;
    let opened = false;
    let frame = 0;

    const sync = async () => {
      const host = hostRef.current;
      if (!host || disposed) return;
      const rect = host.getBoundingClientRect();
      const bounds = {
        x: Math.max(0, rect.x),
        y: Math.max(0, rect.y),
        width: Math.max(1, rect.width),
        height: Math.max(1, rect.height)
      };
      try {
        if (!opened) {
          opened = true;
          await openSiteInAppBrowser(tabId, url, bounds);
        } else {
          await updateSiteInAppBrowser(tabId, bounds, true);
        }
      } catch (error) {
        opened = false;
        if (!disposed) onError(errorMessage(error));
      }
    };
    const scheduleSync = () => {
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => void sync());
    };
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(scheduleSync);
    if (hostRef.current) observer?.observe(hostRef.current);
    window.addEventListener("resize", scheduleSync);
    window.addEventListener("scroll", scheduleSync, true);
    scheduleSync();

    return () => {
      disposed = true;
      window.cancelAnimationFrame(frame);
      observer?.disconnect();
      window.removeEventListener("resize", scheduleSync);
      window.removeEventListener("scroll", scheduleSync, true);
      void updateSiteInAppBrowser(tabId, null, false).catch(() => undefined);
    };
  }, [tabId, url, onError]);

  return <div className="embedded-site-browser">
    <div ref={hostRef} className="embedded-site-browser-host" aria-label="MetaCog browser content" />
  </div>;
}

function PinIcon({ solid = false }: { solid?: boolean }) {
  return <Pin aria-hidden="true" fill={solid ? "currentColor" : "none"} />;
}

function ArchiveIcon() {
  return <Archive aria-hidden="true" />;
}

function TrashIcon() {
  return <Trash2 aria-hidden="true" />;
}

function assistantTextForRun(run?: AgentRunView): string | null {
  if (!run) return null;
  const parts = (run.results ?? []).flatMap((result) => (result.response?.output ?? []).flatMap((item) => {
    if ("text" in item && item.type === "refusal") return [String(item.text)];
    if (!("content" in item) || item.type !== "message" || !Array.isArray(item.content)) return [];
    return item.content.flatMap((part: unknown) => {
      if (!part || typeof part !== "object") return [];
      return "type" in part && part.type === "text" && "text" in part ? [String(part.text)] : [];
    });
  }));
  const text = parts.join("\n\n").trim();
  return text || null;
}

const DEFAULT_CHAT_RUN_PREFERENCES: ChatRunPreferences = {
  reasoning_effort: "medium",
  speed: "standard",
  approval_mode: "ask",
  codex_feature_overrides: {},
  codex_config_overrides: [],
  strict_config: false,
  team_preset: "automatic"
};

const MODEL_CATALOG_MAX_AGE_MS = 24 * 60 * 60 * 1000;

function modelCatalogNeedsDiscovery(provider: ProviderListItem): boolean {
  if (!provider.catalog || provider.catalog.models.length === 0) return true;
  const discoveredAt = Date.parse(provider.catalog.discovered_at);
  return !Number.isFinite(discoveredAt) || Date.now() - discoveredAt > MODEL_CATALOG_MAX_AGE_MS;
}

function storedChatPreferences(key: string): ChatRunPreferences {
  try {
    const stored = window.localStorage.getItem(key);
    if (!stored) return DEFAULT_CHAT_RUN_PREFERENCES;
    const parsed = JSON.parse(stored) as Partial<ChatRunPreferences>;
    return {
      provider_id: parsed.provider_id,
      model_provider: parsed.model_provider,
      model_id: parsed.model_id,
      model_label: parsed.model_label,
      reasoning_effort: parsed.reasoning_effort ?? "medium",
      speed: parsed.speed ?? "standard",
      service_tier: parsed.service_tier,
      approval_mode: parsed.approval_mode ?? "ask",
      approval_policy: parsed.approval_policy,
      sandbox_mode: parsed.sandbox_mode,
      permission_profile: parsed.permission_profile,
      additional_writable_roots: parsed.additional_writable_roots ?? [],
      codex_feature_overrides: parsed.codex_feature_overrides ?? {},
      codex_config_overrides: parsed.codex_config_overrides ?? [],
      codex_profile: parsed.codex_profile,
      strict_config: parsed.strict_config ?? false,
      output_schema: parsed.output_schema,
      team_preset: parsed.team_preset ?? "automatic",
      personality: parsed.personality
    };
  } catch {
    return DEFAULT_CHAT_RUN_PREFERENCES;
  }
}

const CHAT_QUEUE_STORAGE_KEY = "mcg.chat-message-queue";

interface QueuedChatMessage {
  id: string;
  conversation_id: string;
  content: string;
  preferences: ChatRunPreferences;
  mentions: ComposerMentionInput[];
  created_at: string;
}

interface PendingComposerMention {
  id: string;
  candidate: MentionCandidate;
  token: string;
}

interface MentionTrigger {
  start: number;
  end: number;
  query: string;
}

function mentionTrigger(value: string, caret = value.length): MentionTrigger | null {
  const before = value.slice(0, caret);
  const match = before.match(/(?:^|\s)@([^\s@]*)$/u);
  if (!match || match.index === undefined) return null;
  const prefix = match[0].startsWith("@") ? 0 : 1;
  return { start: match.index + prefix, end: caret, query: match[1] ?? "" };
}

function appReferenceTrigger(value: string, caret = value.length): MentionTrigger | null {
  const before = value.slice(0, caret);
  const command = before.match(/^\/apps(?:\s+([^\s$]*))?$/u);
  if (command) return { start: 0, end: caret, query: command[1] ?? "" };
  const match = before.match(/(?:^|\s)\$([^\s$]*)$/u);
  if (!match || match.index === undefined) return null;
  const prefix = match[0].startsWith("$") ? 0 : 1;
  return { start: match.index + prefix, end: caret, query: match[1] ?? "" };
}

function utf8Length(value: string): number { return new TextEncoder().encode(value).length; }

function materializeComposerMentions(content: string, pending: PendingComposerMention[]): ComposerMentionInput[] {
  const mentions: ComposerMentionInput[] = [];
  let characterOffset = 0;
  const seen = new Set<string>();
  for (const item of pending) {
    const characterStart = content.indexOf(item.token, characterOffset);
    if (characterStart < 0) continue;
    const identity = `${item.candidate.kind}:${item.candidate.resource}`;
    if (seen.has(identity)) continue;
    seen.add(identity);
    const characterEnd = characterStart + item.token.length;
    mentions.push({
      kind: item.candidate.kind,
      label: item.candidate.label,
      resource: item.candidate.resource,
      byte_start: utf8Length(content.slice(0, characterStart)),
      byte_end: utf8Length(content.slice(0, characterEnd)),
      plugin_revision_id: item.candidate.plugin_revision_id,
      installation_revision: item.candidate.installation_revision
    });
    characterOffset = characterEnd;
  }
  return mentions;
}

function storedChatQueue(): QueuedChatMessage[] {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(CHAT_QUEUE_STORAGE_KEY) || "[]") as Partial<QueuedChatMessage>[];
    return parsed.filter((item): item is QueuedChatMessage => Boolean(
      item.id && item.conversation_id && item.content?.trim() && item.preferences && item.created_at
    )).map((item) => ({ ...item, mentions: Array.isArray(item.mentions) ? item.mentions : [] }));
  } catch {
    return [];
  }
}

const APPROVAL_MODE_DETAILS: Record<ApprovalMode, { label: string; description: string }> = {
  ask: {
    label: "Ask for approval",
    description: "Ask before editing outside the workspace or using restricted capabilities."
  },
  auto_review: {
    label: "Approve for me",
    description: "Codex reviews requests and only asks you when an action may be unsafe."
  },
  read_only: {
    label: "Read only",
    description: "Inspect the project without allowing file changes or unrestricted commands."
  },
  full_access: {
    label: "Full access",
    description: "Allow internet access and any file on this computer without asking."
  }
};

const TRANSIENT_DETAILS_SELECTOR = "details[data-transient-popover]";

function closeTransientDetails(except?: Element | null) {
  const closed: HTMLDetailsElement[] = [];
  document.querySelectorAll<HTMLDetailsElement>(`${TRANSIENT_DETAILS_SELECTOR}[open]`).forEach((details) => {
    if (except && details.contains(except)) return;
    details.removeAttribute("open");
    closed.push(details);
  });
  return closed;
}

function ApprovalModeIcon({ mode }: { mode: ApprovalMode }) {
  if (mode === "ask") return <Hand aria-hidden="true" />;
  if (mode === "auto_review") return <ShieldCheck aria-hidden="true" />;
  if (mode === "read_only") return <LockKeyhole aria-hidden="true" />;
  return <ShieldAlert aria-hidden="true" />;
}

function PermissionSelector({
  value,
  onChange,
  hasProject,
  permissionProfile,
  permissionProfiles = [],
  advanced = false,
  onPermissionProfileChange
}: {
  value: ApprovalMode;
  onChange: (mode: ApprovalMode) => void;
  hasProject: boolean;
  permissionProfile?: string;
  permissionProfiles?: CodexPermissionProfile[];
  advanced?: boolean;
  onPermissionProfileChange: (profile?: string) => void;
}) {
  const selected = APPROVAL_MODE_DETAILS[value];
  return <details className={`permission-selector permission-${value}`} data-transient-popover>
    <summary aria-label="Change permissions" title="Change permissions">
      <ApprovalModeIcon mode={value} />
      <span>{permissionProfile ?? selected.label}</span>
      <ChevronDown aria-hidden="true" />
    </summary>
    <div className="permission-popover" data-scroll-chain role="menu" aria-label="How should MetaCog actions be approved?">
      <div className="permission-popover-heading">
        <strong>How should MetaCog actions be approved?</strong>
        <small>Applies to the next message and future turns in this chat.</small>
      </div>
      {(Object.entries(APPROVAL_MODE_DETAILS) as [ApprovalMode, (typeof APPROVAL_MODE_DETAILS)[ApprovalMode]][]).map(([mode, detail]) => {
        return <button
          type="button"
          role="menuitemradio"
          aria-checked={value === mode}
          className={mode === "full_access" ? "danger-option" : ""}
          key={mode}
          onClick={(event) => {
            onPermissionProfileChange(undefined);
            onChange(mode);
            event.currentTarget.closest("details")?.removeAttribute("open");
          }}
        >
          <span className="permission-option-icon"><ApprovalModeIcon mode={mode} /></span>
          <span><strong>{detail.label}</strong><small>{detail.description}</small></span>
          <span className="permission-option-check" aria-hidden="true">{value === mode && <Check />}</span>
        </button>;
      })}
      {advanced && hasProject && permissionProfiles.some((profile) => profile.allowed) && <>
        <div className="permission-popover-heading permission-profile-heading">
          <strong>Codex permission profiles</strong>
          <small>Profiles come from this project's effective Codex configuration. MetaCog governance remains authoritative.</small>
        </div>
        {permissionProfiles.filter((profile) => profile.allowed).map((profile) => <button
          type="button"
          role="menuitemradio"
          aria-checked={permissionProfile === profile.id}
          key={profile.id}
          onClick={(event) => {
            onPermissionProfileChange(profile.id);
            event.currentTarget.closest("details")?.removeAttribute("open");
          }}
        >
          <span className="permission-option-icon"><ShieldCheck aria-hidden="true" /></span>
          <span><strong>{profile.id}</strong><small>{profile.description || "Codex permission profile"}</small></span>
          <span className="permission-option-check" aria-hidden="true">{permissionProfile === profile.id && <Check />}</span>
        </button>)}
      </>}
      {!hasProject && <p>Your selection is saved. Project-free turns stay read-only until you attach a project.</p>}
    </div>
  </details>;
}

const TEAM_PRESET_DETAILS: Record<TeamPreset, { label: string; description: string }> = {
  automatic: { label: "Automatic", description: "MetaCog chooses the smallest suitable team." },
  fast: { label: "Fast", description: "Use at most one parallel specialist." },
  balanced: { label: "Balanced", description: "Use up to three parallel specialists." },
  thorough: { label: "Thorough", description: "Use the assigned team's capacity, up to eight." }
};

function TeamPresetSelector({ value = "automatic", onChange }: { value?: TeamPreset; onChange: (preset: TeamPreset) => void }) {
  const selectedValue = TEAM_PRESET_DETAILS[value] ? value : "automatic";
  const selected = TEAM_PRESET_DETAILS[selectedValue];
  return <details className="team-preset-selector" data-transient-popover>
    <summary aria-label="Change team preset" title={selected.description}>
      <Users aria-hidden="true" /><span>{selected.label}</span><ChevronDown aria-hidden="true" />
    </summary>
    <div className="team-preset-popover" role="menu" aria-label="Team preset">
      <strong>Team preset</strong>
      {(Object.entries(TEAM_PRESET_DETAILS) as [TeamPreset, (typeof TEAM_PRESET_DETAILS)[TeamPreset]][]).map(([preset, detail]) => <button
        type="button"
        role="menuitemradio"
        aria-checked={selectedValue === preset}
        key={preset}
        onClick={(event) => { onChange(preset); event.currentTarget.closest("details")?.removeAttribute("open"); }}
      ><span><strong>{detail.label}</strong><small>{detail.description}</small></span><span aria-hidden="true">{selectedValue === preset && <Check />}</span></button>)}
    </div>
  </details>;
}

function projectDisplayName(project: ProjectModel): string {
  return project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root;
}

function NewChatProjectSelector({
  projects,
  value,
  onChange,
  onOpenFolder,
  disabled
}: {
  projects: ProjectModel[];
  value: ProjectModel | null;
  onChange: (project: ProjectModel | null) => void;
  onOpenFolder: () => void;
  disabled: boolean;
}) {
  const [query, setQuery] = useState("");
  const visibleProjects = projects.filter((project) => projectDisplayName(project).toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  return <details className="new-chat-context-selector new-chat-project-selector" data-transient-popover onToggle={(event) => {
    if (!(event.currentTarget as HTMLDetailsElement).open) setQuery("");
  }}>
    <summary aria-labelledby="project-picker-label" role="button" title={value?.root ?? "Choose a project for this chat"}>
      <span className="sr-only" id="project-picker-label">Choose project</span><FolderOpen aria-hidden="true" /><span aria-hidden="true">{value ? projectDisplayName(value) : "Choose project"}</span><ChevronDown aria-hidden="true" />
    </summary>
    <div className="new-chat-context-popover project-context-popover">
      {projects.length > 0 && <label className="context-search"><Search aria-hidden="true" /><input aria-label="Search projects" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search projects" /></label>}
      <div className="context-option-list" role="menu" aria-label="Project choices">
        {visibleProjects.map((project) => <button
          type="button"
          role="menuitemradio"
          aria-checked={value?.project_id === project.project_id}
          key={project.project_id}
          onClick={(event) => {
            onChange(project);
            event.currentTarget.closest("details")?.removeAttribute("open");
          }}
        ><FolderOpen aria-hidden="true" /><span><strong>{projectDisplayName(project)}</strong><small>{project.repository.is_git ? project.repository.branch ?? "Git repository" : "Local project"}</small></span><span className="context-option-check" aria-hidden="true">{value?.project_id === project.project_id && <Check />}</span></button>)}
        {projects.length > 0 && visibleProjects.length === 0 && <p>No projects match “{query}”.</p>}
      </div>
      <div className="context-popover-actions">
        <button type="button" disabled={disabled} onClick={(event) => {
          onOpenFolder();
          event.currentTarget.closest("details")?.removeAttribute("open");
        }}><Plus aria-hidden="true" /><span><strong>Open project folder</strong><small>Add or rediscover a local project</small></span></button>
        <button type="button" disabled={!value} onClick={(event) => {
          onChange(null);
          event.currentTarget.closest("details")?.removeAttribute("open");
        }}><X aria-hidden="true" /><span><strong>Don’t work in a project</strong><small>Keep this chat project-free and read-only</small></span></button>
      </div>
    </div>
  </details>;
}

function ActiveChatProjectSelector({ project }: { project: ProjectModel | null }) {
  const name = project ? projectDisplayName(project) : "No project";
  return <details className="new-chat-context-selector new-chat-project-selector active-chat-project-selector" data-transient-popover>
    <summary aria-label={`Project: ${name}`} title={project?.root ?? "This conversation is project-free"}>
      <FolderOpen aria-hidden="true" /><span>{name}</span><ChevronDown aria-hidden="true" />
    </summary>
    <div className="new-chat-context-popover project-context-popover" role="menu" aria-label="Chat project">
      <div className="context-popover-heading">Conversation project</div>
      <button type="button" role="menuitemradio" aria-checked="true" onClick={(event) => event.currentTarget.closest("details")?.removeAttribute("open")}>
        <FolderOpen aria-hidden="true" /><span><strong>{name}</strong><small>{project ? project.root : "Read-only until a project is attached to a new chat"}</small></span><span className="context-option-check" aria-hidden="true"><Check /></span>
      </button>
      <p>A conversation keeps its original project. Use the Add menu to start another chat with a different workspace.</p>
    </div>
  </details>;
}

function NewChatWorkspaceSelector() {
  return <details className="new-chat-context-selector new-chat-workspace-selector" data-transient-popover>
    <summary aria-label="Workspace: Local" title="Work on this device"><Laptop aria-hidden="true" /><span>Local</span><ChevronDown aria-hidden="true" /></summary>
    <div className="new-chat-context-popover workspace-context-popover" role="menu" aria-label="Work in">
      <div className="context-popover-heading">Work in</div>
      <button type="button" role="menuitemradio" aria-checked="true" onClick={(event) => event.currentTarget.closest("details")?.removeAttribute("open")}><Laptop aria-hidden="true" /><span><strong>Local</strong><small>Use this device</small></span><span className="context-option-check" aria-hidden="true"><Check /></span></button>
      <button type="button" role="menuitem" disabled><GitFork aria-hidden="true" /><span><strong>New local worktree</strong><small>Coding agents create managed worktrees when needed</small></span></button>
      <button type="button" role="menuitem" disabled><Cloud aria-hidden="true" /><span><strong>Cloud</strong><small>No cloud workspace is configured</small></span></button>
    </div>
  </details>;
}

function NewChatBranchSelector({ project }: { project: ProjectModel }) {
  const branch = project.repository.branch ?? "Current branch";
  return <details className="new-chat-context-selector new-chat-branch-selector" data-transient-popover>
    <summary aria-label={`Branch: ${branch}`} title={branch}><GitBranch aria-hidden="true" /><span>{branch}</span><ChevronDown aria-hidden="true" /></summary>
    <div className="new-chat-context-popover branch-context-popover" role="menu" aria-label="Git branch">
      <div className="context-popover-heading">Branch</div>
      <button type="button" role="menuitemradio" aria-checked="true" onClick={(event) => event.currentTarget.closest("details")?.removeAttribute("open")}><GitBranch aria-hidden="true" /><span><strong>{branch}</strong><small>{project.repository.head ? `HEAD ${project.repository.head.slice(0, 10)}` : "Current checkout"}</small></span><span className="context-option-check" aria-hidden="true"><Check /></span></button>
      <p>Branch changes remain governed by the project’s Git workflow.</p>
    </div>
  </details>;
}

function NewChatAddMenu({ project, disabled, onOpenFolder, onClearProject, onPlan, onGoal, onCommands, onAttachFiles, onAttachFolder, onApps, onPlugins, existingChat = false }: { project: ProjectModel | null; disabled: boolean; onOpenFolder: () => void; onClearProject: () => void; onPlan: () => void; onGoal: () => void; onCommands: () => void; onAttachFiles: () => void; onAttachFolder: () => void; onApps: () => void; onPlugins: () => void; existingChat?: boolean }) {
  return <details className="new-chat-add-menu" data-transient-popover>
    <summary aria-label="Add files and more" title="Add files and more"><Plus aria-hidden="true" /></summary>
    <div className="new-chat-add-popover" role="menu" aria-label="Add">
      <div className="context-popover-heading">Add</div>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => { onAttachFiles(); event.currentTarget.closest("details")?.removeAttribute("open"); }}><File aria-hidden="true" /><span><strong>Files</strong><small>Attach durable file snapshots to this message</small></span></button>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => { onAttachFolder(); event.currentTarget.closest("details")?.removeAttribute("open"); }}><Folder aria-hidden="true" /><span><strong>Folder</strong><small>Attach a bounded read-only folder reference</small></span></button>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => { onApps(); event.currentTarget.closest("details")?.removeAttribute("open"); }}><Blocks aria-hidden="true" /><span><strong>Apps</strong><small>Insert an enabled Codex app reference</small></span></button>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => { onPlugins(); event.currentTarget.closest("details")?.removeAttribute("open"); }}><Plug aria-hidden="true" /><span><strong>Plugins</strong><small>Mention an enabled plugin without invoking it</small></span></button>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => { onGoal(); event.currentTarget.closest("details")?.removeAttribute("open"); }}><Target aria-hidden="true" /><span><strong>Goal</strong><small>Set a durable objective to keep pursuing</small></span></button>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => { onPlan(); event.currentTarget.closest("details")?.removeAttribute("open"); }}><ListChecks aria-hidden="true" /><span><strong>Plan mode</strong><small>Research, ask choices, and create a decision-complete plan</small></span></button>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => { onCommands(); event.currentTarget.closest("details")?.removeAttribute("open"); }}><Command aria-hidden="true" /><span><strong>Slash commands</strong><small>Browse every Codex-aligned chat command</small></span></button>
      <button type="button" role="menuitem" disabled={disabled} onClick={(event) => {
        onOpenFolder();
        event.currentTarget.closest("details")?.removeAttribute("open");
      }}><FolderOpen aria-hidden="true" /><span><strong>{existingChat ? "Start a project chat" : "Open project folder"}</strong><small>{existingChat ? "Choose another local workspace in a new chat" : "Work with files in a local project"}</small></span></button>
      <button type="button" role="menuitem" disabled={!project} onClick={(event) => {
        onClearProject();
        event.currentTarget.closest("details")?.removeAttribute("open");
      }}><X aria-hidden="true" /><span><strong>{existingChat ? "Start a project-free chat" : "Remove project"}</strong><small>{existingChat ? "Open a new read-only conversation" : "Continue without a linked workspace"}</small></span></button>
    </div>
  </details>;
}

function AppReferencePalette({ open, apps, query, onSelect }: { open: boolean; apps: CodexAppInfo[]; query: string; onSelect: (app: CodexAppInfo) => void }) {
  if (!open) return null;
  const needle = query.toLocaleLowerCase();
  const candidates = apps
    .filter((app) => app.isEnabled !== false && app.isAccessible !== false)
    .filter((app) => !needle || `${app.name} ${app.id} ${app.description ?? ""}`.toLocaleLowerCase().includes(needle))
    .slice(0, 100);
  return <div className="mention-palette app-reference-palette" role="listbox" aria-label="Enabled apps" data-transient-popover data-scroll-chain>
    <section><div className="mention-palette-heading">Apps</div>{candidates.map((app) => <button type="button" role="option" aria-selected="false" key={app.id} onClick={() => onSelect(app)}>
      <Blocks aria-hidden="true" />
      <span><strong>{app.name}</strong><small>{app.description || `$${app.id}`}</small></span>
    </button>)}</section>
    {candidates.length === 0 && <p>{apps.length === 0 ? "No enabled apps are available." : "No matching apps."}</p>}
  </div>;
}

function MentionPalette({ open, candidates, onSelect }: { open: boolean; candidates: MentionCandidate[]; onSelect: (candidate: MentionCandidate) => void }) {
  if (!open) return null;
  const groups = [
    { kind: "file", label: "Files" },
    { kind: "folder", label: "Folders" },
    { kind: "plugin", label: "Plugins" }
  ] as const;
  return <div className="mention-palette" role="listbox" aria-label="Mention files, folders, or plugins" data-transient-popover data-scroll-chain>
    {groups.map((group) => {
      const matches = candidates.filter((candidate) => candidate.kind === group.kind);
      if (matches.length === 0) return null;
      return <section key={group.kind}><div className="mention-palette-heading">{group.label}</div>{matches.map((candidate) => <button type="button" role="option" aria-selected="false" key={`${candidate.kind}:${candidate.resource}`} onClick={() => onSelect(candidate)}>
        {candidate.kind === "file" ? <File aria-hidden="true" /> : candidate.kind === "folder" ? <Folder aria-hidden="true" /> : <Plug aria-hidden="true" />}
        <span><strong>{candidate.label}</strong><small>{candidate.detail ?? candidate.resource}</small></span>
      </button>)}</section>;
    })}
    {candidates.length === 0 && <p>No matching references.</p>}
  </div>;
}

function MentionChips({ mentions }: { mentions: ConversationMention[] }) {
  if (mentions.length === 0) return null;
  return <div className="message-mention-chips" aria-label="Referenced resources">{mentions.map((mention) => <span className={`message-mention-chip mention-${mention.kind}`} key={mention.id} title={mention.label}>
    {mention.kind === "file" ? <File aria-hidden="true" /> : mention.kind === "folder" ? <Folder aria-hidden="true" /> : <Plug aria-hidden="true" />}@{mention.label}
  </span>)}</div>;
}

function SlashCommandPalette({ value, open, onSelect, hasProject, hasConversation }: { value: string; open: boolean; onSelect: (command: SlashCommandName) => void; hasProject: boolean; hasConversation: boolean }) {
  const commands = slashCommandMatches(value);
  if (!open || commands.length === 0) return null;
  return <div className="slash-command-palette" role="listbox" aria-label="Slash commands" data-scroll-chain>
    {commands.map((command) => {
      const contextUnavailable = command.availability === "project" && !hasProject
        ? "Choose a project first"
        : command.availability === "chat" && !hasConversation
          ? "Available after the chat starts"
          : null;
      return <button type="button" role="option" aria-selected="false" key={command.name} onClick={() => onSelect(command.name)}>
        <span className="slash-command-name">/{command.name}</span>
        <span><strong>{command.description}</strong>{contextUnavailable && <small>{contextUnavailable}</small>}</span>
      </button>;
    })}
  </div>;
}

function TechnicalDetails({ title, summary, children }: { title: string; summary: string; children: React.ReactNode }) {
  return <details className="technical-details">
    <summary aria-label={title} role="button" tabIndex={0}><span aria-hidden="true"><strong>{title}</strong><small>{summary}</small></span><span aria-hidden="true">⌄</span></summary>
    <div className="technical-details-content">{children}</div>
  </details>;
}

type ChatRunPanel = "root" | "provider" | "model" | "effort" | "speed" | "team" | "search" | "features" | "config" | "output";

function ChatRunPanelIcon({ panel }: { panel: Exclude<ChatRunPanel, "root"> }) {
  if (panel === "provider") return <Server aria-hidden="true" />;
  if (panel === "model") return <Cpu aria-hidden="true" />;
  if (panel === "effort") return <BrainCircuit aria-hidden="true" />;
  if (panel === "speed") return <Gauge aria-hidden="true" />;
  if (panel === "search") return <Search aria-hidden="true" />;
  if (panel === "features") return <SlidersHorizontal aria-hidden="true" />;
  if (panel === "config") return <Braces aria-hidden="true" />;
  if (panel === "output") return <File aria-hidden="true" />;
  return <Users aria-hidden="true" />;
}

const GOVERNED_CODEX_FEATURES = new Set([
  "shell_tool", "unified_exec", "multi_agent", "enable_fanout", "browser_use",
  "browser_use_external", "browser_use_full_cdp_access", "in_app_browser",
  "computer_use", "apps", "plugins", "image_generation", "workspace_dependencies"
]);

const GOVERNED_CODEX_CONFIG_SEGMENTS = new Set([
  "agent", "agents", "app", "apps", "approval", "approval_policy", "browser",
  "computer_use", "credential", "credentials", "developer_instructions", "dynamic_tools",
  "experimental_user_input", "features", "hooks", "instructions", "mcp_servers",
  "model_provider", "model_providers", "network_access", "oss_provider", "permissions",
  "plugin", "plugins", "process", "remote", "rules", "sandbox", "sandbox_mode",
  "shell_environment_policy", "skills", "show_raw_agent_reasoning", "tools",
  "user_instructions", "workspace_write"
]);

function codexConfigKeyError(key: string): string | null {
  if (!/^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*$/u.test(key)) return "Use a dotted key such as model_verbosity or history.persistence.";
  const segments = key.toLocaleLowerCase().replaceAll("-", "_").split(".");
  const secret = segments.some((segment) => ["api_key", "auth", "bearer", "credential", "password", "secret", "token"].some((needle) => segment.includes(needle)));
  if (secret || segments.some((segment) => GOVERNED_CODEX_CONFIG_SEGMENTS.has(segment))) return "That setting is governed by MetaCog and cannot be overridden per run.";
  return null;
}

const MAX_OUTPUT_SCHEMA_BYTES = 64 * 1024;
const MAX_OUTPUT_SCHEMA_DEPTH = 32;
const MAX_OUTPUT_SCHEMA_NODES = 4096;

function parseOutputSchema(raw: string): { schema?: Record<string, unknown>; error?: string } {
  const source = raw.trim();
  if (!source) return {};
  if (new TextEncoder().encode(source).byteLength > MAX_OUTPUT_SCHEMA_BYTES) {
    return { error: "The output schema must be 64 KiB or smaller." };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(source);
  } catch (error) {
    return { error: `Invalid JSON: ${error instanceof Error ? error.message : "unable to parse schema"}` };
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    return { error: "The output schema must be a JSON object." };
  }
  let nodes = 0;
  const inspect = (value: unknown, depth: number): string | null => {
    nodes += 1;
    if (nodes > MAX_OUTPUT_SCHEMA_NODES) return "The output schema contains more than 4,096 values.";
    if (depth > MAX_OUTPUT_SCHEMA_DEPTH) return "The output schema is nested more than 32 levels deep.";
    if (Array.isArray(value)) {
      for (const child of value) {
        const error = inspect(child, depth + 1);
        if (error) return error;
      }
    } else if (value && typeof value === "object") {
      for (const child of Object.values(value)) {
        const error = inspect(child, depth + 1);
        if (error) return error;
      }
    }
    return null;
  };
  const error = inspect(parsed, 0);
  return error ? { error } : { schema: parsed as Record<string, unknown> };
}

function ChatRunChoice({
  selected,
  label,
  description,
  onSelect
}: {
  selected: boolean;
  label: string;
  description?: string;
  onSelect: (target: HTMLButtonElement) => void;
}) {
  return <button type="button" role="menuitemradio" aria-checked={selected} className="chat-run-choice" onClick={(event) => onSelect(event.currentTarget)}>
    <span className="chat-run-choice-copy"><strong>{label}</strong>{description && <small>{description}</small>}</span>
    <span className="chat-run-choice-check" aria-hidden="true">{selected && <Check />}</span>
  </button>;
}

function ChatRunSelector({
  providers,
  codexModels = [],
  codexConfigurationStatus,
  value,
  onChange,
  discovering,
  discoveryError,
  onRefresh,
  label = "Run settings",
  includeTeam = false,
  hasProject = false,
  advanced = false,
  codexFeatures = []
}: {
  providers: ProviderListItem[];
  codexModels?: CodexModel[];
  codexConfigurationStatus?: CodexConfigurationStatus | null;
  value: ChatRunPreferences;
  onChange: (preferences: ChatRunPreferences) => void;
  discovering: boolean;
  discoveryError: string | null;
  onRefresh: () => void;
  label?: string;
  includeTeam?: boolean;
  hasProject?: boolean;
  advanced?: boolean;
  codexFeatures?: CodexFeature[];
}) {
  const [panel, setPanel] = useState<ChatRunPanel>("root");
  const [featureSearch, setFeatureSearch] = useState("");
  const [configKey, setConfigKey] = useState("");
  const [configValue, setConfigValue] = useState("");
  const [configError, setConfigError] = useState<string | null>(null);
  const [outputSchemaText, setOutputSchemaText] = useState(() => value.output_schema ? JSON.stringify(value.output_schema, null, 2) : "");
  const [outputSchemaError, setOutputSchemaError] = useState<string | null>(null);
  const serializedOutputSchema = value.output_schema ? JSON.stringify(value.output_schema) : "";
  useEffect(() => {
    if (panel !== "output") {
      setOutputSchemaText(value.output_schema ? JSON.stringify(value.output_schema, null, 2) : "");
      setOutputSchemaError(null);
    }
  }, [panel, serializedOutputSchema]);
  const providerModels = providers.flatMap((provider) => (provider.catalog?.models ?? []).map((model) => ({
    providerId: provider.connection.id,
    providerName: provider.connection.display_name,
    model
  })));
  const localProviders = codexConfigurationStatus?.local_providers?.providers ?? [];
  const selectedLocalProvider = value.model_provider
    ? localProviders.find((provider) => provider.id === value.model_provider)
    : undefined;
  const localModelIds = selectedLocalProvider
    ? Array.from(new Set([selectedLocalProvider.default_model, ...selectedLocalProvider.models]))
    : [];
  const useCodexCatalog = !selectedLocalProvider && codexModels.length > 0;
  const models = selectedLocalProvider
    ? localModelIds.map((id) => ({
        providerId: selectedLocalProvider.id,
        providerName: selectedLocalProvider.display_name,
        model: { id, display_name: id } as ProviderModel
      }))
    : useCodexCatalog
      ? codexModels.filter((model) => !model.hidden).map((model) => ({ providerId: "codex", providerName: "OpenAI Subscription", model: { id: model.id, display_name: model.displayName } as ProviderModel }))
      : providerModels;
  const modelKey = value.model_id ? `${value.model_provider ?? value.provider_id ?? "codex"}\u0000${value.model_id}` : "";
  const summaryModel = value.model_label ?? value.model_id ?? "Automatic";
  const providerSummary = selectedLocalProvider?.display_name ?? "OpenAI Subscription";
  const updateModel = (key: string) => {
    const selected = models.find((entry) => `${entry.providerId}\u0000${entry.model.id}` === key);
    const selectedLocal = selected ? localProviders.find((provider) => provider.id === selected.providerId) : undefined;
    onChange(selected ? {
      ...value,
      provider_id: selected.providerId,
      model_provider: selectedLocal?.id as "ollama" | "lmstudio" | undefined,
      model_id: selected.model.id,
      model_label: selected.model.display_name,
      reasoning_effort: selectedLocal
        ? (["low", "medium", "high"].includes(value.reasoning_effort) ? value.reasoning_effort : "medium")
        : useCodexCatalog
        ? codexModels.find((model) => model.id === selected.model.id)?.defaultReasoningEffort ?? value.reasoning_effort
        : value.reasoning_effort,
      service_tier: selectedLocal
        ? undefined
        : useCodexCatalog
        ? codexModels.find((model) => model.id === selected.model.id)?.defaultServiceTier
        : value.service_tier,
      speed: selectedLocal ? "standard" : value.speed
    } : {
      model_provider: undefined,
      reasoning_effort: value.reasoning_effort,
      speed: value.speed,
      service_tier: value.service_tier,
      approval_mode: value.approval_mode,
      approval_policy: value.approval_policy,
      sandbox_mode: value.sandbox_mode,
      permission_profile: value.permission_profile,
      additional_writable_roots: value.additional_writable_roots,
      codex_feature_overrides: value.codex_feature_overrides,
      codex_config_overrides: value.codex_config_overrides,
      codex_profile: value.codex_profile,
      strict_config: value.strict_config,
      output_schema: value.output_schema,
      team_preset: value.team_preset,
      personality: value.personality
    });
  };
  const selectedCodexModel = !selectedLocalProvider && value.model_id
    ? codexModels.find((model) => model.id === value.model_id)
    : !selectedLocalProvider ? codexModels.find((model) => model.isDefault) : undefined;
  const effortChoices = selectedLocalProvider
    ? ["low", "medium", "high"]
    : selectedCodexModel?.supportedReasoningEfforts.map((item) => item.reasoningEffort) ?? ["none", "low", "medium", "high", "xhigh"];
  const tierChoices = selectedLocalProvider ? [] : selectedCodexModel?.serviceTiers ?? [];
  const effortLabel = (effort: string) => effort === "xhigh" ? "Extra high" : effort.charAt(0).toUpperCase() + effort.slice(1);
  const effortDescription = (effort: string) => ({
    none: "No additional reasoning",
    low: "Quick, lightweight reasoning",
    medium: "Balanced reasoning for most work",
    high: "More deliberate problem solving",
    xhigh: "Deep reasoning, higher usage",
    max: "Maximum supported reasoning",
    ultra: "Highest supported reasoning and usage"
  } as Record<string, string>)[effort] ?? "Model-supported reasoning level";
  const speedLabel = /fast|priority/i.test(value.service_tier ?? "") || value.speed === "fast" ? "Fast" : "Standard";
  const teamValue = TEAM_PRESET_DETAILS[value.team_preset ?? "automatic"] ? value.team_preset ?? "automatic" : "automatic";
  const writableRoots = value.additional_writable_roots ?? [];
  const featureOverrides = value.codex_feature_overrides ?? {};
  const configOverrides = value.codex_config_overrides ?? [];
  const codexProfile = value.codex_profile ?? "";
  const configSummary = [
    codexProfile.trim() ? `Profile ${codexProfile.trim()}` : null,
    value.strict_config ? "Strict" : null,
    configOverrides.length > 0 ? `${configOverrides.length} override${configOverrides.length === 1 ? "" : "s"}` : null
  ].filter(Boolean).join(" · ") || "Use Codex defaults";
  const outputSchemaSummary = value.output_schema ? "Structured JSON" : "Text";
  const webSearchOverride = [...configOverrides].reverse().find((config) => config.key.trim().toLocaleLowerCase().replaceAll("-", "_") === "web_search");
  const webSearchMode = typeof webSearchOverride?.value === "string" && ["cached", "live", "disabled"].includes(webSearchOverride.value)
    ? webSearchOverride.value as "cached" | "live" | "disabled"
    : "cached";
  const configurableFeatures = codexFeatures.filter((feature) => !GOVERNED_CODEX_FEATURES.has(feature.name) && feature.stage !== "removed");
  const visibleFeatures = configurableFeatures.filter((feature) => {
    const query = featureSearch.trim().toLocaleLowerCase();
    return !query || `${feature.displayName ?? ""} ${feature.name} ${feature.description ?? ""}`.toLocaleLowerCase().includes(query);
  });
  const setFeatureOverride = (feature: string, state: "default" | "enabled" | "disabled") => {
    const next = { ...featureOverrides };
    if (state === "default") delete next[feature];
    else next[feature] = state === "enabled";
    onChange({ ...value, codex_feature_overrides: next });
  };
  const addConfigOverride = () => {
    const key = configKey.trim();
    const keyError = codexConfigKeyError(key);
    if (keyError) {
      setConfigError(keyError);
      return;
    }
    const raw = configValue.trim();
    if (!raw) {
      setConfigError("Enter a value. JSON values are typed; plain text is stored as a string.");
      return;
    }
    let parsed: unknown = raw;
    try { parsed = JSON.parse(raw); } catch { /* Codex-compatible literal string fallback. */ }
    if (key.toLocaleLowerCase().replaceAll("-", "_") === "web_search" && (typeof parsed !== "string" || !["cached", "live", "disabled"].includes(parsed))) {
      setConfigError("Web search must be cached, live, or disabled.");
      return;
    }
    onChange({ ...value, codex_config_overrides: [...configOverrides, { key, value: parsed }] });
    setConfigKey("");
    setConfigValue("");
    setConfigError(null);
  };
  const setWebSearchMode = (mode: "cached" | "live" | "disabled") => {
    const withoutWebSearch = configOverrides.filter((config) => config.key.trim().toLocaleLowerCase().replaceAll("-", "_") !== "web_search");
    onChange({ ...value, codex_config_overrides: mode === "cached" ? withoutWebSearch : [...withoutWebSearch, { key: "web_search", value: mode }] });
  };
  const addWritableRoot = async () => {
    const path = await pickWritableFolder();
    if (!path || writableRoots.some((root) => root.toLocaleLowerCase() === path.toLocaleLowerCase())) return;
    onChange({ ...value, additional_writable_roots: [...writableRoots, path] });
  };
  const selectModelProvider = (providerId?: string) => {
    const provider = localProviders.find((candidate) => candidate.id === providerId);
    if (!provider) {
      onChange({
        ...value,
        provider_id: "codex",
        model_provider: undefined,
        model_id: undefined,
        model_label: undefined,
        service_tier: undefined,
        speed: "standard"
      });
      return;
    }
    onChange({
      ...value,
      provider_id: provider.id,
      model_provider: provider.id as "ollama" | "lmstudio",
      model_id: provider.default_model,
      model_label: provider.default_model,
      reasoning_effort: ["low", "medium", "high"].includes(value.reasoning_effort) ? value.reasoning_effort : "medium",
      service_tier: undefined,
      speed: "standard"
    });
  };
  const applyOutputSchema = () => {
    const result = parseOutputSchema(outputSchemaText);
    if (result.error) {
      setOutputSchemaError(result.error);
      return;
    }
    onChange({ ...value, output_schema: result.schema });
    setOutputSchemaText(result.schema ? JSON.stringify(result.schema, null, 2) : "");
    setOutputSchemaError(null);
  };
  const closeSelector = (target: HTMLElement) => target.closest("details")?.removeAttribute("open");
  return <details className="chat-run-selector" data-transient-popover onToggle={(event) => {
    if (!(event.currentTarget as HTMLDetailsElement).open) setPanel("root");
  }}>
    <summary aria-label={label} title={label}>
      <span>{summaryModel}</span>
      <small>{effortLabel(value.reasoning_effort)}</small>
      <ChevronDown aria-hidden="true" />
    </summary>
    <div className="chat-run-popover" data-panel={panel} role="dialog" aria-label={label}>
      {panel === "root" ? <>
        {advanced && <button className="chat-run-menu-row" type="button" onClick={() => setPanel("provider")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="provider" /></span><span className="chat-run-menu-copy"><strong>Provider</strong><small>{providerSummary}</small></span><ChevronRight aria-hidden="true" /></button>}
        <button className="chat-run-menu-row" type="button" onClick={() => setPanel("model")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="model" /></span><span className="chat-run-menu-copy"><strong>Model</strong><small>{summaryModel}</small></span><ChevronRight aria-hidden="true" /></button>
        <button className="chat-run-menu-row" type="button" onClick={() => setPanel("effort")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="effort" /></span><span className="chat-run-menu-copy"><strong>Effort</strong><small>{effortLabel(value.reasoning_effort)}</small></span><ChevronRight aria-hidden="true" /></button>
        <button className="chat-run-menu-row" type="button" onClick={() => setPanel("speed")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="speed" /></span><span className="chat-run-menu-copy"><strong>Speed</strong><small>{speedLabel}</small></span><ChevronRight aria-hidden="true" /></button>
        {includeTeam && <button className="chat-run-menu-row" type="button" onClick={() => setPanel("team")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="team" /></span><span className="chat-run-menu-copy"><strong>Team</strong><small>{TEAM_PRESET_DETAILS[teamValue].label}</small></span><ChevronRight aria-hidden="true" /></button>}
        {advanced && <button className="chat-run-menu-row" type="button" onClick={() => setPanel("search")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="search" /></span><span className="chat-run-menu-copy"><strong>Web search</strong><small>{webSearchMode === "live" ? "Live" : webSearchMode === "disabled" ? "Off" : "Default (cached)"}</small></span><ChevronRight aria-hidden="true" /></button>}
        {advanced && <button className="chat-run-menu-row" type="button" onClick={() => setPanel("features")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="features" /></span><span className="chat-run-menu-copy"><strong>Session features</strong><small>{Object.keys(featureOverrides).length > 0 ? `${Object.keys(featureOverrides).length} override${Object.keys(featureOverrides).length === 1 ? "" : "s"}` : "Use Codex defaults"}</small></span><ChevronRight aria-hidden="true" /></button>}
        {advanced && <button className="chat-run-menu-row" type="button" onClick={() => setPanel("config")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="config" /></span><span className="chat-run-menu-copy"><strong>Codex config</strong><small>{configSummary}</small></span><ChevronRight aria-hidden="true" /></button>}
        {advanced && <button className="chat-run-menu-row" type="button" onClick={() => setPanel("output")}><span className="chat-run-menu-icon"><ChatRunPanelIcon panel="output" /></span><span className="chat-run-menu-copy"><strong>Output format</strong><small>{outputSchemaSummary}</small></span><ChevronRight aria-hidden="true" /></button>}
        <details className="chat-run-advanced">
          <summary>Advanced <ChevronDown aria-hidden="true" /></summary>
          <div className="chat-run-model-status">
            <small role={discoveryError ? "alert" : "status"}>{discovering
              ? "Loading available models…"
              : discoveryError
                ? discoveryError
                : models.length > 0
                  ? `${models.length} available model${models.length === 1 ? "" : "s"}`
                  : providers.length > 0 || useCodexCatalog
                    ? "No models have been reported yet."
                    : "Connect a model provider to choose a specific model."}</small>
            <button className="secondary chat-run-refresh" type="button" disabled={discovering || (!useCodexCatalog && providers.length === 0)} onClick={onRefresh}>{discovering ? "Loading…" : "Refresh"}</button>
          </div>
          <div className="chat-run-writable-roots">
            <div className="chat-run-writable-heading">
              <span><FolderOpen aria-hidden="true" /> Additional writable folders</span>
              <button className="secondary" type="button" disabled={!hasProject || value.approval_mode === "read_only" || value.sandbox_mode === "read-only"} title={!hasProject ? "Choose a project first" : value.approval_mode === "read_only" || value.sandbox_mode === "read-only" ? "Choose a writable permission mode first" : undefined} onClick={() => void addWritableRoot()}>Add folder</button>
            </div>
            <small>Explicitly allows workspace-write turns to modify these folders. Normal approvals and governance still apply.</small>
            {!hasProject && <small>Choose a project before adding writable folders.</small>}
            {(value.approval_mode === "read_only" || value.sandbox_mode === "read-only") && writableRoots.length > 0 && <small role="alert">Read only cannot be combined with additional writable folders.</small>}
            {writableRoots.length > 0 && <ul aria-label="Additional writable folders">
              {writableRoots.map((root) => <li key={root}><code title={root}>{root}</code><button type="button" aria-label={`Remove writable folder ${root}`} onClick={() => onChange({ ...value, additional_writable_roots: writableRoots.filter((candidate) => candidate !== root) })}><X aria-hidden="true" /></button></li>)}
            </ul>}
          </div>
        </details>
      </> : <>
        <button className="chat-run-menu-back" type="button" onClick={() => setPanel("root")}><ChevronLeft aria-hidden="true" /><span>{panel === "provider" ? "Provider" : panel === "model" ? "Model" : panel === "effort" ? "Effort" : panel === "speed" ? "Speed" : panel === "search" ? "Web search" : panel === "features" ? "Session features" : panel === "config" ? "Codex config" : panel === "output" ? "Output format" : "Team"}</span></button>
        {panel === "provider" ? <div className="chat-run-choice-list" role="menu" aria-label="Model provider">
          <ChatRunChoice selected={!selectedLocalProvider} label="OpenAI Subscription" description="Use your shared Codex account" onSelect={(target) => { selectModelProvider(); closeSelector(target); setPanel("root"); }} />
          {localProviders.map((provider) => <ChatRunChoice
            selected={selectedLocalProvider?.id === provider.id}
            label={provider.display_name}
            description={`${provider.running ? `Running${provider.version ? ` · ${provider.version}` : ""}` : "Not running"} · ${provider.endpoint}`}
            key={provider.id}
            onSelect={(target) => { selectModelProvider(provider.id); closeSelector(target); setPanel("root"); }}
          />)}
          {localProviders.length === 0 && <small className="empty-state">Local-provider discovery is unavailable from the connected MetaCog service.</small>}
        </div> : panel === "search" ? <div className="chat-run-choice-list" role="menu" aria-label="Web search">
          <ChatRunChoice selected={webSearchMode === "cached"} label="Default" description="Use cached search when available" onSelect={(target) => { setWebSearchMode("cached"); closeSelector(target); setPanel("root"); }} />
          <ChatRunChoice selected={webSearchMode === "live"} label="Live" description="Use current web results for this run" onSelect={(target) => { setWebSearchMode("live"); closeSelector(target); setPanel("root"); }} />
          <ChatRunChoice selected={webSearchMode === "disabled"} label="Off" description="Do not use web search for this run" onSelect={(target) => { setWebSearchMode("disabled"); closeSelector(target); setPanel("root"); }} />
        </div> : panel === "features" ? <div className="chat-run-feature-panel">
          <p>Override supported Codex flags for this run only. MetaCog-governed authority features are managed elsewhere.</p>
          <input type="search" aria-label="Search session features" placeholder="Search features" value={featureSearch} onChange={(event) => setFeatureSearch(event.target.value)} />
          <div className="chat-run-feature-list" data-scroll-chain>
            {visibleFeatures.map((feature) => {
              const state = feature.name in featureOverrides ? featureOverrides[feature.name] ? "enabled" : "disabled" : "default";
              return <div className="chat-run-feature-row" key={feature.name}>
                <span><strong>{feature.displayName || feature.name}</strong><code>{feature.name}</code><small>{feature.description || `${feature.stage} Codex feature`}</small></span>
                <select aria-label={`${feature.displayName || feature.name} session override`} value={state} onChange={(event) => setFeatureOverride(feature.name, event.target.value as "default" | "enabled" | "disabled")}>
                  <option value="default">Default ({feature.enabled ? "on" : "off"})</option>
                  <option value="enabled">On</option>
                  <option value="disabled">Off</option>
                </select>
              </div>;
            })}
            {visibleFeatures.length === 0 && <small className="empty-state">{codexFeatures.length === 0 ? "Feature catalog is still loading." : "No configurable features match."}</small>}
          </div>
        </div> : panel === "config" ? <div className="chat-run-config-panel">
          <p>Set typed Codex configuration for this run only. Later rows with the same key win. MetaCog-governed authority and secret-bearing settings are blocked.</p>
          <label className="chat-run-profile-field">
            <span><strong>Codex profile</strong><small>Layer <code>$CODEX_HOME/&lt;name&gt;.config.toml</code>. MetaCog ignores authority- and secret-bearing profile settings.</small></span>
            <input aria-label="Codex profile" placeholder="team" value={codexProfile} onChange={(event) => onChange({ ...value, codex_profile: event.target.value || undefined })} />
          </label>
          <label className="chat-run-strict-config">
            <span><strong>Strict config</strong><small>Fail before dispatch when Codex finds an unknown field in the effective user or project configuration.</small></span>
            <span className="settings-switch"><input aria-label="Require strict Codex configuration" type="checkbox" checked={value.strict_config ?? false} onChange={(event) => onChange({ ...value, strict_config: event.target.checked })} /><i aria-hidden="true" /></span>
          </label>
          <div className="chat-run-config-form">
            <label><span>Key</span><input aria-label="Codex configuration key" placeholder="model_verbosity" value={configKey} onChange={(event) => { setConfigKey(event.target.value); setConfigError(null); }} /></label>
            <label><span>Value</span><input aria-label="Codex configuration value" placeholder='"low", true, 42, or plain text' value={configValue} onChange={(event) => { setConfigValue(event.target.value); setConfigError(null); }} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); addConfigOverride(); } }} /></label>
            <button className="secondary" type="button" onClick={addConfigOverride}>Add override</button>
          </div>
          {configError && <small className="chat-run-config-error" role="alert">{configError}</small>}
          <div className="chat-run-config-list" data-scroll-chain aria-label="Codex configuration overrides">
            {configOverrides.map((config, index) => <div className="chat-run-config-row" key={`${config.key}-${index}`}>
              <span><code>{config.key}</code><small title={JSON.stringify(config.value)}>{JSON.stringify(config.value)}</small></span>
              <button type="button" aria-label={`Remove Codex configuration ${config.key}`} onClick={() => onChange({ ...value, codex_config_overrides: configOverrides.filter((_, candidate) => candidate !== index) })}><X aria-hidden="true" /></button>
            </div>)}
            {configOverrides.length === 0 && <small className="empty-state">No per-run configuration overrides.</small>}
          </div>
        </div> : panel === "output" ? <div className="chat-run-output-panel">
          <p>Require the final owner-visible response to match a JSON Schema. Routing and agent work remain unconstrained; Plan and active Goal runs cannot use structured output.</p>
          <label>
            <span><strong>Output JSON Schema</strong><small>JSON object · 64 KiB maximum · 32 nesting levels</small></span>
            <textarea aria-label="Output JSON Schema" spellCheck={false} placeholder={'{\n  "type": "object",\n  "properties": {\n    "summary": { "type": "string" }\n  },\n  "required": ["summary"],\n  "additionalProperties": false\n}'} value={outputSchemaText} onChange={(event) => { setOutputSchemaText(event.target.value); setOutputSchemaError(null); }} />
          </label>
          {outputSchemaError && <small className="chat-run-config-error" role="alert">{outputSchemaError}</small>}
          <div className="chat-run-output-actions">
            <button className="secondary" type="button" disabled={!value.output_schema && !outputSchemaText.trim()} onClick={() => { onChange({ ...value, output_schema: undefined }); setOutputSchemaText(""); setOutputSchemaError(null); }}>Use text</button>
            <button className="primary" type="button" onClick={applyOutputSchema}>Apply schema</button>
          </div>
        </div> : <div className="chat-run-choice-list" role="menu" aria-label={panel === "model" ? "Model" : panel === "effort" ? "Effort" : panel === "speed" ? "Speed" : "Team preset"}>
          {panel === "model" && <>
            {!selectedLocalProvider && <ChatRunChoice selected={!modelKey} label="Automatic" description="Use the account default" onSelect={(target) => { updateModel(""); closeSelector(target); setPanel("root"); }} />}
            {models.map(({ providerId, providerName, model }) => {
              const key = `${providerId}\u0000${model.id}`;
              return <ChatRunChoice selected={modelKey === key} label={model.display_name} description={providerName} key={`${providerId}-${model.id}`} onSelect={(target) => { updateModel(key); closeSelector(target); setPanel("root"); }} />;
            })}
          </>}
          {panel === "effort" && effortChoices.map((effort) => <ChatRunChoice selected={value.reasoning_effort === effort} label={effortLabel(effort)} description={effortDescription(effort)} key={effort} onSelect={(target) => { onChange({ ...value, reasoning_effort: effort as ReasoningEffort }); closeSelector(target); setPanel("root"); }} />)}
          {panel === "speed" && [{ id: "", name: "Standard" }, ...(tierChoices.length > 0 ? tierChoices : selectedLocalProvider ? [] : [{ id: "fast", name: "Fast" }])].map((tier) => {
            const selectedTier = tier.id ? value.service_tier === tier.id : !value.service_tier;
            return <ChatRunChoice selected={selectedTier} label={tier.name} description={tier.id ? "Faster responses, more usage" : "Default speed"} key={tier.id || "standard"} onSelect={(target) => { onChange({ ...value, service_tier: tier.id || undefined, speed: /fast|priority/i.test(tier.id) ? "fast" : "standard" as ExecutionSpeed }); closeSelector(target); setPanel("root"); }} />;
          })}
          {panel === "team" && (Object.entries(TEAM_PRESET_DETAILS) as [TeamPreset, (typeof TEAM_PRESET_DETAILS)[TeamPreset]][]).map(([preset, detail]) => <ChatRunChoice selected={teamValue === preset} label={detail.label} description={detail.description} key={preset} onSelect={(target) => { onChange({ ...value, team_preset: preset }); closeSelector(target); setPanel("root"); }} />)}
        </div>}
      </>}
    </div>
  </details>;
}

function ConversationTurnStatus({ turn, hasResponseText }: { turn: ConversationTurn; hasResponseText: boolean }) {
  const [elapsedSeconds, setElapsedSeconds] = useState(() => Math.max(0, Math.floor((Date.now() - Date.parse(turn.created_at)) / 1000)));

  useEffect(() => {
    const update = () => setElapsedSeconds(Math.max(0, Math.floor((Date.now() - Date.parse(turn.created_at)) / 1000)));
    update();
    const timer = window.setInterval(update, 1000);
    return () => window.clearInterval(timer);
  }, [turn.created_at]);

  const label = turn.state === "pending"
    ? "Thinking"
    : turn.state === "waiting_for_approval"
      ? "Waiting for your approval"
    : turn.state === "recovering"
      ? "Connection interrupted — checking turn status"
      : hasResponseText
        ? "Responding"
        : "Thinking";
  return <div className={`chat-turn-status state-${turn.state}`} role="status" aria-live="polite">
    <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>
    <span>{label}{["recovering", "waiting_for_approval"].includes(turn.state) ? "" : ` · ${elapsedSeconds}s`}</span>
  </div>;
}

function activityDuration(durationMs?: number): string | null {
  if (durationMs === undefined) return null;
  return durationMs < 1000 ? `${durationMs}ms` : `${(durationMs / 1000).toFixed(durationMs < 10_000 ? 1 : 0)}s`;
}

function conversationActivityPresentation(activity: ConversationActivity): [string, string] {
  const presentation: Record<ConversationActivity["kind"], [string, string]> = {
    working: ["…", "Working"], waiting: ["◷", "Waiting"], reasoning_summary: ["✦", "Reasoning"],
    plan: ["☷", "Updated plan"], command: ["›_", "Ran commands"], file_change: ["✎", "Updated files"],
    mcp_call: ["⌘", "Used a tool"], collaboration_call: ["⌘", "Delegated work"], web_search: ["⌕", "Searched the web"],
    image_view: ["▧", "Viewed an image"], approval: ["!", "Approval"], elicitation: ["?", "Input requested"],
    context_compaction: ["≡", activity.state === "running" ? "Context automatically compacting" : "Context automatically compacted"],
    hook: ["⌁", activity.state === "running" ? "Running lifecycle hook" : "Lifecycle hook"],
    usage: ["◔", "Usage updated"], recovery: ["↻", "Recovering"], interruption: ["■", "Interrupted"],
    failure: ["!", "Failed"], completion: ["✓", "Completed"]
  };
  return presentation[activity.kind];
}

function AgentLiveActivity({ activities }: { activities: ConversationActivity[] }) {
  const visible = activities
    .filter((activity) => activity.kind !== "usage")
    .slice()
    .sort((left, right) => Date.parse(left.started_at) - Date.parse(right.started_at) || left.codex_item_id.localeCompare(right.codex_item_id));
  if (visible.length === 0) return <div className="agent-live-activity-empty">No reasoning or tool activity recorded yet.</div>;
  const latest = visible.at(-1)!;
  const [latestIcon, latestFallback] = conversationActivityPresentation(latest);
  const latestLabel = latest.command || latestFallback;
  return <details className={`agent-live-activity state-${latest.state}`}>
    <summary>
      <span className="chat-activity-icon" aria-hidden="true">{latestIcon}</span>
      <span className="agent-live-activity-label">{latestLabel}</span>
      <small>{visible.length} event{visible.length === 1 ? "" : "s"}</small>
      {latest.state === "running" && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}
      <span className="chat-activity-chevron" aria-hidden="true">⌄</span>
    </summary>
    <div className="agent-live-activity-scroll" data-scroll-chain aria-label="Agent activity history">
      {visible.map((activity) => {
        const [icon, fallback] = conversationActivityPresentation(activity);
        const label = activity.command || fallback;
        const duration = activityDuration(activity.duration_ms);
        const hasDetails = Boolean(activity.cwd || activity.output || activity.exit_code !== undefined || duration);
        const row = <>
          <span className="chat-activity-icon" aria-hidden="true">{icon}</span>
          <span>{label}</span>
          {activity.state === "running" && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}
        </>;
        return hasDetails
          ? <details className={`agent-live-activity-event state-${activity.state}`} key={activity.codex_item_id}>
              <summary>{row}</summary>
              <div className="chat-command-details">
                {activity.cwd && <div><span>Directory</span><code>{activity.cwd}</code></div>}
                {(activity.exit_code !== undefined || duration) && <div className="chat-command-meta">
                  {activity.exit_code !== undefined && <span>Exit code {activity.exit_code}</span>}
                  {duration && <span>{duration}</span>}
                </div>}
                {activity.output && <pre data-scroll-chain>{activity.output}</pre>}
              </div>
            </details>
          : <div className={`agent-live-activity-event state-${activity.state}`} key={activity.codex_item_id}>{row}</div>;
      })}
    </div>
  </details>;
}

function AgentChatRoom({ session, sessions, messages, activities, children }: {
  session?: ConversationAgentSession;
  sessions: ConversationAgentSession[];
  messages: AgentMessage[];
  activities: ConversationActivity[];
  children?: ReactNode;
}) {
  const sessionName = (id: string) => sessions.find((candidate) => candidate.id === id)?.display_name || "Agent";
  const orderedMessages = messages.slice().sort((left, right) => left.created_at.localeCompare(right.created_at));
  const chatMessages = orderedMessages.filter((message) => !["tool_call", "tool_result"].includes(message.kind));
  const toolMessages = orderedMessages.filter((message) => ["tool_call", "tool_result"].includes(message.kind));
  const displayContent = (message: AgentMessage): string => {
    if (!message.content.trim().startsWith("{")) return message.content;
    try {
      const value = JSON.parse(message.content) as Record<string, unknown>;
      if (message.kind === "delegation" && typeof value.objective === "string") {
        const sections: Array<[string, unknown]> = [
          ["Scope", value.scope], ["Constraints", value.constraints],
          ["Success conditions", value.success_conditions], ["Context", value.context_references]
        ];
        return [`**${value.objective}**`, ...sections.flatMap(([label, items]) =>
          Array.isArray(items) && items.length > 0 ? [`\n${label}`, ...items.map((item) => `- ${String(item)}`)] : []
        )].join("\n");
      }
      if (message.kind === "report" && typeof value.summary === "string") {
        const lines = [`**Result**\n\n${value.summary}`];
        if (Array.isArray(value.checks) && value.checks.length > 0) lines.push(`\n**Checks**\n${value.checks.map((item) => `- ${String(item)}`).join("\n")}`);
        if (Array.isArray(value.evidence) && value.evidence.length > 0) lines.push(`\n**Evidence**\n${value.evidence.map((item) => `- ${String(item)}`).join("\n")}`);
        if (Array.isArray(value.unresolved_issues) && value.unresolved_issues.length > 0) lines.push(`\n**Unresolved**\n${value.unresolved_issues.map((item) => `- ${String(item)}`).join("\n")}`);
        return lines.join("\n");
      }
    } catch {
      // Preserve non-contract text that merely begins with a brace.
    }
    return message.content;
  };
  return <section className="agent-chat-room" data-scroll-chain aria-label={`${session?.display_name || "Agent"} chat room`} tabIndex={0}>
    <div className="agent-chat-transcript chat-message-list">
      {chatMessages.length === 0
        ? <p className="agent-chat-empty">No agent messages recorded yet.</p>
        : chatMessages.map((message) => {
            const own = message.from_session_id === session?.id;
            return <article className={`chat-message agent-chat-message ${own ? "assistant-message from-agent" : "user-message to-agent"}`} key={message.id}>
              <span>{sessionName(message.from_session_id)}</span>
              <MarkdownContent content={displayContent(message)} onOpenLink={openSiteExternalUrl} />
            </article>;
          })}
    </div>
    {toolMessages.length > 0 && <details className="agent-tool-trace">
      <summary><span className="chat-activity-icon" aria-hidden="true">⌘</span><span>{toolMessages.at(-1)?.tool_name || "Used an internal tool"}</span><small>{toolMessages.length} event{toolMessages.length === 1 ? "" : "s"}</small><span className="chat-activity-chevron" aria-hidden="true">⌄</span></summary>
      <div className="agent-live-activity-scroll" data-scroll-chain>{toolMessages.map((message) => <div className="agent-live-activity-event" key={message.id}><span className="chat-activity-icon" aria-hidden="true">{message.kind === "tool_call" ? "›" : "✓"}</span><span>{message.tool_name || message.kind.replaceAll("_", " ")}</span><small>{message.content}</small></div>)}</div>
    </details>}
    <AgentLiveActivity activities={activities} />
    {children && <div className="agent-chat-children" aria-label="Sub-agent widgets">{children}</div>}
  </section>;
}

function ConversationCommandGroup({ commands }: { commands: ConversationActivity[] }) {
  const commandsRunning = commands.some((activity) => activity.state === "running");
  const commandsFailed = commands.some((activity) => activity.state === "failed" || activity.state === "declined");
  const commandGroupLabel = commandsRunning ? "Running commands" : commandsFailed ? "Commands finished with errors" : "Ran commands";
  const latestCommand = commands.at(-1)?.command;

  return <details className={`chat-command-group ${commandsRunning ? "is-running" : ""}`}>
      <summary>
        <span className="chat-activity-icon" aria-hidden="true">›_</span>
        <span>{commandGroupLabel}</span>
        <small>{commands.length} {commands.length === 1 ? "command" : "commands"}</small>
        {latestCommand && <code className="chat-command-latest" title={latestCommand}>{latestCommand}</code>}
        <span className="chat-activity-chevron" aria-hidden="true">⌄</span>
      </summary>
      <div className="chat-command-list">
        {commands.map((activity) => {
          const duration = activityDuration(activity.duration_ms);
          const verb = activity.state === "running" ? "Running" : activity.state === "declined" ? "Declined" : activity.state === "failed" ? "Failed" : "Ran";
          const hasDetails = Boolean(activity.cwd || activity.output || activity.exit_code !== undefined || duration);
          const summary = <>
            <span className="chat-activity-icon" aria-hidden="true">›_</span>
            <code>{verb} {activity.command || "command"}</code>
            {activity.state === "running" && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}
          </>;
          return hasDetails
            ? <details className={`chat-command state-${activity.state}`} key={activity.codex_item_id}>
                <summary>{summary}</summary>
                <div className="chat-command-details">
                  {activity.cwd && <div><span>Directory</span><code>{activity.cwd}</code></div>}
                  {(activity.exit_code !== undefined || duration) && <div className="chat-command-meta">
                    {activity.exit_code !== undefined && <span>Exit code {activity.exit_code}</span>}
                    {duration && <span>{duration}</span>}
                  </div>}
                  {activity.output && <pre data-scroll-chain>{activity.output}</pre>}
                </div>
              </details>
            : <div className={`chat-command state-${activity.state}`} key={activity.codex_item_id}>{summary}</div>;
        })}
      </div>
    </details>;
}

type ConversationTimelineEntry =
  | { type: "message"; timestamp: number; stableOrder: number; message: ConversationMessage }
  | { type: "activity"; timestamp: number; stableOrder: number; activity: ConversationActivity }
  | { type: "batch"; timestamp: number; stableOrder: number; batch: CoordinationBatch };

function activityBelongsToBatch(activity: ConversationActivity, batch: CoordinationBatch, team?: ConversationTeamState): boolean {
  if (activity.delegation_id && batch.delegation_ids.includes(activity.delegation_id)) return true;
  if (activity.agent_session_id === batch.coordinator_session_id) return true;
  if (activity.agent_session_id && team?.delegations.some((delegation) =>
    batch.delegation_ids.includes(delegation.id) && delegation.coder_session_id === activity.agent_session_id
  )) return true;
  // Schema-44 and earlier activities predate durable agent attribution. A single-Coder
  // batch can safely adopt its origin-turn trace because Chat and Coordinator cannot run
  // project commands. New multi-agent activity is always attributed explicitly.
  return !activity.agent_session_id
    && !activity.delegation_id
    && batch.delegation_ids.length === 1
    && activity.turn_id === batch.origin_turn_id;
}

function ConversationActivityTimeline({ messages, activities, batches, team, conversationId, onView, onOpenPreview }: {
  messages: ConversationMessage[];
  activities: ConversationActivity[];
  batches: CoordinationBatch[];
  team?: ConversationTeamState;
  conversationId?: string;
  onView: (view: ConversationView) => void;
  onOpenPreview: (preview: ConversationPreview) => void;
}) {
  const agentActivityIds = new Set(activities
    .filter((activity) => batches.some((batch) => activityBelongsToBatch(activity, batch, team)))
    .map((activity) => activity.codex_item_id));
  const visibleActivities = activities.filter((activity) => activity.kind !== "usage" && !agentActivityIds.has(activity.codex_item_id));
  let entries: ConversationTimelineEntry[] = [
    ...messages.map((message, stableOrder) => ({
      type: "message" as const,
      timestamp: Date.parse(message.created_at),
      stableOrder,
      message
    })),
    ...visibleActivities.map((activity, stableOrder) => ({
      type: "activity" as const,
      timestamp: Date.parse(activity.started_at),
      stableOrder: messages.length + stableOrder,
      activity
    })),
    ...batches.map((batch, stableOrder) => ({
      type: "batch" as const,
      timestamp: Date.parse(batch.created_at),
      stableOrder: messages.length + visibleActivities.length + stableOrder,
      batch
    }))
  ].sort((left, right) => left.timestamp - right.timestamp || left.stableOrder - right.stableOrder);

  // A coordination batch is the durable equivalent of a long-running tool call. Storage clocks
  // can place its atomic creation a few milliseconds before the Chat acknowledgement, while its
  // completion callback is appended much later. Preserve semantic chronology instead: the first
  // Chat message acknowledges the handoff, the batch stays directly beneath it, and every later
  // Chat message (including the final callback) follows the batch.
  const batchEntries = entries.filter((entry) => entry.type === "batch");
  const messageCount = entries.filter((entry) => entry.type === "message").length;
  if (batchEntries.length > 0 && messageCount > 0) {
    entries = entries.filter((entry) => entry.type !== "batch");
    const firstMessageIndex = entries.findIndex((entry) => entry.type === "message");
    const activeBatch = batchEntries.some((entry) => entry.type === "batch" && !["completed", "failed"].includes(entry.batch.state));
    const insertAt = messageCount > 1 || activeBatch ? firstMessageIndex + 1 : firstMessageIndex;
    entries.splice(insertAt, 0, ...batchEntries);
  }

  const content: ReactNode[] = [];
  const terminalBatch = batches.length > 0 && batches.every((batch) => ["completed", "failed"].includes(batch.state));
  const finalMessageId = terminalBatch && messages.length > 1 ? messages.at(-1)?.id : undefined;
  for (let index = 0; index < entries.length;) {
    const entry = entries[index];
    if (entry.type === "activity" && entry.activity.kind === "command") {
      const commands: ConversationActivity[] = [];
      while (index < entries.length) {
        const candidate = entries[index];
        if (candidate.type !== "activity" || candidate.activity.kind !== "command") break;
        commands.push(candidate.activity);
        index += 1;
      }
      content.push(<ConversationCommandGroup commands={commands} key={`commands-${commands[0].codex_item_id}`} />);
      continue;
    }
    if (entry.type === "batch") {
      const batchActivities = activities.filter((activity) => activityBelongsToBatch(activity, entry.batch, team));
      content.push(<section className={`conversation-batch-card state-${entry.batch.state}`} key={`batch-${entry.batch.id}`} aria-label="Delegated agent progress">
        <header><strong>{entry.batch.delivery_target === "codex_sites" ? "Building with ChatGPT Sites" : "Agent team progress"}</strong><small>{entry.batch.delivery_rationale}</small></header>
        <CoordinatorAgentCard team={team} batch={entry.batch} activities={batchActivities}>
          <CodingAgentCards team={team} conversationId={conversationId} delegationIds={entry.batch.delegation_ids} activities={batchActivities} onView={onView} />
        </CoordinatorAgentCard>
      </section>);
    } else if (entry.type === "activity") {
      const activity = entry.activity;
      const [icon, fallbackLabel] = conversationActivityPresentation(activity);
      const label = activity.command || fallbackLabel;
      const canExpand = Boolean(activity.output) && activity.kind !== "reasoning_summary";
      const row = <>
        <span className="chat-activity-icon" aria-hidden="true">{icon}</span>
        <span>{label}</span>
        {activity.state === "running" && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}
      </>;
      content.push(canExpand
        ? <details className={`chat-compaction state-${activity.state}`} key={activity.codex_item_id}>
            <summary>{row}</summary><pre className="chat-activity-detail" data-scroll-chain>{activity.output}</pre>
          </details>
        : <div className={`chat-compaction state-${activity.state}`} role="status" key={activity.codex_item_id}>{row}</div>);
    } else {
      const message = entry.message;
      content.push(<div className="chat-agent-result" key={message.id}>
        <article className="chat-message assistant-message">
          <span>Chat Agent</span><MarkdownContent content={message.content} onOpenLink={openSiteExternalUrl} />
        </article>
        {message.id === finalMessageId && <ConversationDeliveryHandoff batches={batches} team={team} conversationId={conversationId} onView={onView} onOpenPreview={onOpenPreview} />}
      </div>);
    }
    index += 1;
  }

  if (content.length === 0) return null;
  return <div className="chat-activity-timeline" aria-label="Turn activity">{content}</div>;
}

function formatAgentElapsed(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return `${minutes}m ${remainder.toString().padStart(2, "0")}s`;
}

function CodingAgentElapsed({ createdAt, updatedAt, active }: { createdAt: string; updatedAt: string; active: boolean }) {
  const elapsed = () => Math.max(0, Math.floor(((active ? Date.now() : Date.parse(updatedAt)) - Date.parse(createdAt)) / 1000));
  const [seconds, setSeconds] = useState(elapsed);
  useEffect(() => {
    setSeconds(elapsed());
    if (!active) return;
    const timer = window.setInterval(() => setSeconds(elapsed()), 1000);
    return () => window.clearInterval(timer);
  }, [active, createdAt, updatedAt]);
  return <span className="coding-agent-elapsed">{formatAgentElapsed(seconds)}</span>;
}

function codingAgentProgress(state: AgentDelegation["state"]): number {
  const progress: Record<AgentDelegation["state"], number> = {
    requested: 8,
    preparing: 24,
    running: 56,
    waiting: 56,
    blocked: 68,
    reported: 78,
    integrating: 90,
    completed: 100,
    failed: 100,
    cancelled: 100
  };
  return progress[state];
}

function codingAgentStateLabel(state: AgentDelegation["state"]): string {
  const labels: Record<AgentDelegation["state"], string> = {
    requested: "Queued",
    preparing: "Preparing worktree",
    running: "Coding",
    waiting: "Waiting for Chat Agent",
    blocked: "Needs attention",
    reported: "Ready for review",
    integrating: "Applying changes",
    completed: "Completed",
    failed: "Failed",
    cancelled: "Cancelled"
  };
  return labels[state];
}

function codingAgentIsActive(state: AgentDelegation["state"]): boolean {
  return ["requested", "preparing", "running", "waiting", "integrating"].includes(state);
}

export function conversationTeamHasPendingDelivery(team?: ConversationTeamState): boolean {
  if (!team) return false;
  return team.delegations.some((delegation) => codingAgentIsActive(delegation.state))
    || team.sessions.some((session) =>
      session.kind === "coordinator" && ["starting", "working", "waiting"].includes(session.state)
    )
    || (team.coordination_batches ?? []).some((batch) => !["completed", "failed"].includes(batch.state))
    || (team.wakes ?? []).some((wake) => ["pending", "claimed", "recovery_required"].includes(wake.state));
}

function coordinatorStateLabel(batch: CoordinationBatch, activeCoders: number): string {
  if (batch.state === "reviewing" || batch.state === "review_pending") return "Reviewing results";
  if (batch.state === "chat_pending") return "Handing results to Chat Agent";
  if (batch.state === "recovery_required") return "Needs recovery";
  if (batch.state === "completed") return "Coordination complete";
  if (batch.state === "failed") return "Coordination failed";
  if (activeCoders > 0) return `Supervising ${activeCoders} working agent${activeCoders === 1 ? "" : "s"}`;
  return "Planning and assigning work";
}

function CoordinatorAgentCard({ team, batch, activities, children }: {
  team?: ConversationTeamState;
  batch: CoordinationBatch;
  activities: ConversationActivity[];
  children?: ReactNode;
}) {
  const [tab, setTab] = useState<"chat_room" | "activity" | "exchange" | "blackboard" | "evidence">("chat_room");
  if (!team) return null;
  const coordinator = team.sessions.slice().reverse().find((session) => session.kind === "coordinator");
  const delegationIds = new Set(batch.delegation_ids);
  const activeCoders = team.delegations.filter((delegation) => delegationIds.has(delegation.id) && codingAgentIsActive(delegation.state)).length;
  const active = !["completed", "failed"].includes(batch.state);
  const messages = team.messages.filter((message) =>
    message.from_session_id === coordinator?.id || message.to_session_id === coordinator?.id
  );
  const blackboard = team.blackboard.filter((entry) => entry.author_session_id === coordinator?.id || (entry.delegation_id && delegationIds.has(entry.delegation_id)));
  const coordinatorActivities = activities.filter((activity) => activity.agent_session_id === coordinator?.id);
  const label = coordinatorStateLabel(batch, activeCoders);
  const progress = batch.state === "completed" || batch.state === "failed" ? 100
    : batch.state === "reviewing" || batch.state === "review_pending" ? 82
      : batch.state === "chat_pending" ? 94
        : activeCoders > 0 ? 48 : 18;
  return <details className={`coding-agent-card coordinator-agent-card state-${batch.state}`} aria-label="Coordinator activity">
    <summary>
      <span className="coding-agent-glyph coordinator" aria-hidden="true">◎</span>
      <span className="coding-agent-title">
        <strong>{coordinator?.display_name || "Coordinator"}</strong>
        <small>{label}</small>
        <span className="coding-agent-progress" role="progressbar" aria-label={`Coordinator progress: ${label}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={progress}><i style={{ width: `${progress}%` }} /></span>
      </span>
      <span className="coding-agent-state"><span>{label}</span>{active && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}</span>
      <span className="chat-activity-chevron" aria-hidden="true">⌄</span>
    </summary>
    <div className="coding-agent-detail">
      <div className="coding-agent-tabs" role="tablist" aria-label="Coordinator details">{(["chat_room", "activity", "exchange", "blackboard", "evidence"] as const).map((value) => <button type="button" role="tab" aria-selected={tab === value} className={tab === value ? "active" : ""} onClick={() => setTab(value)} key={value}>{value === "chat_room" ? "Chat room" : value === "exchange" ? "Agent Exchange" : value.replace(/^./, (letter) => letter.toUpperCase())}</button>)}</div>
      {tab === "chat_room" && <AgentChatRoom session={coordinator} sessions={team.sessions} messages={messages} activities={coordinatorActivities}>{children}</AgentChatRoom>}
      {tab === "activity" && <section><h4>Activity</h4><p>{label}</p><p>{batch.routing_rationale || batch.delivery_rationale}</p><div className="coding-agent-meta">
        {coordinator?.model && <span>Model <code>{coordinator.model}</code></span>}
        <span>Route <code>{batch.coordinator_bypassed ? "deterministic single-Coder" : "Coordinator model"}</code></span>
        <span>Turns <code>{batch.coordinator_turn_count ?? 0}</code></span>
        <span>Estimated tokens <code>~{(batch.coordinator_approximate_tokens ?? 0).toLocaleString()}</code></span>
        <span>Owner interventions <code>{batch.owner_intervention_count ?? 0}</code></span>
      </div></section>}
      {tab === "exchange" && <section><h4>Agent exchange</h4>{messages.length === 0 ? <p>No messages recorded yet.</p> : messages.map((message) => <article key={message.id}><small>{message.tool_name || message.kind.replaceAll("_", " ")}</small><p>{message.content}</p></article>)}</section>}
      {tab === "blackboard" && <section><h4>Blackboard</h4>{blackboard.length === 0 ? <p>No shared entries yet.</p> : blackboard.map((entry) => <article key={entry.id}><small>{entry.kind}</small><p>{entry.content}</p></article>)}</section>}
      {tab === "evidence" && <section><h4>Evidence</h4><p>{batch.delegation_ids.length} delegated agent{batch.delegation_ids.length === 1 ? "" : "s"}</p><p>{batch.coordinator_turn_count ?? 0} Coordinator model turn{batch.coordinator_turn_count === 1 ? "" : "s"}</p><p>Approximate Coordinator tokens: {(batch.coordinator_approximate_tokens ?? 0).toLocaleString()}</p></section>}
    </div>
  </details>;
}

function ConversationDeliveryHandoff({ batches, team, conversationId, onView, onOpenPreview }: {
  batches: CoordinationBatch[];
  team?: ConversationTeamState;
  conversationId?: string;
  onView: (view: ConversationView) => void;
  onOpenPreview: (preview: ConversationPreview) => void;
}) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  if (!team || batches.length === 0) return null;
  const batchIds = new Set(batches.map((batch) => batch.id));
  const delegationIds = new Set(batches.flatMap((batch) => batch.delegation_ids));
  const attempts = team.delegations.filter((delegation) => delegationIds.has(delegation.id));
  const delegations = effectiveDelegationAttempts(attempts);
  const previews = (team.previews ?? []).filter((preview) => batchIds.has(preview.coordination_batch_id));
  const preview = previews.slice().sort((left, right) => {
    const priority = (candidate: ConversationPreview) => candidate.strategy === "codex_sites" && candidate.safe_url ? 3 : candidate.safe_url ? 2 : candidate.state === "stopped" ? 1 : 0;
    return priority(right) - priority(left) || right.updated_at.localeCompare(left.updated_at);
  })[0];
  const completed = delegations.filter((delegation) => delegation.state === "completed").length;
  const needsReview = delegations.filter((delegation) => ["reported", "blocked"].includes(delegation.state)).length;
  const failed = delegations.filter((delegation) => ["failed", "cancelled"].includes(delegation.state)).length;
  const changedFiles = new Set(delegations.flatMap((delegation) => delegation.changed_files)).size;
  const checks = delegations.reduce((total, delegation) => total + delegation.checks.length, 0);
  const success = failed === 0 && batches.every((batch) => batch.state === "completed");
  const hardGateReady = success && delegations.length > 0 && delegations.every((delegation) =>
    delegation.governance?.preflight.passed !== false
    && delegation.governance?.outcome.successful !== false
    && delegation.integration_ready !== false
  );

  async function runPreviewAction(action: "open" | "copy" | "stop") {
    if (!conversationId || !preview) return;
    setBusy(action);
    setError("");
    try {
      if (action === "stop") {
        await stopConversationPreview(conversationId, preview.id);
        onView(await getConversation(conversationId));
      } else if (action === "copy") {
        if (!preview.safe_url) throw new Error("Open the preview once before copying its temporary URL.");
        await navigator.clipboard.writeText(preview.safe_url);
      } else {
        const running = preview.state === "running" && preview.safe_url
          ? preview
          : await startConversationPreview(conversationId, preview.id);
        onOpenPreview(running);
        onView(await getConversation(conversationId));
      }
    } catch (actionError) {
      setError(errorMessage(actionError));
    } finally {
      setBusy(null);
    }
  }

  async function copyReceipt() {
    setBusy("receipt");
    setError("");
    try {
      await navigator.clipboard.writeText(JSON.stringify({
        schema_version: 1,
        conversation_id: conversationId,
        batch_ids: batches.map((batch) => batch.id),
        hard_gate_ready: hardGateReady,
        effective_agent_count: delegations.length,
        attempt_count: attempts.length,
        changed_file_count: changedFiles,
        check_count: checks,
        coordinator_turn_count: batches.reduce((total, batch) => total + batch.coordinator_turn_count, 0),
        coordinator_approximate_tokens: batches.reduce((total, batch) => total + batch.coordinator_approximate_tokens, 0),
        delegations: delegations.map((delegation) => ({
          id: delegation.id,
          state: delegation.state,
          report_sha256: delegation.report_sha256,
          changed_files: delegation.changed_files,
          checks: delegation.checks,
          governance: delegation.governance
        }))
      }, null, 2));
    } catch (copyError) {
      setError(errorMessage(copyError));
    } finally {
      setBusy(null);
    }
  }

  return <section className={`chat-delivery-handoff ${success ? "success" : "attention"}`} aria-label="Chat Agent result actions">
    <div className="chat-delivery-heading">
      <span aria-hidden="true">{success ? "✓" : "!"}</span>
      <span><strong>{success ? "Result ready" : "Result needs attention"}</strong><small>
        {delegations.length} effective agent{delegations.length === 1 ? "" : "s"} · {attempts.length} attempt{attempts.length === 1 ? "" : "s"} · {completed} completed{needsReview > 0 ? ` · ${needsReview} ready for review` : ""}{failed > 0 ? ` · ${failed} failed` : ""}
      </small></span>
    </div>
    <div className="chat-delivery-summary">
      {changedFiles > 0 && <span>{changedFiles} changed file{changedFiles === 1 ? "" : "s"}</span>}
      {checks > 0 && <span>{checks} check{checks === 1 ? "" : "s"}</span>}
      <span>{hardGateReady ? "Verification passed" : "Verification needs review"}</span>
      <span>{batches.reduce((total, batch) => total + batch.coordinator_turn_count, 0)} Coordinator turn{batches.reduce((total, batch) => total + batch.coordinator_turn_count, 0) === 1 ? "" : "s"}</span>
      {preview && <span>{preview.strategy === "codex_sites" ? "ChatGPT Sites" : "Temporary local preview"} · {preview.state.replaceAll("_", " ")}</span>}
    </div>
    {preview && <div className="conversation-preview-actions">
      {!["failed", "stale"].includes(preview.state) && <button type="button" className="primary" disabled={busy !== null} onClick={() => void runPreviewAction("open")}>Open preview</button>}
      {preview.safe_url && <button type="button" className="secondary" disabled={busy !== null} onClick={() => void runPreviewAction("copy")}>Copy URL</button>}
      {preview.strategy !== "codex_sites" && preview.state === "running" && <button type="button" className="secondary" disabled={busy !== null} onClick={() => void runPreviewAction("stop")}>Stop preview</button>}
    </div>}
    <div className="conversation-preview-actions"><button type="button" className="secondary" disabled={busy !== null} onClick={() => void copyReceipt()}>Copy verification receipt</button></div>
    {(error || preview?.last_error) && <p className="error-message" role="alert">{error || preview?.last_error}</p>}
  </section>;
}

export function effectiveDelegationAttempts(delegations: AgentDelegation[]): AgentDelegation[] {
  const retriedIds = new Set(delegations.flatMap((delegation) => delegation.retry_of_id ? [delegation.retry_of_id] : []));
  return delegations.filter((delegation) => !retriedIds.has(delegation.id));
}

function delegationAttemptChain(delegation: AgentDelegation, attempts: AgentDelegation[]): AgentDelegation[] {
  const byId = new Map(attempts.map((attempt) => [attempt.id, attempt]));
  const chain: AgentDelegation[] = [delegation];
  let current = delegation;
  const seen = new Set([delegation.id]);
  while (current.retry_of_id) {
    const parent = byId.get(current.retry_of_id);
    if (!parent || seen.has(parent.id)) break;
    seen.add(parent.id);
    chain.unshift(parent);
    current = parent;
  }
  return chain;
}

function CodingAgentCards({ team, conversationId, delegationIds, activities, onView }: { team?: ConversationTeamState; conversationId?: string; delegationIds?: string[]; activities: ConversationActivity[]; onView: (view: ConversationView) => void }) {
  const [tabs, setTabs] = useState<Record<string, "chat_room" | "activity" | "exchange" | "blackboard" | "changes" | "evidence">>({});
  const [patches, setPatches] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const allAttempts = team?.delegations.filter((delegation) => !delegationIds || delegationIds.includes(delegation.id)) ?? [];
  const visibleDelegations = effectiveDelegationAttempts(allAttempts);
  if (!team || visibleDelegations.length === 0) return null;
  const stateLabel = codingAgentStateLabel;
  async function runAction(delegation: AgentDelegation, action: "review" | "cancel" | "apply" | "discard" | "retry", worktreeRevision?: number) {
    if (!conversationId) return;
    setBusy(`${delegation.id}:${action}`);
    setErrors((current) => ({ ...current, [delegation.id]: "" }));
    try {
      if (action === "review") {
        const review = await reviewConversationDelegation(conversationId, delegation.id);
        setPatches((current) => ({ ...current, [delegation.id]: review.patch || "No file changes were reported." }));
        setTabs((current) => ({ ...current, [delegation.id]: "changes" }));
      } else if (action === "cancel") {
        onView(await cancelConversationDelegation(conversationId, delegation.id, delegation.revision));
      } else if (action === "apply") {
        onView(await applyConversationDelegation(conversationId, delegation.id, delegation.revision));
      } else if (action === "retry") {
        await retryConversationDelegation(conversationId, delegation.id, delegation.revision, crypto.randomUUID());
        onView(await getConversation(conversationId));
      } else if (worktreeRevision !== undefined) {
        onView(await discardConversationDelegation(conversationId, delegation.id, worktreeRevision));
      }
    } catch (error) {
      setErrors((current) => ({ ...current, [delegation.id]: errorMessage(error) }));
    } finally {
      setBusy(null);
    }
  }
  return <section className="coding-agent-stack" aria-label="Coding agent activity">
    {visibleDelegations.map((delegation, index) => {
      const session = team.sessions.find((candidate) => candidate.id === delegation.coder_session_id);
      const tester = session?.kind === "tester";
      const roleLabel = tester ? "Tester" : "Coding Agent";
      const worktree = team.worktrees.find((candidate) => candidate.id === delegation.worktree_id);
      const messages = team.messages.filter((message) => message.delegation_id === delegation.id);
      const blackboard = team.blackboard.filter((entry) => entry.delegation_id === delegation.id);
      const sessionActivities = activities.filter((activity) =>
        activity.delegation_id === delegation.id
        || activity.agent_session_id === session?.id
        || (!activity.agent_session_id && !activity.delegation_id && visibleDelegations.length === 1)
      );
      const active = codingAgentIsActive(delegation.state);
      const stateText = tester && delegation.state === "running" ? "Testing" : stateLabel(delegation.state);
      const tab = tabs[delegation.id] ?? "chat_room";
      const cancellable = ["requested", "preparing", "running", "waiting"].includes(delegation.state);
      const reviewable = !tester && Boolean(worktree) && ["reported", "blocked", "failed", "cancelled"].includes(delegation.state);
      const applicable = !tester && Boolean(worktree) && ["reported", "blocked"].includes(delegation.state);
      const discardable = !tester && Boolean(worktree) && worktree?.state !== "removed" && !["preparing", "running", "integrating"].includes(delegation.state);
      const retryable = ["failed", "cancelled", "blocked"].includes(delegation.state);
      const governance = delegation.governance;
      const needsAttention = Boolean(governance && (governance.preflight.blockers.length > 0 || governance.failure_class !== "none"));
      const latestContext = governance?.context_receipts.at(-1);
      const attemptHistory = delegationAttemptChain(delegation, allAttempts);
      return <details className={`coding-agent-card state-${delegation.state}`} key={delegation.id} open={needsAttention}>
        <summary>
          <span className="coding-agent-glyph" aria-hidden="true">{tester ? "✓" : "⌘"}</span>
          <span className="coding-agent-title">
            <strong>{session?.display_name || `${roleLabel} ${index + 1}`}</strong>
            <small>{delegation.brief.objective}</small>
            <span className="coding-agent-progress" role="progressbar" aria-label={`${roleLabel} ${index + 1} progress: ${stateText}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={codingAgentProgress(delegation.state)}><i style={{ width: `${codingAgentProgress(delegation.state)}%` }} /></span>
          </span>
          <span className="coding-agent-state"><span>{stateText}</span><CodingAgentElapsed createdAt={delegation.created_at} updatedAt={delegation.updated_at} active={active} />{active && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}</span>
          <span className="chat-activity-chevron" aria-hidden="true">⌄</span>
        </summary>
        <div className="coding-agent-detail">
          <div className="coding-agent-tabs" role="tablist" aria-label={`${roleLabel} ${index + 1} details`}>{(["chat_room", "activity", "exchange", "blackboard", "changes", "evidence"] as const).filter((value) => !tester || value !== "changes").map((value) => <button type="button" role="tab" aria-selected={tab === value} className={tab === value ? "active" : ""} onClick={() => setTabs((current) => ({ ...current, [delegation.id]: value }))} key={value}>{value === "chat_room" ? "Chat room" : value === "exchange" ? "Agent Exchange" : value.replace(/^./, (letter) => letter.toUpperCase())}</button>)}</div>
          {tab === "chat_room" && <AgentChatRoom session={session} sessions={team.sessions} messages={messages} activities={sessionActivities} />}
          {tab === "activity" && <section><h4>Activity</h4><p>Status: {stateText}</p><p>Started {new Date(delegation.created_at).toLocaleString()}</p><div className="coding-agent-meta">
            {session?.model && <span>Model <code>{session.model}</code></span>}
            <span>Prompt revision <code>{session?.prompt_revision ?? "—"}</code></span>
            {worktree && <span>Branch <code>{worktree.branch}</code></span>}
            {delegation.run_id && <span>Run <code>{delegation.run_id}</code></span>}
            {delegation.agent_lease_id && <span>Lease <code>{delegation.agent_lease_id}</code></span>}
            {governance && <span>Preflight <code>{governance.preflight.passed ? "passed" : "blocked"}</code></span>}
            {governance && <span>Attempts <code>{governance.outcome.attempts}</code></span>}
            {governance?.outcome.latency_ms ? <span>Runtime <code>{Math.round(governance.outcome.latency_ms / 1000)}s</code></span> : null}
          </div>{worktree && <p>Isolation: {(worktree.workspace_mode ?? "git_worktree").replaceAll("_", " ")} · baseline {(worktree.baseline_confidence ?? "verified").replaceAll("_", " ")}</p>}{governance?.preflight.checks.map((check) => <p key={check}>✓ {check}</p>)}{governance?.preflight.blockers.map((blocker) => <p className="error-message" key={blocker}>Preflight: {blocker}</p>)}{latestContext && <p>Context receipt: ~{latestContext.approximate_tokens.toLocaleString()} / {latestContext.token_budget.toLocaleString()} tokens · {latestContext.source_count} attributed sources{latestContext.truncated ? " · truncated to budget" : ""}</p>}{governance && governance.failure_class !== "none" && <p className="error-message">Failure class: {governance.failure_class.replaceAll("_", " ")} · recovery: {governance.recovery.replaceAll("_", " ")}</p>}{worktree?.retained_reason && <p>Retained: {worktree.retained_reason}</p>}{attemptHistory.length > 1 && <details><summary>Attempt history <span>{attemptHistory.length}</span></summary>{attemptHistory.map((attempt, attemptIndex) => <p key={attempt.id}>Attempt {attemptIndex + 1}: {stateLabel(attempt.state)} · <code>{attempt.id}</code>{attempt.error ? ` · ${attempt.error}` : ""}</p>)}</details>}</section>}
          {tab === "exchange" && <section><h4>Agent exchange</h4>{messages.length === 0 ? <p>No messages recorded yet.</p> : messages.map((message) => <article key={message.id}><small>{message.kind.replaceAll("_", " ")}</small><p>{message.content}</p></article>)}</section>}
          {tab === "blackboard" && <section><h4>Blackboard</h4>{blackboard.length === 0 ? <p>No shared entries yet.</p> : blackboard.map((entry) => <article key={entry.id}><small>{entry.kind}</small><p>{entry.content}</p></article>)}</section>}
          {tab === "changes" && <section><h4>Changes</h4>{delegation.changed_files.length === 0 ? <p>No changed files reported yet.</p> : delegation.changed_files.map((file) => <code key={file}>{file}</code>)}{patches[delegation.id] && <pre className="coding-agent-patch" data-scroll-chain>{patches[delegation.id]}</pre>}</section>}
          {tab === "evidence" && <section><h4>Evidence</h4>{delegation.checks.length === 0 ? <p>No checks reported yet.</p> : delegation.checks.map((check) => <p key={check}>{check}</p>)}{(delegation.evidence ?? []).map((item) => <p key={item}>{item}</p>)}{(delegation.unresolved_issues ?? []).map((item) => <p className="error-message" key={item}>{item}</p>)}{delegation.summary && <p>{delegation.summary}</p>}{worktree?.baseline_exclusions?.map((item) => <p key={item.normalized_path}>{item.normalized_path}: {item.reason}{item.review_blocker ? " (review blocker)" : ""}</p>)}{worktree?.integration_journal && <p>Integration journal: {worktree.integration_journal.state.replaceAll("_", " ")}</p>}</section>}
          <div className="coding-agent-actions">{cancellable && <button type="button" className="secondary" disabled={busy !== null} onClick={() => void runAction(delegation, "cancel")}>Cancel</button>}{reviewable && <button type="button" className="secondary" disabled={busy !== null} onClick={() => void runAction(delegation, "review")}>Review</button>}{applicable && <button type="button" className="primary" disabled={busy !== null} onClick={() => void runAction(delegation, "apply")}>Apply</button>}{retryable && <button type="button" className="secondary" disabled={busy !== null} onClick={() => void runAction(delegation, "retry")}>Retry</button>}{discardable && <button type="button" className="secondary danger" disabled={busy !== null} onClick={() => void runAction(delegation, "discard", worktree?.revision)}>Discard</button>}</div>
          {(delegation.error || errors[delegation.id]) && <p className="error-message" role="alert">{errors[delegation.id] || delegation.error}</p>}
        </div>
      </details>;
    })}
  </section>;
}

function ConversationTurnTranscript({
  turn,
  messages,
  activities,
  active,
  batches,
  team,
  conversationId,
  onView,
  onOpenPreview
}: {
  turn: ConversationTurn;
  messages: ConversationMessage[];
  activities: ConversationActivity[];
  active: boolean;
  batches: CoordinationBatch[];
  team?: ConversationTeamState;
  conversationId?: string;
  onView: (view: ConversationView) => void;
  onOpenPreview: (preview: ConversationPreview) => void;
}) {
  const visibleMessages = messages.filter((message) => message.role !== "assistant" || message.content.length > 0);
  const assistantMessages = visibleMessages.filter((message) => message.role !== "user");
  const hasResponseText = messages.some((message) => message.role === "assistant" && message.content.length > 0);
  return <section className="chat-turn" aria-label="Conversation turn">
    {visibleMessages.filter((message) => message.role === "user").map((message) => <article className="chat-message user-message" key={message.id}>
      <span>You</span><MarkdownContent content={message.content} onOpenLink={openSiteExternalUrl} /><MentionChips mentions={message.mentions ?? []} />
    </article>)}
    {(activities.length > 0 || assistantMessages.length > 0 || batches.length > 0) && <ConversationActivityTimeline messages={assistantMessages} activities={activities} batches={batches} team={team} conversationId={conversationId} onView={onView} onOpenPreview={onOpenPreview} />}
    {active && <ConversationTurnStatus turn={turn} hasResponseText={hasResponseText} />}
    {turn.error && <p className="error-message" role="alert">{turn.error}</p>}
  </section>;
}

function accountPlanLabel(plan?: string): string {
  if (!plan) return "Account type unavailable";
  const labels: Record<string, string> = {
    free: "ChatGPT Free",
    plus: "ChatGPT Plus",
    pro: "ChatGPT Pro",
    team: "ChatGPT Team",
    business: "ChatGPT Business",
    enterprise: "ChatGPT Enterprise",
    edu: "ChatGPT Edu"
  };
  return labels[plan.toLocaleLowerCase()] ?? `ChatGPT ${plan.replace(/[_-]+/g, " ").replace(/\b\w/g, (letter) => letter.toLocaleUpperCase())}`;
}

function usageWindowLabel(window: CodexUsageWindow, fallback: string): string {
  if (!window.window_seconds) return fallback;
  const hours = window.window_seconds / 3600;
  if (hours < 24) return `${Math.round(hours)}-hour limit`;
  const days = hours / 24;
  return `${Math.round(days)}-day limit`;
}

function usageResetLabel(resetAt?: number): string {
  if (!resetAt) return "Reset time unavailable";
  return `Resets ${new Date(resetAt * 1000).toLocaleString()}`;
}

function usageResetDateLabel(resetAt?: number): string {
  if (!resetAt) return "Date unavailable";
  return new Date(resetAt * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric"
  });
}

type StoredAccountAvatar = { accountKey: string; dataUrl: string };

const ACCOUNT_AVATAR_STORAGE_KEY = "mcg.account-avatar";
const MAX_ACCOUNT_AVATAR_BYTES = 2 * 1024 * 1024;
const ACCOUNT_AVATAR_TYPES = new Set(["image/jpeg", "image/png", "image/webp", "image/gif"]);

function AccountAvatar({ profile, avatarUrl, fallback, className }: { profile?: OAuthAccountProfile; avatarUrl?: string; fallback: string; className: string }) {
  const [imageFailed, setImageFailed] = useState(false);
  const label = profile?.display_name || profile?.username || profile?.email || fallback;
  const initial = label.trim().slice(0, 1).toLocaleUpperCase() || fallback.slice(0, 1).toLocaleUpperCase();
  const imageUrl = avatarUrl || profile?.avatar_url;
  useEffect(() => setImageFailed(false), [imageUrl]);
  return <span className={className} aria-hidden="true">
    {imageUrl && !imageFailed
      ? <img src={imageUrl} alt="" referrerPolicy="no-referrer" onError={() => setImageFailed(true)} />
      : initial}
  </span>;
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
type ThemePreference = "system" | "light" | "dark";
type WorkspacePanelTab = "browser" | "files" | "activity";
type WorkspacePopover = "open-in" | "share" | null;

interface TerminalEntry {
  id: string;
  workingDirectory: string;
  command: string;
  result?: TerminalCommandResult;
  error?: string;
}

const METACOG_SHORTCUTS = [
  ["New chat", "Start a new chat", "Ctrl+N"],
  ["Open global folder", "Make a folder available to every project and general chat", "Ctrl+O"],
  ["Settings", "Open MetaCog settings", "Ctrl+,"],
  ["Find a chat", "Search chats in the sidebar", "Ctrl+F"],
  ["Toggle sidebar", "Show or hide the sidebar", "Ctrl+B"],
  ["Toggle file tree", "Show or hide the project file tree", "Ctrl+Shift+E"],
  ["Toggle side panel", "Show or hide browser, files, and activity", "Ctrl+Alt+B"],
  ["Toggle bottom panel", "Show or hide the project terminal", "Ctrl+J"],
  ["Previous chat", "Switch to the previous chat", "Ctrl+Shift+["],
  ["Next chat", "Switch to the next chat", "Ctrl+Shift+]"],
  ["Back", "Go back in navigation history", "Ctrl+["],
  ["Forward", "Go forward in navigation history", "Ctrl+]"],
  ["Find on page", "Find text in the active page", "Ctrl+F"],
  ["Close", "Close the active window", "Ctrl+W"],
  ["Zoom in", "Increase interface scale", "Ctrl+Shift+="],
  ["Zoom out", "Decrease interface scale", "Ctrl+-"],
  ["Actual size", "Reset interface scale", "Ctrl+0"],
  ["Toggle full screen", "Enter or leave full screen", "F11"]
] as const;

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
  const accountAvatarInputRef = useRef<HTMLInputElement>(null);
  const terminalInputRef = useRef<HTMLInputElement>(null);
  const terminalOutputRef = useRef<HTMLDivElement>(null);
  const workspaceRef = useRef<HTMLElement>(null);
  const scrollChainRootRef = useRef<HTMLDivElement>(null);
  const chatViewportRef = useRef<HTMLDivElement>(null);
  const chatShouldFollowRef = useRef(true);
  const queueDispatchingRef = useRef<string | null>(null);
  const [activeSurface, setActiveSurface] = useState("tasks");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [chatSearch, setChatSearch] = useState("");
  const [selectedChatId, setSelectedChatId] = useState<string | null>(null);
  const [ownerMenuOpen, setOwnerMenuOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("general");
  const [settingsSearch, setSettingsSearch] = useState("");
  const [shortcutSearch, setShortcutSearch] = useState("");
  const [storedAccountAvatar, setStoredAccountAvatar] = useState<StoredAccountAvatar | null>(() => {
    try {
      const parsed = JSON.parse(window.localStorage.getItem(ACCOUNT_AVATAR_STORAGE_KEY) || "null") as Partial<StoredAccountAvatar> | null;
      return parsed && typeof parsed.accountKey === "string" && /^data:image\/(?:jpeg|png|webp|gif);base64,/i.test(parsed.dataUrl || "")
        ? { accountKey: parsed.accountKey, dataUrl: parsed.dataUrl as string }
        : null;
    } catch {
      return null;
    }
  });
  const [accountAvatarError, setAccountAvatarError] = useState<string | null>(null);
  const [themePreference, setThemePreference] = useState<ThemePreference>(() => {
    const stored = window.localStorage.getItem("mcg.theme");
    return stored === "light" || stored === "dark" || stored === "system" ? stored : "dark";
  });
  const [reduceMotion, setReduceMotion] = useState(() => window.localStorage.getItem("mcg.reduce-motion") === "true");
  const [desktopMenu, setDesktopMenu] = useState<string | null>(null);
  const [pinnedSummaryOpen, setPinnedSummaryOpen] = useState(true);
  const [projectTreeOpen, setProjectTreeOpen] = useState(true);
  const [recentsOpen, setRecentsOpen] = useState(() => window.localStorage.getItem("mcg.sidebar-recents-open") !== "false");
  const [archivedChatsOpen, setArchivedChatsOpen] = useState(() => window.localStorage.getItem("mcg.sidebar-archived-open") === "true");
  const [workspacePopover, setWorkspacePopover] = useState<WorkspacePopover>(null);
  const [workspaceSidePanelOpen, setWorkspaceSidePanelOpen] = useState(() => window.localStorage.getItem("mcg.workspace-side-panel") === "true");
  const [workspaceSidePanelTab, setWorkspaceSidePanelTab] = useState<WorkspacePanelTab>("browser");
  const [workspaceBottomPanelOpen, setWorkspaceBottomPanelOpen] = useState(() => window.localStorage.getItem("mcg.workspace-bottom-panel") === "true");
  const [workspacePinnedSummaryOpen, setWorkspacePinnedSummaryOpen] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(() => Number(window.localStorage.getItem("mcg.sidebar-width")) || 256);
  const [settingsNavWidth, setSettingsNavWidth] = useState(() => Number(window.localStorage.getItem("mcg.settings-nav-width")) || 244);
  const [workspaceSideWidth, setWorkspaceSideWidth] = useState(() => Number(window.localStorage.getItem("mcg.workspace-side-width")) || 520);
  const [workspaceBottomHeight, setWorkspaceBottomHeight] = useState(() => Number(window.localStorage.getItem("mcg.workspace-bottom-height")) || 260);
  const [openInApplications, setOpenInApplications] = useState<OpenInApplication[]>([]);
  const [workspaceActionStatus, setWorkspaceActionStatus] = useState<string | null>(null);
  const [sideBrowserUrl, setSideBrowserUrl] = useState("https://example.com");
  const [siteBrowserTabs, setSiteBrowserTabs] = useState<SiteBrowserTab[]>([]);
  const [activeSiteBrowserTabId, setActiveSiteBrowserTabId] = useState<string | null>(null);
  const [selectedBrowserSessionId, setSelectedBrowserSessionId] = useState<string | null>(null);
  const [terminalWorkingDirectory, setTerminalWorkingDirectory] = useState("");
  const [terminalCommand, setTerminalCommand] = useState("");
  const [terminalEntries, setTerminalEntries] = useState<TerminalEntry[]>([]);
  const [terminalBusy, setTerminalBusy] = useState(false);
  const [zoomLevel, setZoomLevel] = useState(1);
  const [pinnedTaskIds, setPinnedTaskIds] = useState<string[]>(() => {
    try {
      const stored = window.localStorage.getItem("mcg.pinned-chat-ids") ?? window.localStorage.getItem("mcg.pinned-task-ids");
      return stored ? JSON.parse(stored) as string[] : [];
    } catch {
      return [];
    }
  });
  const [unreadChatIds, setUnreadChatIds] = useState<string[]>(() => {
    try {
      const stored = window.localStorage.getItem("mcg.unread-chat-ids");
      return stored ? JSON.parse(stored) as string[] : [];
    } catch {
      return [];
    }
  });
  const [chatContextMenu, setChatContextMenu] = useState<ChatContextMenu>(null);
  const [chatMutationBusy, setChatMutationBusy] = useState(false);
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
  const [protectedRoot, setProtectedRoot] = useState<ProtectedRootStatus | null>(null);
  const [canonicalWorkflows, setCanonicalWorkflows] = useState<CanonicalWorkflowAttempt[]>([]);
    const [workflowInspections, setWorkflowInspections] = useState<Record<string, CanonicalWorkflowInspection>>({});
    const [workflowActivities, setWorkflowActivities] = useState<Record<string, CanonicalWorkflowInvocationActivity>>({});
  const [workflowBusy, setWorkflowBusy] = useState<string | null>(null);
  const [workflowError, setWorkflowError] = useState<string | null>(null);
  const [canonicalActions, setCanonicalActions] = useState<CanonicalActionSummary[]>([]);
  const [actionInspections, setActionInspections] = useState<Record<string, CanonicalActionInspection>>({});
  const [actionBusy, setActionBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [compensationReasons, setCompensationReasons] = useState<Record<string, string>>({});
  const [compensationIdempotencyKeys, setCompensationIdempotencyKeys] = useState<Record<string, string>>({});
  const [canonicalInvocations, setCanonicalInvocations] = useState<CanonicalInvocationSummary[]>([]);
  const [invocationInspections, setInvocationInspections] = useState<Record<string, CanonicalInvocationInspection>>({});
  const [focusedInvocationId, setFocusedInvocationId] = useState<string | null>(null);
  const [invocationBusy, setInvocationBusy] = useState<string | null>(null);
  const [invocationError, setInvocationError] = useState<string | null>(null);
  const [toolAuthorizations, setToolAuthorizations] = useState<ToolAuthorizationSummary[]>([]);
  const [toolAuthorizationInspections, setToolAuthorizationInspections] = useState<Record<string, ToolAuthorizationInspection>>({});
  const [capabilityBusy, setCapabilityBusy] = useState<string | null>(null);
  const [capabilityError, setCapabilityError] = useState<string | null>(null);
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
  const [codexLogin, setCodexLogin] = useState<CodexLoginStart | null>(null);
  const [codexLoginMethod, setCodexLoginMethod] = useState<"api_key" | "access_token">("api_key");
  const [codexCredential, setCodexCredential] = useState("");
  const [codexRemoteEndpoint, setCodexRemoteEndpoint] = useState("");
  const [codexRemoteAuthTokenEnv, setCodexRemoteAuthTokenEnv] = useState("");
  const [codexTransportBusy, setCodexTransportBusy] = useState(false);
  const [codexTransportError, setCodexTransportError] = useState<string | null>(null);
  const [codexAccountDisconnected, setCodexAccountDisconnected] = useState(() => window.localStorage.getItem("mcg.codex-account-disconnected") === "true");
  const [providerStatus, setProviderStatus] = useState("Not connected");
  const [busy, setBusy] = useState(false);
  const [providerError, setProviderError] = useState<string | null>(null);
  const [providers, setProviders] = useState<ProviderListItem[]>([]);
  const [githubConnection, setGitHubConnection] = useState<GitHubConnectionStatus | null>(null);
  const [githubRepositories, setGitHubRepositories] = useState<GitHubRepository[]>([]);
  const [githubBusy, setGitHubBusy] = useState(false);
  const [githubError, setGitHubError] = useState<string | null>(null);
  const [githubRepositoryName, setGitHubRepositoryName] = useState("");
  const [githubRepositoryPrivate, setGitHubRepositoryPrivate] = useState(true);
  const [modelDiscoveryBusy, setModelDiscoveryBusy] = useState(false);
  const [modelDiscoveryError, setModelDiscoveryError] = useState<string | null>(null);
  const [codexUsageBusy, setCodexUsageBusy] = useState(false);
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
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [conversationViews, setConversationViews] = useState<Record<string, ConversationView | undefined>>({});
  const [codexModels, setCodexModels] = useState<CodexModel[]>([]);
  const [codexFeatures, setCodexFeatures] = useState<CodexFeature[]>([]);
  const [codexPermissionProfiles, setCodexPermissionProfiles] = useState<CodexPermissionProfile[]>([]);
  const [codexFeatureSearch, setCodexFeatureSearch] = useState("");
  const [codexFeatureBusy, setCodexFeatureBusy] = useState<string | null>(null);
  const [codexFeatureError, setCodexFeatureError] = useState<string | null>(null);
  const [windowsSandboxBusy, setWindowsSandboxBusy] = useState<"setup" | "read" | null>(null);
  const [windowsSandboxError, setWindowsSandboxError] = useState<string | null>(null);
  const [windowsSandboxNotice, setWindowsSandboxNotice] = useState<string | null>(null);
  const [windowsSandboxProjectId, setWindowsSandboxProjectId] = useState("");
  const [windowsSandboxReadPath, setWindowsSandboxReadPath] = useState("");
  const [windowsSandboxPermissionProfile, setWindowsSandboxPermissionProfile] = useState(":workspace");
  const [windowsSandboxPermissionProfiles, setWindowsSandboxPermissionProfiles] = useState<CodexPermissionProfile[]>([]);
  const [codexAccount, setCodexAccount] = useState<CodexAccount | null>(null);
  const [codexRateLimits, setCodexRateLimits] = useState<CodexRateLimits | null>(null);
  const [codexHarnessStatus, setCodexHarnessStatus] = useState<CodexHarnessStatus | null>(null);
  const [codexConfigurationStatus, setCodexConfigurationStatus] = useState<CodexConfigurationStatus | null>(null);
  const [codexPendingRequests, setCodexPendingRequests] = useState<CodexPendingRequest[]>([]);
  const [codexStableDiscovery, setCodexStableDiscovery] = useState<CodexStableDiscovery | null>(null);
  const [codexHookBusy, setCodexHookBusy] = useState<string | null>(null);
  const [codexHookError, setCodexHookError] = useState<string | null>(null);
  const [codexRecoveryBusy, setCodexRecoveryBusy] = useState(false);
  const [codexReviewBusy, setCodexReviewBusy] = useState(false);
  const [draftThreadId, setDraftThreadId] = useState(() => crypto.randomUUID());
  const [taskGoal, setTaskGoal] = useState("");
  const [composerMentions, setComposerMentions] = useState<PendingComposerMention[]>([]);
  const [mentionCandidates, setMentionCandidates] = useState<MentionCandidate[]>([]);
  const [mentionPaletteOpen, setMentionPaletteOpen] = useState(false);
  const [mentionQuery, setMentionQuery] = useState("");
  const [appPaletteOpen, setAppPaletteOpen] = useState(false);
  const [appQuery, setAppQuery] = useState("");
  const [slashPaletteOpen, setSlashPaletteOpen] = useState(false);
  const [commandDialog, setCommandDialog] = useState<"status" | "feedback" | "personality" | "model" | "reasoning" | "notice" | null>(null);
  const [commandNotice, setCommandNotice] = useState({ title: "", detail: "" });
  const [feedbackDraft, setFeedbackDraft] = useState("");
  const [feedbackIncludeDiagnostics, setFeedbackIncludeDiagnostics] = useState(false);
  const [goalEditOpen, setGoalEditOpen] = useState(false);
  const [goalDraft, setGoalDraft] = useState("");
  const [planningAnswers, setPlanningAnswers] = useState<Record<string, string>>({});
  const [, setGoalClock] = useState(0);
  const [queuedChatMessages, setQueuedChatMessages] = useState<QueuedChatMessage[]>(storedChatQueue);
  const [steeringMessageId, setSteeringMessageId] = useState<string | null>(null);
  const [queueDispatchErrorId, setQueueDispatchErrorId] = useState<string | null>(null);
  const [taskBusy, setTaskBusy] = useState(false);
  const [conversationApprovalBusy, setConversationApprovalBusy] = useState(false);
  const [taskError, setTaskError] = useState<string | null>(null);
  const [codeReviewScope, setCodeReviewScope] = useState<CodeReviewScope>("branch");
  const [codeReviewBase, setCodeReviewBase] = useState("main");
  const [codeReviewInstructions, setCodeReviewInstructions] = useState("");
  const [codeReviewError, setCodeReviewError] = useState<string | null>(null);
  const [codeReviewTab, setCodeReviewTab] = useState<"all" | "reviewing" | "authored">("all");
  const [codeReviewSearch, setCodeReviewSearch] = useState("");
  const [codeReviewStatusFilter, setCodeReviewStatusFilter] = useState<"all" | "open" | "finished">("all");
  const [codeReviewFilterOpen, setCodeReviewFilterOpen] = useState(false);
  const [codeReviewSetupOpen, setCodeReviewSetupOpen] = useState(false);
  const [cloudApplySetupOpen, setCloudApplySetupOpen] = useState(false);
  const [cloudApplyTaskId, setCloudApplyTaskId] = useState("");
  const [cloudApplyCandidate, setCloudApplyCandidate] = useState<CloudDiffApply | null>(null);
  const [cloudApplyCandidates, setCloudApplyCandidates] = useState<CloudDiffApply[]>([]);
  const [cloudApplyOperation, setCloudApplyOperation] = useState<"loading" | "preparing" | "applying" | "discarding" | null>(null);
  const cloudApplyBusy = cloudApplyOperation !== null;
  const [cloudApplyError, setCloudApplyError] = useState<string | null>(null);
  const [selectedCodeReviewTaskId, setSelectedCodeReviewTaskId] = useState<string | null>(null);
  const [sidebarSearchOpen, setSidebarSearchOpen] = useState(false);
  const [taskLaunch, setTaskLaunch] = useState<Record<string, string>>({});
  const [taskHistory, setTaskHistory] = useState<Record<string, TaskEventsResponse | undefined>>({});
  const [taskAudit, setTaskAudit] = useState<Record<string, ToolAuditResponse | undefined>>({});
  const [taskRunIds, setTaskRunIds] = useState<Record<string, string | undefined>>({});
  const [taskOrganizationIds, setTaskOrganizationIds] = useState<Record<string, string>>({});
  const [newChatRunPreferences, setNewChatRunPreferences] = useState<ChatRunPreferences>(() => storedChatPreferences("mcg.chat-run-preferences.default"));
  const [taskRunPreferences, setTaskRunPreferences] = useState<Record<string, ChatRunPreferences>>(() => {
    try {
      const stored = window.localStorage.getItem("mcg.chat-run-preferences.tasks");
      if (!stored) return {};
      const parsed = JSON.parse(stored) as Record<string, Partial<ChatRunPreferences>>;
      return Object.fromEntries(Object.entries(parsed).map(([taskId, preferences]) => [taskId, {
        ...DEFAULT_CHAT_RUN_PREFERENCES,
        ...preferences,
        team_preset: preferences.team_preset ?? "automatic"
      }])) as Record<string, ChatRunPreferences>;
    } catch {
      return {};
    }
  });
  const [taskReplays, setTaskReplays] = useState<Record<string, RunReplayReport | undefined>>({});
  const [replayBusyTask, setReplayBusyTask] = useState<string | null>(null);
  const [selectedProject, setSelectedProject] = useState<ProjectModel | null>(null);
  const [projects, setProjects] = useState<ProjectModel[]>([]);
  const [archivedProjects, setArchivedProjects] = useState<ProjectModel[]>([]);
  const [archivedProjectsOpen, setArchivedProjectsOpen] = useState(() => window.localStorage.getItem("mcg.sidebar-archived-projects-open") === "true");
  const [projectMutationBusy, setProjectMutationBusy] = useState<string | null>(null);
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
  const [agentTab, setAgentTab] = useState<"overview" | "teams" | "activity" | "governor">("overview");
  const [governorOverview, setGovernorOverview] = useState<GovernorOverview | null>(null);
  const [governorPolicy, setGovernorPolicy] = useState<GovernorControlPolicy | null>(null);
  const [governorProposals, setGovernorProposals] = useState<GovernorProposal[]>([]);
  const [governorConversationId, setGovernorConversationId] = useState("");
  const [organizationAssignments, setOrganizationAssignments] = useState<OrganizationAssignments | null>(null);
  const [agentRuns, setAgentRuns] = useState<AgentRunView[]>([]);
  const [globalAgentSessions, setGlobalAgentSessions] = useState<GlobalAgentSessionSummary[]>([]);
  const [globalAgentSearch, setGlobalAgentSearch] = useState("");
  const [selectedAgentRunId, setSelectedAgentRunId] = useState<string | null>(null);
  const [organizationDraft, setOrganizationDraft] = useState<OrganizationGenome | null>(null);
  const [organizationPreview, setOrganizationPreview] = useState<OrganizationRevisionPreview | null>(null);
  const [organizationChangeSummary, setOrganizationChangeSummary] = useState("");
  const [agentBusy, setAgentBusy] = useState(false);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [managedWorktrees, setManagedWorktrees] = useState<ManagedWorktree[]>([]);
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
  const [focusedApprovalSource, setFocusedApprovalSource] = useState<ApprovalRequest | null>(null);
  const [approvalSourceBusy, setApprovalSourceBusy] = useState<string | null>(null);
  const [approvalSourceError, setApprovalSourceError] = useState<string | null>(null);
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
  const [codexSites, setCodexSites] = useState<CodexSiteInventoryItem[]>([]);
  const [codexSitesError, setCodexSitesError] = useState<string | null>(null);
  const [siteContextMenu, setSiteContextMenu] = useState<SiteContextMenu>(null);
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
    const root = scrollChainRootRef.current;
    if (!root) return;
    root.addEventListener("wheel", handleScrollChainWheel, { capture: true, passive: false });
    return () => root.removeEventListener("wheel", handleScrollChainWheel, true);
  }, []);

  useEffect(() => {
    const previousScrollRestoration = window.history.scrollRestoration;
    window.history.scrollRestoration = "manual";
    window.requestAnimationFrame(() => window.scrollTo({ top: 0, left: 0, behavior: "instant" }));
    return () => {
      window.history.scrollRestoration = previousScrollRestoration;
    };
  }, []);

  useEffect(() => {
    window.localStorage.setItem("mcg.pinned-chat-ids", JSON.stringify(pinnedTaskIds));
  }, [pinnedTaskIds]);

  useEffect(() => {
    window.localStorage.setItem("mcg.unread-chat-ids", JSON.stringify(unreadChatIds));
  }, [unreadChatIds]);

  useEffect(() => {
    if (!advanced && settingsTab === "hooks") setSettingsTab("general");
  }, [advanced, settingsTab]);

  useEffect(() => {
    const inside = (target: Element | null, name: string) => Boolean(target?.closest(`[data-transient-popover-root="${name}"]`));
    const closeStateMenusOutside = (target: Element | null) => {
      if (!inside(target, "desktop-menu")) setDesktopMenu(null);
      if (!inside(target, "owner-menu")) setOwnerMenuOpen(false);
      if (!inside(target, "workspace-menu")) setWorkspacePopover(null);
      if (!inside(target, "review-filter")) setCodeReviewFilterOpen(false);
      if (!inside(target, "context-menu")) {
        setChatContextMenu(null);
        setSiteContextMenu(null);
      }
    };
    const closeAll = () => {
      closeTransientDetails();
      setDesktopMenu(null);
      setOwnerMenuOpen(false);
      setWorkspacePopover(null);
      setCodeReviewFilterOpen(false);
      setChatContextMenu(null);
      setSiteContextMenu(null);
      setSlashPaletteOpen(false);
      setMentionPaletteOpen(false);
      setAppPaletteOpen(false);
    };
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      closeTransientDetails(target);
      closeStateMenusOutside(target);
      if (!target?.closest(".slash-command-palette, #chat-message, #goal")) setSlashPaletteOpen(false);
      if (!target?.closest(".mention-palette, #chat-message, #goal")) setMentionPaletteOpen(false);
      if (!target?.closest(".app-reference-palette, #chat-message, #goal")) setAppPaletteOpen(false);
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const activeElement = document.activeElement instanceof Element ? document.activeElement : null;
      const activeDetails = activeElement?.closest<HTMLDetailsElement>(`${TRANSIENT_DETAILS_SELECTOR}[open]`)
        ?? document.querySelector<HTMLDetailsElement>(`${TRANSIENT_DETAILS_SELECTOR}[open]`);
      const expandedTrigger = activeElement?.closest<HTMLElement>("[data-transient-popover-root]")?.querySelector<HTMLElement>('[aria-expanded="true"]')
        ?? document.querySelector<HTMLElement>('[data-transient-popover-root] [aria-expanded="true"]');
      const hadOpenMenu = Boolean(activeDetails || expandedTrigger);
      closeAll();
      if (!hadOpenMenu) return;
      event.preventDefault();
      window.requestAnimationFrame(() => {
        (activeDetails?.querySelector<HTMLElement>(":scope > summary") ?? expandedTrigger)?.focus();
      });
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "hidden") closeAll();
    };

    window.addEventListener("pointerdown", handlePointerDown, true);
    window.addEventListener("blur", closeAll);
    window.addEventListener("keydown", handleEscape);
    window.addEventListener("pagehide", closeAll);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown, true);
      window.removeEventListener("blur", closeAll);
      window.removeEventListener("keydown", handleEscape);
      window.removeEventListener("pagehide", closeAll);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  useEffect(() => {
    window.localStorage.setItem("mcg.global-folders", JSON.stringify(globalFolders));
  }, [globalFolders]);

  useEffect(() => {
    window.localStorage.setItem("mcg.chat-run-preferences.default", JSON.stringify(newChatRunPreferences));
  }, [newChatRunPreferences]);

  useEffect(() => {
    let active = true;
    const projectId = selectedProject?.project_id;
    if (!projectId) {
      setCodexPermissionProfiles([]);
      return () => { active = false; };
    }
    void getCodexPermissionProfiles(projectId)
      .then((profiles) => { if (active) setCodexPermissionProfiles(profiles); })
      .catch(() => { if (active) setCodexPermissionProfiles([]); });
    return () => { active = false; };
  }, [selectedProject?.project_id]);

  useEffect(() => {
    if (windowsSandboxProjectId || projects.length === 0) return;
    setWindowsSandboxProjectId(selectedProject?.project_id ?? projects[0].project_id);
  }, [projects, selectedProject?.project_id, windowsSandboxProjectId]);

  useEffect(() => {
    let active = true;
    if (!windowsSandboxProjectId) {
      setWindowsSandboxPermissionProfiles([]);
      return () => { active = false; };
    }
    void getCodexPermissionProfiles(windowsSandboxProjectId)
      .then((profiles) => {
        if (!active) return;
        const allowed = profiles.filter((profile) => profile.allowed);
        setWindowsSandboxPermissionProfiles(allowed);
        setWindowsSandboxPermissionProfile(allowed.find((profile) => profile.id === ":workspace")?.id ?? allowed[0]?.id ?? ":workspace");
      })
      .catch((error) => {
        if (!active) return;
        setWindowsSandboxPermissionProfiles([]);
        setWindowsSandboxError(`Could not load sandbox permission profiles. ${errorMessage(error)}`);
      });
    return () => { active = false; };
  }, [windowsSandboxProjectId]);

  useEffect(() => {
    window.localStorage.setItem("mcg.chat-run-preferences.tasks", JSON.stringify(taskRunPreferences));
  }, [taskRunPreferences]);

  useEffect(() => {
    window.localStorage.setItem(CHAT_QUEUE_STORAGE_KEY, JSON.stringify(queuedChatMessages));
  }, [queuedChatMessages]);

  useEffect(() => {
    window.localStorage.setItem("mcg.sidebar-width", String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    window.localStorage.setItem("mcg.settings-nav-width", String(settingsNavWidth));
  }, [settingsNavWidth]);

  useEffect(() => {
    if (settingsTab === "git") void refreshGitHub();
  }, [settingsTab]);

  useEffect(() => {
    window.localStorage.setItem("mcg.workspace-side-panel", String(workspaceSidePanelOpen));
    window.localStorage.setItem("mcg.workspace-side-width", String(workspaceSideWidth));
  }, [workspaceSidePanelOpen, workspaceSideWidth]);

  useEffect(() => {
    window.localStorage.setItem("mcg.workspace-bottom-panel", String(workspaceBottomPanelOpen));
    window.localStorage.setItem("mcg.workspace-bottom-height", String(workspaceBottomHeight));
  }, [workspaceBottomPanelOpen, workspaceBottomHeight]);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    void listOpenInApplications()
      .then(setOpenInApplications)
      .catch((error) => setWorkspaceActionStatus(errorMessage(error)));
  }, []);

  useEffect(() => {
    const task = selectedChatId ? tasks.find((candidate) => candidate.id === selectedChatId) : undefined;
    const project = projects.find((candidate) => candidate.project_id === task?.project_id) ?? selectedProject;
    if (project?.root && !terminalWorkingDirectory) setTerminalWorkingDirectory(project.root);
  }, [projects, selectedChatId, selectedProject, tasks, terminalWorkingDirectory]);

  useEffect(() => {
    if (workspaceBottomPanelOpen) window.requestAnimationFrame(() => terminalInputRef.current?.focus());
  }, [workspaceBottomPanelOpen]);

  useEffect(() => {
    const output = terminalOutputRef.current;
    if (!workspaceBottomPanelOpen || !output) return;
    if (typeof output.scrollTo === "function") output.scrollTo({ top: output.scrollHeight });
    else output.scrollTop = output.scrollHeight;
  }, [terminalEntries, workspaceBottomPanelOpen]);

  useEffect(() => {
    window.localStorage.setItem("mcg.theme", themePreference);
    document.documentElement.dataset.theme = themePreference;
  }, [themePreference]);

  useEffect(() => {
    window.localStorage.setItem("mcg.reduce-motion", String(reduceMotion));
    document.documentElement.dataset.reduceMotion = String(reduceMotion);
  }, [reduceMotion]);

  useEffect(() => {
    const restoreSurface = () => {
      const [surface, encodedChatId] = window.location.hash.slice(1).split("/");
      if (surface) setActiveSurface(surface);
      if (surface === "threads" && encodedChatId) setSelectedChatId(decodeURIComponent(encodedChatId));
      window.requestAnimationFrame(() => window.scrollTo({ top: 0, left: 0, behavior: "instant" }));
    };
    restoreSurface();
    window.addEventListener("popstate", restoreSurface);
    return () => window.removeEventListener("popstate", restoreSurface);
  }, []);

  useEffect(() => {
    if (activeSurface !== "sites") return;
    let active = true;
    setSiteError(null);
    setCodexSitesError(null);
    void listSites()
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
    void listCodexSites()
      .then((response) => {
        if (active) setCodexSites(response.sites);
      })
      .catch((error) => {
        if (active) setCodexSitesError(`The ChatGPT Sites library could not refresh: ${errorMessage(error)}`);
      });
    return () => {
      active = false;
    };
  }, [activeSurface]);

  useEffect(() => {
    if (!selectedChatId || conversationViews[selectedChatId] || !conversations.some((conversation) => conversation.id === selectedChatId)) return;
    void getConversation(selectedChatId)
      .then((view) => setConversationViews((current) => ({ ...current, [selectedChatId]: view })))
      .catch((error) => setTaskError(errorMessage(error)));
  }, [conversationViews, conversations, selectedChatId]);

  useEffect(() => {
    const handleAppShortcut = (event: KeyboardEvent) => {
      const command = event.ctrlKey || event.metaKey;
      const key = event.key.toLocaleLowerCase();
      if (event.key === "F11") {
        event.preventDefault();
        void toggleFullScreen();
        return;
      }
      if (command && event.altKey && key === "b") {
        event.preventDefault();
        setWorkspaceSidePanelOpen((current) => !current);
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
      } else if (key === "j" || event.key === "`") {
        event.preventDefault();
        setWorkspaceBottomPanelOpen((current) => !current);
      } else if (key === "f") {
        event.preventDefault();
        setSidebarOpen(true);
        setSidebarSearchOpen(true);
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
    const chats = Array.from(new Set(tasks.map(chatIdForTask)));
    const current = selectedChatId ? chats.indexOf(selectedChatId) : -1;
    const next = current < 0
      ? (offset > 0 ? 0 : chats.length - 1)
      : (current + offset + chats.length) % chats.length;
    openChat(chats[next]);
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

  async function openNewWindow(url = window.location.href) {
    setDesktopMenu(null);
    if (!("__TAURI_INTERNALS__" in window)) {
      window.open(url, "_blank", "noopener,noreferrer");
      return;
    }
    new WebviewWindow(`mcg-${Date.now()}`, {
      url,
      title: "MetaCog",
      decorations: false,
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

  async function minimizeCurrentWindow() {
    if ("__TAURI_INTERNALS__" in window) await getCurrentWindow().minimize();
  }

  async function toggleCurrentWindowMaximized() {
    if ("__TAURI_INTERNALS__" in window) await getCurrentWindow().toggleMaximize();
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
    setDraftThreadId(crypto.randomUUID());
    setTaskGoal("");
    setTaskError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function startSiteChat(project: ProjectModel | null = selectedProject) {
    setSelectedProject(project);
    setSelectedChatId(null);
    setTaskGoal("Build a website. Start by asking me about its audience, purpose, required behavior, and information, then build and test a reviewable version before publishing it.");
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
    setTaskError(null);
    setCodeReviewError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function openLocalReviewSetup() {
    setCodeReviewSetupOpen(true);
    setCloudApplySetupOpen(false);
    setSelectedCodeReviewTaskId(null);
    setCodeReviewError(null);
  }

  async function loadCloudApplyCandidates(projectId: string) {
    setCloudApplyOperation("loading");
    setCloudApplyError(null);
    try {
      const response = await listCloudDiffApplies(projectId);
      const ready = response.applies.filter((candidate) => candidate.state === "ready");
      setCloudApplyCandidates(ready);
      setCloudApplyCandidate(ready[0] ?? null);
      if (ready[0]) setCloudApplyTaskId(ready[0].task_id);
    } catch (error) {
      setCloudApplyCandidates([]);
      setCloudApplyCandidate(null);
      setCloudApplyError(errorMessage(error));
    } finally {
      setCloudApplyOperation((current) => current === "loading" ? null : current);
    }
  }

  function openCloudApplySetup() {
    setCloudApplySetupOpen(true);
    setCodeReviewSetupOpen(false);
    setSelectedCodeReviewTaskId(null);
    setCloudApplyCandidate(null);
    setCloudApplyCandidates([]);
    setCloudApplyError(null);
    if (selectedProject?.repository.is_git) void loadCloudApplyCandidates(selectedProject.project_id);
  }

  async function prepareCodexCloudApply() {
    if (!selectedProject?.repository.is_git) {
      setCloudApplyError("Choose a project backed by a Git repository before checking cloud changes.");
      return;
    }
    const taskId = cloudApplyTaskId.trim();
    if (!taskId) {
      setCloudApplyError("Enter the Codex cloud task ID.");
      return;
    }
    setCloudApplyOperation("preparing");
    setCloudApplyError(null);
    try {
      const candidate = await prepareCloudDiffApply(selectedProject.project_id, taskId);
      setCloudApplyCandidate(candidate);
      setCloudApplyCandidates((current) => [candidate, ...current.filter((item) => item.id !== candidate.id)]);
    } catch (error) {
      setCloudApplyCandidate(null);
      setCloudApplyError(errorMessage(error));
    } finally {
      setCloudApplyOperation((current) => current === "preparing" ? null : current);
    }
  }

  async function confirmCodexCloudApply() {
    if (!cloudApplyCandidate) return;
    setCloudApplyOperation("applying");
    setCloudApplyError(null);
    try {
      const applied = await applyCloudDiff(cloudApplyCandidate.id, cloudApplyCandidate.revision);
      setCloudApplyCandidate(applied);
      setCloudApplyCandidates((current) => current.filter((item) => item.id !== applied.id));
      const projectList = await listProjects();
      setProjects(projectList.projects);
      setArchivedProjects(projectList.archived_projects ?? []);
    } catch (error) {
      setCloudApplyError(errorMessage(error));
    } finally {
      setCloudApplyOperation((current) => current === "applying" ? null : current);
    }
  }

  async function discardCodexCloudApply() {
    if (!cloudApplyCandidate) return;
    setCloudApplyOperation("discarding");
    setCloudApplyError(null);
    try {
      await discardCloudDiffApply(cloudApplyCandidate.id, cloudApplyCandidate.revision);
      const remaining = cloudApplyCandidates.filter((item) => item.id !== cloudApplyCandidate.id);
      setCloudApplyOperation(null);
      setCloudApplyCandidates(remaining);
      setCloudApplyCandidate(remaining[0] ?? null);
      setCloudApplyTaskId(remaining[0]?.task_id ?? "");
    } catch (error) {
      setCloudApplyError(errorMessage(error));
      setCloudApplyOperation(null);
    }
  }

  function continueSiteChat(site: Site) {
    const project = projects.find((candidate) => candidate.project_id === site.project_id) ?? null;
    setSelectedProject(project);
    setSelectedChatId(null);
    setTaskGoal(`Refine the website ${site.display_name} at ${site.public_url}. Inspect the existing project, show me a reviewable version, and do not publish changes until they are approved.`);
    setTaskError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function continueSiteLibraryChat(entry: SiteLibraryEntry) {
    if (entry.localSite) {
      continueSiteChat(entry.localSite);
      return;
    }
    setSelectedProject(null);
    setSelectedChatId(null);
    setTaskGoal(`Refine the existing ChatGPT Site ${entry.displayName} at ${entry.publicUrl}. Inspect its current Sites project and source, show me a reviewable version, and do not publish changes until they are approved.`);
    setTaskError(null);
    navigateSurface("tasks");
    window.requestAnimationFrame(() => goalInputRef.current?.focus());
  }

  function openSiteContextMenu(event: MouseEvent<HTMLElement>, key: string) {
    event.preventDefault();
    event.stopPropagation();
    setSiteContextMenu({
      key,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 244)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - 132))
    });
  }

  async function openSiteInApp(entry: SiteLibraryEntry) {
    setSiteContextMenu(null);
    setCodexSitesError(null);
    setPlatformError(null);
    setSideBrowserUrl(entry.publicUrl);
    const existing = siteBrowserTabs.find((tab) => comparableSiteUrl(tab.url ?? "") === comparableSiteUrl(entry.publicUrl));
    if (existing) {
      setActiveSiteBrowserTabId(existing.id);
    } else {
      const tab = { id: crypto.randomUUID(), title: entry.displayName, url: entry.publicUrl };
      setSiteBrowserTabs((current) => [...current, tab]);
      setActiveSiteBrowserTabId(tab.id);
    }
    setWorkspaceSidePanelTab("browser");
    setWorkspaceSidePanelOpen(true);
  }

  function openConversationPreview(preview: ConversationPreview) {
    if (!preview.safe_url) return;
    setSideBrowserUrl(preview.safe_url);
    const stableId = preview.browser_tab_id || `conversation-preview-${preview.id}`;
    const existing = siteBrowserTabs.find((tab) => tab.id === stableId);
    if (existing) {
      setSiteBrowserTabs((current) => current.map((tab) => tab.id === stableId ? { ...tab, url: preview.safe_url! } : tab));
    } else {
      setSiteBrowserTabs((current) => [...current, {
        id: stableId,
        title: preview.strategy === "codex_sites" ? "ChatGPT Site" : "Local preview",
        url: preview.safe_url!
      }]);
    }
    setActiveSiteBrowserTabId(stableId);
    setWorkspaceSidePanelTab("browser");
    setWorkspaceSidePanelOpen(true);
  }

  async function openSiteExternally(entry: SiteLibraryEntry) {
    setSiteContextMenu(null);
    setSiteError(null);
    try {
      await openSiteExternalUrl(entry.publicUrl);
    } catch (error) {
      setSiteError(errorMessage(error));
    }
  }

  function openChat(chatId: string) {
    setUnreadChatIds((current) => current.filter((candidate) => candidate !== chatId));
    setChatContextMenu(null);
    setSelectedChatId(chatId);
    const conversation = conversations.find((candidate) => candidate.id === chatId);
    const latest = tasks.find((task) => chatIdForTask(task) === chatId);
    const projectId = conversation?.project_id ?? latest?.project_id;
    if (projectId) {
      setSelectedProject(projects.find((project) => project.project_id === projectId) ?? null);
    }
    if (conversation) {
      const model = codexModels.find((candidate) => candidate.id === conversation.execution_preferences.model_id);
      const tier = conversation.execution_preferences.service_tier;
      setNewChatRunPreferences((current) => ({
        ...current,
        provider_id: conversation.execution_preferences.model_provider ?? (conversation.execution_preferences.model_id ? "codex" : undefined),
        model_provider: conversation.execution_preferences.model_provider,
        model_id: conversation.execution_preferences.model_id,
        model_label: model?.displayName,
        reasoning_effort: conversation.execution_preferences.reasoning_effort ?? current.reasoning_effort,
        service_tier: tier,
        speed: tier && /fast|priority/i.test(tier) ? "fast" : "standard",
        approval_mode: conversation.execution_preferences.approval_mode ?? "ask",
        approval_policy: conversation.execution_preferences.approval_policy,
        sandbox_mode: conversation.execution_preferences.sandbox_mode,
        permission_profile: conversation.execution_preferences.permission_profile,
        additional_writable_roots: conversation.execution_preferences.additional_writable_roots ?? [],
        codex_feature_overrides: conversation.execution_preferences.codex_feature_overrides ?? {},
        codex_config_overrides: conversation.execution_preferences.codex_config_overrides ?? [],
        codex_profile: conversation.execution_preferences.codex_profile,
        strict_config: conversation.execution_preferences.strict_config ?? false,
        output_schema: conversation.execution_preferences.output_schema,
        team_preset: conversation.execution_preferences.team_preset ?? "automatic",
        personality: conversation.execution_preferences.personality
      }));
    }
    // A completed schema migration opens every chat as a conversation. If an
    // older service has not materialized that record yet, keep its immutable
    // task usable from the durable-work surface instead of presenting an empty
    // conversation.
    navigateSurface(conversation ? "threads" : "tasks");
    if (conversation) window.history.replaceState(null, "", `#threads/${encodeURIComponent(chatId)}`);
    if (conversation && !conversationViews[chatId]) {
      void getConversation(chatId)
        .then((view) => setConversationViews((current) => ({ ...current, [chatId]: view })))
        .catch((error) => setTaskError(errorMessage(error)));
    }
  }

  function currentWorkspaceTask() {
    return selectedChatId ? tasks.find((task) => chatIdForTask(task) === selectedChatId) : undefined;
  }

  function currentWorkspaceProject() {
    const conversation = selectedChatId ? conversations.find((candidate) => candidate.id === selectedChatId) : undefined;
    const task = currentWorkspaceTask();
    return projects.find((project) => project.project_id === (conversation?.project_id ?? task?.project_id)) ?? selectedProject ?? undefined;
  }

  async function copyWorkspaceText(value: string, success: string) {
    try {
      if (navigator.clipboard?.writeText) await navigator.clipboard.writeText(value);
      else {
        const textarea = document.createElement("textarea");
        textarea.value = value;
        textarea.style.position = "fixed";
        textarea.style.opacity = "0";
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand("copy");
        textarea.remove();
      }
      setWorkspaceActionStatus(success);
      setWorkspacePopover(null);
    } catch (error) {
      setWorkspaceActionStatus(errorMessage(error));
    }
  }

  async function shareWorkspaceSummary() {
    const task = currentWorkspaceTask();
    const project = currentWorkspaceProject();
    const lines = [
      task?.goal ?? activeSurfaceTitle,
      task ? `Status: ${task.state.replaceAll("_", " ")}` : undefined,
      project ? `Project: ${project.root}` : undefined,
      task ? `Task: ${task.id}` : undefined
    ].filter(Boolean);
    await copyWorkspaceText(lines.join("\n"), "Share summary copied");
  }

  async function openCurrentWorkspace(application: string) {
    const project = currentWorkspaceProject();
    if (!project) {
      setWorkspaceActionStatus("Choose a project before opening it in another application.");
      return;
    }
    try {
      await openWorkspaceIn(application, project.root);
      setWorkspaceActionStatus(`Opened ${project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? "project"}`);
      setWorkspacePopover(null);
    } catch (error) {
      setWorkspaceActionStatus(errorMessage(error));
    }
  }

  async function startSideBrowser(requestedUrl = sideBrowserUrl) {
    let parsed: URL;
    try {
      parsed = new URL(requestedUrl.trim());
      if (!(["http:", "https:"] as string[]).includes(parsed.protocol)) throw new Error("Use an HTTP or HTTPS URL.");
    } catch (error) {
      setPlatformError(errorMessage(error));
      return;
    }
    const sessionId = freshPlatformSessionId();
    siteBrowserTabs.forEach((tab) => void closeSiteInAppBrowser(tab.id).catch(() => undefined));
    setSiteBrowserTabs([]);
    setActiveSiteBrowserTabId(null);
    setPlatformBusy(true);
    setPlatformError(null);
    try {
      const session = await requestPlatformSession({
        session_id: sessionId,
        kind: "browser",
        project_id: currentWorkspaceProject()?.project_id,
        task_id: currentWorkspaceTask()?.id,
        display_name: `Browser · ${parsed.hostname}`,
        allowed_origins: [parsed.origin],
        lifetime_seconds: 3600
      });
      replacePlatformSession(session);
      const attached = await attachBrowserSession(sessionId, "headless");
      replacePlatformSession(attached.session);
      const browser = await navigateBrowserSession(sessionId, parsed.href);
      setPlatformBrowsers((current) => ({ ...current, [sessionId]: browser }));
      setPlatformBrowserUrls((current) => ({ ...current, [sessionId]: parsed.href }));
      setSelectedBrowserSessionId(sessionId);
      setSideBrowserUrl(parsed.href);
      setWorkspaceSidePanelTab("browser");
      setWorkspaceSidePanelOpen(true);
    } catch (error) {
      setPlatformError(errorMessage(error));
    } finally {
      setPlatformBusy(false);
    }
  }

  function navigateSiteBrowserPanel() {
    try {
      const parsed = new URL(sideBrowserUrl.trim());
      if (parsed.protocol !== "https:" || parsed.username || parsed.password || !parsed.hostname) {
        throw new Error("Use a credential-free HTTPS URL.");
      }
      setPlatformError(null);
      if (activeSiteBrowserTabId) {
        setSiteBrowserTabs((current) => current.map((tab) => tab.id === activeSiteBrowserTabId
          ? { ...tab, title: parsed.hostname, url: parsed.href }
          : tab));
      } else {
        const tab = { id: crypto.randomUUID(), title: parsed.hostname, url: parsed.href };
        setSiteBrowserTabs((current) => [...current, tab]);
        setActiveSiteBrowserTabId(tab.id);
      }
    } catch (error) {
      setPlatformError(errorMessage(error));
    }
  }

  async function submitTerminalCommand() {
    const command = terminalCommand.trim();
    if (!command || terminalBusy) return;
    if (command.toLocaleLowerCase() === "clear") {
      setTerminalEntries([]);
      setTerminalCommand("");
      return;
    }
    const workingDirectory = terminalWorkingDirectory || currentWorkspaceProject()?.root;
    if (!workingDirectory) {
      setWorkspaceActionStatus("Choose a project before opening its terminal.");
      return;
    }
    const id = crypto.randomUUID();
    setTerminalEntries((current) => [...current, { id, command, workingDirectory }]);
    setTerminalCommand("");
    setTerminalBusy(true);
    try {
      const result = await runTerminalCommand(workingDirectory, command);
      setTerminalWorkingDirectory(result.working_directory);
      setTerminalEntries((current) => current.map((entry) => entry.id === id ? { ...entry, result } : entry));
    } catch (error) {
      setTerminalEntries((current) => current.map((entry) => entry.id === id ? { ...entry, error: errorMessage(error) } : entry));
    } finally {
      setTerminalBusy(false);
      window.requestAnimationFrame(() => terminalInputRef.current?.focus());
    }
  }

  function beginSidePanelResize(event: ReactPointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = workspaceSideWidth;
    const rail = event.currentTarget;
    rail.setPointerCapture(event.pointerId);
    const maximum = window.innerWidth <= 820
      ? Math.max(320, window.innerWidth - 48)
      : Math.max(320, window.innerWidth - (sidebarOpen ? sidebarWidth : 0) - 320);
    const move = (moveEvent: globalThis.PointerEvent) => setWorkspaceSideWidth(Math.min(maximum, Math.max(280, startWidth + startX - moveEvent.clientX)));
    const stop = () => {
      rail.removeEventListener("pointermove", move);
      rail.removeEventListener("pointerup", stop);
      rail.removeEventListener("pointercancel", stop);
    };
    rail.addEventListener("pointermove", move);
    rail.addEventListener("pointerup", stop);
    rail.addEventListener("pointercancel", stop);
  }

  function beginSidebarResize(event: ReactPointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = sidebarWidth;
    const rail = event.currentTarget;
    rail.setPointerCapture(event.pointerId);
    const move = (moveEvent: globalThis.PointerEvent) => setSidebarWidth(Math.min(440, Math.max(190, startWidth + moveEvent.clientX - startX)));
    const stop = () => {
      rail.removeEventListener("pointermove", move);
      rail.removeEventListener("pointerup", stop);
      rail.removeEventListener("pointercancel", stop);
    };
    rail.addEventListener("pointermove", move);
    rail.addEventListener("pointerup", stop);
    rail.addEventListener("pointercancel", stop);
  }

  function beginSettingsNavResize(event: ReactPointerEvent<HTMLDivElement>) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = settingsNavWidth;
    const rail = event.currentTarget;
    rail.setPointerCapture(event.pointerId);
    const move = (moveEvent: globalThis.PointerEvent) => setSettingsNavWidth(Math.min(420, Math.max(190, startWidth + moveEvent.clientX - startX)));
    const stop = () => {
      rail.removeEventListener("pointermove", move);
      rail.removeEventListener("pointerup", stop);
      rail.removeEventListener("pointercancel", stop);
    };
    rail.addEventListener("pointermove", move);
    rail.addEventListener("pointerup", stop);
    rail.addEventListener("pointercancel", stop);
  }

  function createSiteBrowserTab() {
    const tab = { id: crypto.randomUUID(), title: "New tab", url: null };
    setSiteBrowserTabs((current) => [...current, tab]);
    setActiveSiteBrowserTabId(tab.id);
    setSideBrowserUrl("");
  }

  function selectSiteBrowserTab(tab: SiteBrowserTab) {
    setActiveSiteBrowserTabId(tab.id);
    setSideBrowserUrl(tab.url ?? "");
  }

  function closeSiteBrowserTab(tabId: string) {
    void closeSiteInAppBrowser(tabId).catch((error) => setPlatformError(errorMessage(error)));
    setSiteBrowserTabs((current) => {
      const index = current.findIndex((tab) => tab.id === tabId);
      const next = current.filter((tab) => tab.id !== tabId);
      if (activeSiteBrowserTabId === tabId) {
        const replacement = next[Math.min(Math.max(index, 0), next.length - 1)] ?? null;
        setActiveSiteBrowserTabId(replacement?.id ?? null);
        setSideBrowserUrl(replacement?.url ?? "");
      }
      return next;
    });
  }

  function beginBottomPanelResize(event: MouseEvent<HTMLDivElement>) {
    event.preventDefault();
    const startY = event.clientY;
    const startHeight = workspaceBottomHeight;
    const move = (moveEvent: globalThis.MouseEvent) => setWorkspaceBottomHeight(Math.min(520, Math.max(170, startHeight + startY - moveEvent.clientY)));
    const stop = () => { window.removeEventListener("mousemove", move); window.removeEventListener("mouseup", stop); };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", stop);
  }

  function togglePinnedTask(chatId: string, legacyTaskIds: string[] = []) {
    setPinnedTaskIds((current) => {
      const pinned = current.includes(chatId) || legacyTaskIds.some((id) => current.includes(id));
      const withoutChat = current.filter((candidate) => candidate !== chatId && !legacyTaskIds.includes(candidate));
      return pinned ? withoutChat : [chatId, ...withoutChat];
    });
    setChatContextMenu(null);
  }

  function openChatContextMenu(event: MouseEvent<HTMLElement>, chatId: string, source: ChatMenuSource) {
    event.preventDefault();
    event.stopPropagation();
    const width = source === "pinned" ? 270 : 250;
    const height = source === "pinned" ? 330 : 275;
    setChatContextMenu({
      chatId,
      source,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - width - 8)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - height - 8))
    });
  }

  function movePinnedChat(chatId: string, offset: -1 | 1) {
    setPinnedTaskIds((current) => {
      const index = current.indexOf(chatId);
      if (index < 0) return current;
      const next = index + offset;
      if (next < 0 || next >= current.length) return current;
      const reordered = [...current];
      [reordered[index], reordered[next]] = [reordered[next], reordered[index]];
      return reordered;
    });
    setChatContextMenu(null);
  }

  async function mutateConversation(chatId: string, title: string, projectId?: string) {
    setChatMutationBusy(true);
    setTaskError(null);
    try {
      const updated = await updateConversation(chatId, { title, project_id: projectId });
      setConversations((current) => current.map((conversation) => conversation.id === updated.id ? updated : conversation));
      setConversationViews((current) => {
        const existing = current[updated.id];
        return existing ? { ...current, [updated.id]: { ...existing, conversation: updated } } : current;
      });
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setChatMutationBusy(false);
      setChatContextMenu(null);
    }
  }

  async function renameChat(chatId: string, currentTitle: string, projectId?: string) {
    const next = window.prompt("Rename chat", currentTitle)?.trim();
    if (!next || next === currentTitle) {
      setChatContextMenu(null);
      return;
    }
    await mutateConversation(chatId, next, projectId);
  }

  async function setChatArchived(chatId: string, archived: boolean): Promise<boolean> {
    setChatMutationBusy(true);
    setTaskError(null);
    try {
      const updated = await archiveConversation(chatId, archived);
      setConversations((current) => current.map((conversation) => conversation.id === updated.id ? updated : conversation));
      if (archived) {
        const linkedTaskIds = tasks.filter((task) => chatIdForTask(task) === chatId).map((task) => task.id);
        setPinnedTaskIds((current) => current.filter((candidate) => candidate !== chatId && !linkedTaskIds.includes(candidate)));
      }
      setUnreadChatIds((current) => current.filter((candidate) => candidate !== chatId));
      if (archived && selectedChatId === chatId) {
        setSelectedChatId(null);
        navigateSurface("tasks");
      }
      return true;
    } catch (error) {
      setTaskError(errorMessage(error));
      return false;
    } finally {
      setChatMutationBusy(false);
      setChatContextMenu(null);
    }
  }

  async function restoreArchivedChat(chatId: string, openAfterRestore = false) {
    const restored = await setChatArchived(chatId, false);
    if (restored && openAfterRestore) openChat(chatId);
  }

  async function setProjectArchived(project: ProjectModel, archived: boolean) {
    setProjectMutationBusy(project.project_id);
    setProjectError(null);
    setTaskError(null);
    try {
      const updated = await archiveProject(project.project_id, archived);
      if (archived) {
        setProjects((current) => current.filter((candidate) => candidate.project_id !== project.project_id));
        setArchivedProjects((current) => [updated, ...current.filter((candidate) => candidate.project_id !== project.project_id)]);
        if (selectedProject?.project_id === project.project_id) setSelectedProject(null);
      } else {
        setArchivedProjects((current) => current.filter((candidate) => candidate.project_id !== project.project_id));
        setProjects((current) => [updated, ...current.filter((candidate) => candidate.project_id !== project.project_id)]);
      }
    } catch (error) {
      setProjectError(errorMessage(error));
      setTaskError(errorMessage(error));
    } finally {
      setProjectMutationBusy(null);
    }
  }

  async function deleteProject(project: ProjectModel) {
    const name = project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root;
    if (!window.confirm(`Remove ${name} from MetaCog?\n\nThis only removes the project from MetaCog. Its folder, local Git history, and GitHub repository will not be deleted.`)) return;
    setProjectMutationBusy(project.project_id);
    setProjectError(null);
    setTaskError(null);
    try {
      await removeProject(project.project_id);
      setProjects((current) => current.filter((candidate) => candidate.project_id !== project.project_id));
      setArchivedProjects((current) => current.filter((candidate) => candidate.project_id !== project.project_id));
      if (selectedProject?.project_id === project.project_id) setSelectedProject(null);
    } catch (error) {
      setProjectError(errorMessage(error));
      setTaskError(errorMessage(error));
    } finally {
      setProjectMutationBusy(null);
    }
  }

  async function copyChatText(value: string, status: string) {
    try {
      await navigator.clipboard.writeText(value);
      setWorkspaceActionStatus(status);
    } catch (error) {
      setTaskError(`Copy failed: ${errorMessage(error)}`);
    }
    setChatContextMenu(null);
  }

  async function refreshCanonicalWorkflows() {
    setWorkflowBusy("list");
    setWorkflowError(null);
    try {
      setCanonicalWorkflows(await listProtectedRootWorkflows());
    } catch (error) {
      setWorkflowError(errorMessage(error));
    } finally {
      setWorkflowBusy(null);
    }
  }

  async function inspectCanonicalWorkflow(workflowId: string) {
    setWorkflowBusy(workflowId);
    setWorkflowError(null);
    try {
      const inspection = await getProtectedRootWorkflow(workflowId);
      if (!inspection) throw new Error(`Canonical workflow ${workflowId} was not found.`);
      setWorkflowInspections((current) => ({ ...current, [workflowId]: inspection }));
    } catch (error) {
      setWorkflowError(errorMessage(error));
    } finally {
      setWorkflowBusy(null);
    }
  }

  async function inspectCanonicalWorkflowActivity(workflowId: string) {
    setWorkflowBusy(workflowId);
    setWorkflowError(null);
    try {
      const activity = await getProtectedRootWorkflowActivity(workflowId);
      if (!activity) throw new Error(`Canonical workflow ${workflowId} was not found.`);
      setWorkflowActivities((current) => ({ ...current, [workflowId]: activity }));
      setWorkflowInspections((current) => ({ ...current, [workflowId]: activity.workflow }));
    } catch (error) {
      setWorkflowError(errorMessage(error));
    } finally {
      setWorkflowBusy(null);
    }
  }

  async function requestCanonicalWorkflowCancellation(attempt: CanonicalWorkflowAttempt) {
    if (!window.confirm("Request cancellation? MetaCog will fence late work, but any uncertain external effect must still be reconciled before cancellation can finish.")) return;
    setWorkflowBusy(attempt.workflow_id);
    setWorkflowError(null);
    try {
      const updated = await cancelProtectedRootWorkflow({
        attempt_id: attempt.attempt_id,
        expected_revision: attempt.revision,
        expected_generation: attempt.lease_generation,
        idempotency_key: `desktop-cancel-${crypto.randomUUID()}`
      });
      setCanonicalWorkflows((current) => current.map((candidate) => candidate.workflow_id === updated.workflow_id ? updated : candidate));
      const inspection = await getProtectedRootWorkflow(updated.workflow_id);
      if (inspection) setWorkflowInspections((current) => ({ ...current, [updated.workflow_id]: inspection }));
    } catch (error) {
      setWorkflowError(errorMessage(error));
    } finally {
      setWorkflowBusy(null);
    }
  }

  async function refreshCanonicalActions() {
    setActionBusy("list");
    setActionError(null);
    try {
      setCanonicalActions(await listProtectedRootActions());
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setActionBusy(null);
    }
  }

  async function inspectCanonicalAction(actionId: string) {
    setActionBusy(actionId);
    setActionError(null);
    try {
      const inspection = await getProtectedRootAction(actionId);
      if (!inspection) throw new Error(`Canonical action ${actionId} was not found.`);
      setActionInspections((current) => ({ ...current, [actionId]: inspection }));
      const latestExecution = inspection.executions.at(-1);
      if (latestExecution) {
        setCanonicalActions((current) => current.some((action) => action.action_id === actionId) ? current : [{
          action_id: actionId,
          execution_id: latestExecution.id,
          workflow_id: inspection.envelope.workflow_id,
          attempt_id: inspection.envelope.attempt_id,
          actor: inspection.envelope.actor,
          effect_class: inspection.envelope.effect_class,
          state: latestExecution.state,
          outcome: latestExecution.outcome,
          revision: latestExecution.revision,
          lease_generation: latestExecution.lease_generation,
          updated_at: latestExecution.updated_at
        }, ...current]);
      }
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setActionBusy(null);
    }
  }

  async function compensateCanonicalAction(actionId: string) {
    const reason = compensationReasons[actionId]?.trim() ?? "";
    if (!reason) {
      setActionError("Enter an owner-visible reason before initiating compensation.");
      return;
    }
    if (!window.confirm("Execute the exact compensation already bound to this committed action? Protected Root will reject changed or stale intent.")) return;
    setActionBusy(`compensate:${actionId}`);
    setActionError(null);
    setActionNotice(null);
    const idempotencyKey = compensationIdempotencyKeys[actionId] ?? `desktop-owner-compensation-${crypto.randomUUID()}`;
    setCompensationIdempotencyKeys((current) => ({ ...current, [actionId]: idempotencyKey }));
    try {
      const original = await getProtectedRootAction(actionId);
      if (!original) throw new Error(`Canonical action ${actionId} was not found.`);
      const execution = original.executions.at(-1);
      if (!execution) throw new Error(`Canonical action ${actionId} has no execution.`);
      const workflow = await getProtectedRootWorkflow(original.envelope.workflow_id);
      if (!workflow) throw new Error(`Source workflow ${original.envelope.workflow_id} was not found.`);
      const receipt = await compensateProtectedRootAction({
        action_id: actionId,
        execution_id: execution.id,
        expected_action_revision: execution.revision,
        expected_action_generation: execution.lease_generation,
        expected_workflow_revision: workflow.attempt.revision,
        expected_workflow_generation: workflow.attempt.lease_generation,
        reason,
        confirmed: true,
        idempotency_key: idempotencyKey
      });
      const refreshedOriginal = await getProtectedRootAction(actionId);
      setActionInspections((current) => ({
        ...current,
        ...(refreshedOriginal ? { [actionId]: refreshedOriginal } : {}),
        [receipt.compensation.envelope.id]: receipt.compensation
      }));
      setCompensationReasons((current) => ({ ...current, [actionId]: "" }));
      setCompensationIdempotencyKeys((current) => {
        const updated = { ...current };
        delete updated[actionId];
        return updated;
      });
      setActionNotice(`Compensation ${receipt.compensation.envelope.id} ${receipt.replayed ? "was already initiated; Protected Root did not redispatch it" : "was initiated from the exact bound plan"}.`);
      setCanonicalActions(await listProtectedRootActions());
    } catch (error) {
      setActionError(errorMessage(error));
    } finally {
      setActionBusy(null);
    }
  }

  async function refreshCanonicalInvocations() {
    setInvocationBusy("list");
    setInvocationError(null);
    try {
      setCanonicalInvocations(await listProtectedRootInvocations());
    } catch (error) {
      setInvocationError(errorMessage(error));
    } finally {
      setInvocationBusy(null);
    }
  }

  async function inspectCanonicalInvocation(invocationId: string) {
    setInvocationBusy(invocationId);
    setInvocationError(null);
    try {
      const inspection = await getProtectedRootInvocation(invocationId);
      if (!inspection) throw new Error(`Invocation ${invocationId} was not found.`);
      setInvocationInspections((current) => ({ ...current, [invocationId]: inspection }));
    } catch (error) {
      setInvocationError(errorMessage(error));
    } finally {
      setInvocationBusy(null);
    }
  }

  async function refreshToolAuthorizations() {
    setCapabilityBusy("list");
    setCapabilityError(null);
    try {
      setToolAuthorizations(await listProtectedRootCapabilities());
    } catch (error) {
      setCapabilityError(errorMessage(error));
    } finally {
      setCapabilityBusy(null);
    }
  }

  async function inspectToolAuthorization(actionId: string) {
    setCapabilityBusy(actionId);
    setCapabilityError(null);
    try {
      const inspection = await getProtectedRootCapability(actionId);
      if (!inspection) throw new Error(`Tool authorization for action ${actionId} was not found.`);
      setToolAuthorizationInspections((current) => ({ ...current, [actionId]: inspection }));
    } catch (error) {
      setCapabilityError(errorMessage(error));
    } finally {
      setCapabilityBusy(null);
    }
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
    getProtectedRootStatus()
      .then((status) => {
        if (active) setProtectedRoot(status);
      })
      .catch(() => {
        if (active) setProtectedRoot(null);
      });
    listProtectedRootWorkflows()
      .then((workflows) => {
        if (active) setCanonicalWorkflows(workflows);
      })
      .catch((error) => {
        if (active) setWorkflowError(errorMessage(error));
      });
    listProtectedRootActions()
      .then((actions) => {
        if (active) setCanonicalActions(actions);
      })
      .catch((error) => {
        if (active) setActionError(errorMessage(error));
      });
    listProtectedRootInvocations()
      .then((invocations) => {
        if (active) setCanonicalInvocations(invocations);
      })
      .catch((error) => {
        if (active) setInvocationError(errorMessage(error));
      });
    listProtectedRootCapabilities()
      .then((capabilities) => {
        if (active) setToolAuthorizations(capabilities);
      })
      .catch((error) => {
        if (active) setCapabilityError(errorMessage(error));
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
          setAdvanced(settings.interface_complexity !== "simple");
          setCodexRemoteEndpoint(settings.codex_remote_endpoint ?? "");
          setCodexRemoteAuthTokenEnv(settings.codex_remote_auth_token_env ?? "");
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
          void discoverModelCatalogs(response.providers);
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
    listConversations()
      .then((response) => {
        if (active) setConversations(response.conversations);
      })
      .catch((error) => {
        if (active) setTaskError(errorMessage(error));
      });
    Promise.allSettled([getCodexModels(), getCodexFeatures(), getCodexAccount(), getCodexRateLimits(), getCodexHarnessStatus(), getCodexConfigurationStatus(), getCodexPendingRequests()])
      .then(([models, features, account, rateLimits, harness, configuration, pendingRequests]) => {
        if (!active) return;
        if (models.status === "fulfilled") setCodexModels(models.value);
        if (features.status === "fulfilled") setCodexFeatures(features.value);
        if (account.status === "fulfilled") setCodexAccount(account.value);
        if (rateLimits.status === "fulfilled") setCodexRateLimits(rateLimits.value);
        if (harness.status === "fulfilled") setCodexHarnessStatus(harness.value);
        if (configuration.status === "fulfilled") setCodexConfigurationStatus(configuration.value);
        if (pendingRequests.status === "fulfilled") setCodexPendingRequests(pendingRequests.value);
        if (models.status === "rejected") setModelDiscoveryError(`Codex model discovery unavailable: ${errorMessage(models.reason)}`);
        if (features.status === "rejected") setCodexFeatureError(`Codex feature discovery unavailable: ${errorMessage(features.reason)}`);
      });
    getCodexStableDiscovery()
      .then((discovery) => { if (active) setCodexStableDiscovery(discovery); })
      .catch(() => { if (active) setCodexStableDiscovery(null); });
    listProjects()
      .then((response) => {
        if (active) {
          setProjects(response.projects);
          setArchivedProjects(response.archived_projects ?? []);
        }
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
    getGovernorOverview()
      .then(async (response) => {
        if (active) {
          setGovernorOverview(response);
          setOrganizationAssignments(response.assignments);
        }
        const proposals = await listGovernorProposals(response.active_organization.organization_id);
        if (active) setGovernorProposals(proposals.proposals);
      })
      .catch((error) => {
        if (active) setAgentError(errorMessage(error));
      });
    getGovernorControlPolicy()
      .then((policy) => { if (active) setGovernorPolicy(policy); })
      .catch((error) => { if (active) setAgentError(errorMessage(error)); });
    listManagedWorktrees()
      .then((response) => { if (active) setManagedWorktrees(response.worktrees); })
      .catch((error) => { if (active) setAgentError(errorMessage(error)); });
    listAgentRuns()
      .then((response) => {
        if (active) setAgentRuns(response.runs);
      })
      .catch((error) => {
        if (active) setAgentError(errorMessage(error));
      });
    listGlobalAgentSessions()
      .then((response) => {
        if (active) setGlobalAgentSessions(response.sessions);
      })
      .catch((error) => {
        if (active) setAgentError(errorMessage(error));
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
        interface_complexity: next.interface_complexity,
        autonomy: next.autonomy,
        shadow_experiments: next.shadow_experiments,
        reason
      });
      setUserSettings(saved);
      setAdvanced(saved.interface_complexity !== "simple");
      setSettingsHistory((current) => [...current, { settings: saved, reason, changed_at: saved.updated_at }]);
    } catch (error) {
      setSettingsError(errorMessage(error));
      const current = await getUserSettings().catch(() => null);
      if (current) {
        setUserSettings(current);
        setAdvanced(current.interface_complexity !== "simple");
      }
    } finally {
      setSettingsBusy(false);
    }
  }

  async function setInterfaceComplexity(nextComplexity: InterfaceComplexity) {
    if (!userSettings || settingsBusy) return;
    const restoreFocus = document.activeElement === modeButtonRef.current;
    if (nextComplexity === "simple") {
      setBackupPassphrase("");
      setBackupConfirmation("");
      setProviderCredential("");
      setMcpCredential("");
      setCodexCredential("");
    }
    await saveUserSettings(
      { ...userSettings, interface_complexity: nextComplexity },
      `Enabled ${nextComplexity} interface`
    );
    if (restoreFocus) {
      window.requestAnimationFrame(() => modeButtonRef.current?.focus());
    }
  }

  async function cycleInterfaceComplexity() {
    if (!userSettings) return;
    const next: InterfaceComplexity = userSettings.interface_complexity === "simple"
      ? "advanced"
      : userSettings.interface_complexity === "advanced" ? "developer" : "simple";
    await setInterfaceComplexity(next);
  }

  async function saveCodexTransport() {
    if (!userSettings || codexTransportBusy) return;
    setCodexTransportBusy(true);
    setCodexTransportError(null);
    try {
      const saved = await updateCodexTransport(
        userSettings.revision,
        codexRemoteEndpoint.trim() || undefined,
        codexRemoteAuthTokenEnv.trim() || undefined,
      );
      setUserSettings(saved);
      setSettingsHistory((current) => [...current, { settings: saved, reason: "Updated Codex transport from desktop", changed_at: saved.updated_at }]);
      setCodexHarnessStatus(await getCodexHarnessStatus());
    } catch (error) {
      setCodexTransportError(errorMessage(error));
      const current = await getUserSettings().catch(() => null);
      if (current) {
        setUserSettings(current);
        setCodexRemoteEndpoint(current.codex_remote_endpoint ?? "");
        setCodexRemoteAuthTokenEnv(current.codex_remote_auth_token_env ?? "");
      }
    } finally {
      setCodexTransportBusy(false);
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

  async function startSharedCodexLogin(method: "browser" | "device_code" | "api_key" | "access_token", credential?: string) {
    setBusy(true);
    setProviderError(null);
    try {
      if (method === "browser" && codexAccount?.authenticated) {
        window.localStorage.removeItem("mcg.codex-account-disconnected");
        setCodexAccountDisconnected(false);
        setProviderStatus("Using shared Codex account");
        return;
      }
      const response = await startCodexLogin(method, credential);
      if (method === "api_key" || method === "access_token") {
        const account = await getCodexAccount();
        setCodexAccount(account);
        setCodexAccountDisconnected(false);
        window.localStorage.removeItem("mcg.codex-account-disconnected");
        setProviderStatus(account.authenticated ? "Using shared Codex account" : "Credential accepted; account refresh pending");
        await refreshCodexCatalog();
        return;
      }
      const started = response as CodexLoginStart;
      setCodexLogin(started);
      const authorizationUrl = started.authUrl ?? started.verificationUrl;
      setProviderStatus(method === "device_code" ? "Waiting for device authorization" : "Waiting for browser authorization (up to 3 minutes)");
      if (authorizationUrl) await openExternalUrl(authorizationUrl);
      const deadline = Date.now() + 180_000;
      while (Date.now() < deadline) {
        await new Promise((resolve) => window.setTimeout(resolve, 1_000));
        const account = await getCodexAccount();
        if (account.authenticated) {
          window.localStorage.removeItem("mcg.codex-account-disconnected");
          setCodexAccountDisconnected(false);
          setCodexAccount(account);
          setCodexLogin(null);
          setProviderStatus("Using shared Codex account");
          await refreshCodexCatalog();
          return;
        }
      }
      if (started.loginId) await cancelCodexLogin(started.loginId).catch(() => undefined);
      setCodexLogin(null);
      setProviderStatus("Sign-in timed out");
      setProviderError("Sign-in timed out after 3 minutes. Start sign-in again when you’re ready.");
    } catch (error) {
      setCodexLogin(null);
      setProviderError(errorMessage(error));
      setProviderStatus("Connection failed");
    } finally {
      setCodexCredential("");
      setBusy(false);
    }
  }

  async function startLogin(method: OpenAiOAuthMethod) {
    setBusy(true);
    setProviderError(null);
    if (method === "browser") {
      await startSharedCodexLogin("browser");
      return;
    }
    try {
      const started = await startOpenAiOAuth(method);
      setLogin(started);
      setProviderStatus(started.user_code
        ? "Waiting for device authorization"
        : "Waiting for browser authorization (up to 3 minutes)");
      try {
        await openExternalUrl(started.verification_url);
      } catch (error) {
        const retryLabel = started.user_code ? "Open device authorization page" : "Open sign-in page";
        setProviderError(`${errorMessage(error)}. Use “${retryLabel}” below.`);
      }
      setBusy(false);
      await finishLogin(started);
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

  async function finishLogin(attempt: OAuthLoginStart) {
    try {
      const completed = await completeOpenAiOAuth(attempt.session_id);
      setProviderStatus(`${completed.connection.display_name} connected`);
      setProviders((current) => [
        { connection: completed.connection, catalog: completed.catalog },
        ...current.filter((item) => item.connection.id !== completed.connection.id)
      ]);
      setLogin((current) => current?.session_id === attempt.session_id ? null : current);
      setProviderError(null);
      try {
        const refreshed = await listProviders();
        setProviders(refreshed.providers);
        await discoverModelCatalogs(refreshed.providers);
      } catch (error) {
        setProviderError(`Connected, but the account profile could not be refreshed: ${errorMessage(error)}`);
      }
    } catch (error) {
      const message = errorMessage(error);
      setLogin((current) => current?.session_id === attempt.session_id ? null : current);
      setProviderError(/timed out|expired/i.test(message)
        ? attempt.user_code
          ? "Device sign-in timed out. Start sign-in again when you’re ready."
          : "Sign-in timed out after 3 minutes. Start sign-in again when you’re ready."
        : message);
      setProviderStatus(/timed out|expired/i.test(message) ? "Sign-in timed out" : "Connection failed");
    }
  }

  async function refreshGitHub() {
    try {
      const status = await githubStatus();
      setGitHubConnection(status);
      setGitHubRepositories(status.connected ? (await listGitHubRepositories()).repositories : []);
      setGitHubError(null);
    } catch (error) {
      setGitHubError(errorMessage(error));
    }
  }

  async function signInToGitHub() {
    setGitHubBusy(true);
    setGitHubError(null);
    try {
      const started = await startGitHubLogin();
      await openExternalUrl(started.authorization_url);
      const status = await completeGitHubLogin(started.session_id);
      setGitHubConnection(status);
      setGitHubRepositories((await listGitHubRepositories()).repositories);
    } catch (error) {
      setGitHubError(errorMessage(error));
    } finally {
      setGitHubBusy(false);
    }
  }

  async function signOutOfGitHub() {
    setGitHubBusy(true);
    try {
      setGitHubConnection(await disconnectGitHub());
      setGitHubRepositories([]);
      setGitHubError(null);
    } catch (error) {
      setGitHubError(errorMessage(error));
    } finally {
      setGitHubBusy(false);
    }
  }

  async function createAndLinkGitHubRepository() {
    if (!workspaceProject || !githubRepositoryName.trim()) return;
    setGitHubBusy(true);
    setGitHubError(null);
    try {
      const repository = await createGitHubRepository({ name: githubRepositoryName.trim(), private: githubRepositoryPrivate });
      await linkGitHubRepository(workspaceProject.project_id, repository.clone_url);
      setGitHubRepositories((current) => [repository, ...current.filter((item) => item.id !== repository.id)]);
      setGitHubRepositoryName("");
    } catch (error) {
      setGitHubError(errorMessage(error));
    } finally {
      setGitHubBusy(false);
    }
  }

  async function linkExistingGitHubRepository(repository: GitHubRepository) {
    if (!workspaceProject) return;
    setGitHubBusy(true);
    setGitHubError(null);
    try {
      await linkGitHubRepository(workspaceProject.project_id, repository.clone_url);
    } catch (error) {
      setGitHubError(errorMessage(error));
    } finally {
      setGitHubBusy(false);
    }
  }

  async function initializeSelectedProjectGit() {
    if (!workspaceProject) return;
    setGitHubBusy(true);
    setGitHubError(null);
    try {
      const mutation = await initializeProjectGit(workspaceProject.project_id);
      setSelectedProject(mutation.project);
      setProjects((current) => [mutation.project, ...current.filter((project) => project.project_id !== mutation.project.project_id)]);
    } catch (error) {
      setGitHubError(errorMessage(error));
    } finally {
      setGitHubBusy(false);
    }
  }

  function generatedProviderId(): string {
    const source = (providerId || providerName || "provider").toLowerCase();
    const slug = source.replace(/[^a-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "");
    return (slug || "provider").slice(0, 128);
  }

  async function discoverModelCatalogs(items: ProviderListItem[], force = false) {
    const candidates = items.filter((item) => force || modelCatalogNeedsDiscovery(item));
    if (candidates.length === 0) {
      setModelDiscoveryError(null);
      return;
    }
    setModelDiscoveryBusy(true);
    setModelDiscoveryError(null);
    const results = await Promise.allSettled(candidates.map(async (item) => ({
      connectionId: item.connection.id,
      providerName: item.connection.display_name,
      catalog: await discoverProvider(item.connection.id)
    })));
    const discovered = new Map(results.flatMap((result) => result.status === "fulfilled"
      ? [[result.value.connectionId, result.value.catalog] as const]
      : []));
    if (discovered.size > 0) {
      setProviders((current) => current.map((item) => discovered.has(item.connection.id)
        ? { ...item, catalog: discovered.get(item.connection.id) }
        : item));
    }
    const failures = results.flatMap((result, index) => result.status === "rejected"
      ? [`${candidates[index].connection.display_name}: ${errorMessage(result.reason)}`]
      : []);
    if (failures.length > 0) {
      setModelDiscoveryError(`Could not load available models. ${failures.join("; ")}`);
    }
    setModelDiscoveryBusy(false);
  }

  async function refreshCodexCatalog() {
    setModelDiscoveryBusy(true);
    setModelDiscoveryError(null);
    try {
      const [models, features, account, limits, harness, configuration, pendingRequests] = await Promise.all([
        getCodexModels(), getCodexFeatures(), getCodexAccount(), getCodexRateLimits(), getCodexHarnessStatus(), getCodexConfigurationStatus(), getCodexPendingRequests()
      ]);
      setCodexModels(models);
      setCodexFeatures(features);
      setCodexAccount(account);
      setCodexRateLimits(limits);
      setCodexHarnessStatus(harness);
      setCodexConfigurationStatus(configuration);
      setCodexPendingRequests(pendingRequests);
      setCodexStableDiscovery(await getCodexStableDiscovery([], true));
    } catch (error) {
      setModelDiscoveryError(`Could not load the Codex account catalog. ${errorMessage(error)}`);
    } finally {
      setModelDiscoveryBusy(false);
    }
  }

  async function refreshCodexHooks(forceReload = true) {
    setCodexHookError(null);
    setCodexHookBusy("refresh");
    try {
      const root = currentWorkspaceProject()?.root;
      setCodexStableDiscovery(await getCodexStableDiscovery(root ? [root] : [], forceReload));
    } catch (error) {
      setCodexHookError(errorMessage(error));
    } finally {
      setCodexHookBusy(null);
    }
  }

  async function changeCodexHook(entry: CodexHooksListEntry, hook: CodexHookMetadata, action: "trust" | "enable" | "disable") {
    if (hook.isManaged) return;
    if (action === "trust" && !window.confirm(`Trust the current ${hook.eventName} hook definition from ${hook.sourcePath}? Future changes will require review again.`)) return;
    setCodexHookError(null);
    setCodexHookBusy(hook.key);
    try {
      setCodexStableDiscovery(await updateCodexHookState({
        cwd: entry.cwd,
        key: hook.key,
        expectedCurrentHash: hook.currentHash,
        enabled: action === "enable" ? true : action === "disable" ? false : undefined,
        trustCurrentDefinition: action === "trust"
      }));
    } catch (error) {
      setCodexHookError(errorMessage(error));
    } finally {
      setCodexHookBusy(null);
    }
  }

  async function changeCodexFeature(feature: CodexFeature, enabled: boolean) {
    setCodexFeatureBusy(feature.name);
    setCodexFeatureError(null);
    try {
      setCodexFeatures(await setCodexFeature(feature.name, enabled));
    } catch (error) {
      setCodexFeatureError(errorMessage(error));
    } finally {
      setCodexFeatureBusy(null);
    }
  }

  async function refreshWindowsSandboxStatus() {
    const [configuration, harness] = await Promise.all([
      getCodexConfigurationStatus(),
      getCodexHarnessStatus()
    ]);
    setCodexConfigurationStatus(configuration);
    setCodexHarnessStatus(harness);
  }

  async function configureWindowsSandbox() {
    setWindowsSandboxBusy("setup");
    setWindowsSandboxError(null);
    setWindowsSandboxNotice(null);
    try {
      const result = await setupWindowsSandbox("elevated", windowsSandboxProjectId || undefined);
      await refreshWindowsSandboxStatus();
      setWindowsSandboxNotice(`Elevated Windows sandbox is configured and reports ${result.readiness}.`);
    } catch (error) {
      setWindowsSandboxError(errorMessage(error));
    } finally {
      setWindowsSandboxBusy(null);
    }
  }

  async function chooseWindowsSandboxReadFolder() {
    try {
      const path = await pickSandboxReadFolder();
      if (path) setWindowsSandboxReadPath(path);
    } catch (error) {
      setWindowsSandboxError(errorMessage(error));
    }
  }

  async function addWindowsSandboxReadDirectory() {
    if (!windowsSandboxProjectId) {
      setWindowsSandboxError("Choose the project whose Codex sandbox should receive this read-only folder.");
      return;
    }
    if (!windowsSandboxReadPath.trim()) {
      setWindowsSandboxError("Choose an existing absolute folder to grant read access.");
      return;
    }
    setWindowsSandboxBusy("read");
    setWindowsSandboxError(null);
    setWindowsSandboxNotice(null);
    try {
      const result = await grantWindowsSandboxReadDirectory(
        windowsSandboxProjectId,
        windowsSandboxReadPath.trim(),
        windowsSandboxPermissionProfile
      );
      setWindowsSandboxReadPath(result.path);
      setWindowsSandboxNotice(`Codex sandbox read access is ready for ${result.path}.`);
    } catch (error) {
      setWindowsSandboxError(errorMessage(error));
    } finally {
      setWindowsSandboxBusy(null);
    }
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
      await discoverModelCatalogs([{ connection: mutation.connection, catalog: mutation.catalog }]);
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

  async function refreshCodexUsage() {
    setCodexUsageBusy(true);
    setProviderError(null);
    try {
      const refreshed = await listProviders();
      setProviders(refreshed.providers);
    } catch (error) {
      setProviderError(errorMessage(error));
    } finally {
      setCodexUsageBusy(false);
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

  function chooseAccountAvatar(file: File | undefined, accountKey: string) {
    setAccountAvatarError(null);
    if (!file) return;
    if (!ACCOUNT_AVATAR_TYPES.has(file.type)) {
      setAccountAvatarError("Choose a PNG, JPEG, WebP, or GIF image.");
      return;
    }
    if (file.size > MAX_ACCOUNT_AVATAR_BYTES) {
      setAccountAvatarError("Choose an image smaller than 2 MB.");
      return;
    }
    const reader = new FileReader();
    reader.onerror = () => setAccountAvatarError("MetaCog could not read that image.");
    reader.onload = () => {
      const dataUrl = typeof reader.result === "string" ? reader.result : "";
      if (!/^data:image\/(?:jpeg|png|webp|gif);base64,/i.test(dataUrl)) {
        setAccountAvatarError("That file is not a supported image.");
        return;
      }
      const next = { accountKey, dataUrl };
      try {
        window.localStorage.setItem(ACCOUNT_AVATAR_STORAGE_KEY, JSON.stringify(next));
        setStoredAccountAvatar(next);
      } catch {
        setAccountAvatarError("MetaCog could not save that image locally.");
      }
    };
    reader.readAsDataURL(file);
  }

  function removeAccountAvatar() {
    window.localStorage.removeItem(ACCOUNT_AVATAR_STORAGE_KEY);
    setStoredAccountAvatar(null);
    setAccountAvatarError(null);
  }

  async function logOutOfOpenAi() {
    const oauthConnections = providers.filter(({ connection }) =>
      connection.kind === "open_ai"
      && (connection.auth_mode === "chat_gpt_browser_oauth" || connection.auth_mode === "chat_gpt_device_code")
    );
    setBusy(true);
    setProviderError(null);
    const disconnected = new Set<string>();
    try {
      for (const item of oauthConnections) {
        await disconnectProvider(item.connection.id);
        disconnected.add(item.connection.id);
      }
      window.localStorage.setItem("mcg.codex-account-disconnected", "true");
      setCodexAccountDisconnected(true);
      setProviderStatus("MetaCog disconnected from the shared Codex account");
      setOwnerMenuOpen(false);
    } catch (error) {
      setProviderError(errorMessage(error));
      setProviderStatus("Sign out failed");
    } finally {
      if (disconnected.size > 0) setProviders((current) => current.filter(({ connection }) => !disconnected.has(connection.id)));
      setBusy(false);
    }
  }

  async function signOutOfCodexDevice() {
    if (!window.confirm("Sign out of the shared Codex account on this device? This also affects Codex.")) return;
    setBusy(true);
    setProviderError(null);
    try {
      await logoutCodexAccount();
      setCodexAccount({ authenticated: false, requires_openai_auth: true });
      setCodexAccountDisconnected(false);
      window.localStorage.removeItem("mcg.codex-account-disconnected");
      setProviderStatus("Signed out of Codex on this device");
    } catch (error) {
      setProviderError(errorMessage(error));
    } finally {
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
    const label = plugin.active_revision.manifest.display_name;
    setTaskGoal(`Use @${label} to `);
    setComposerMentions([{ id: crypto.randomUUID(), candidate: {
      kind: "plugin",
      label,
      resource: plugin.installation.plugin_id,
      detail: `Plugin ${plugin.active_revision.manifest.version}`,
      plugin_revision_id: plugin.active_revision.id,
      installation_revision: plugin.installation.revision
    }, token: `@${label}` }]);
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
    setTasks((current) => {
      const existing = current.find((candidate) => candidate.id === task.id);
      const normalized = !task.thread_id && existing?.thread_id
        ? { ...task, thread_id: existing.thread_id }
        : task;
      return [normalized, ...current.filter((candidate) => candidate.id !== task.id)];
    });
  }

  async function sendMessageContent(
    content: string,
    targetConversation?: Conversation,
    preferences: ChatRunPreferences = newChatRunPreferences,
    mentions: ComposerMentionInput[] = []
  ): Promise<boolean> {
    if (!content.trim()) {
      setTaskError("Write a message to continue.");
      return false;
    }
    setTaskBusy(true);
    setTaskError(null);
    let pollHandle: number | undefined;
    try {
      const executionPreferences = {
        model_provider: preferences.model_provider,
        model_id: preferences.model_id,
        reasoning_effort: preferences.reasoning_effort,
        service_tier: preferences.service_tier,
        approval_mode: preferences.approval_mode,
        approval_policy: preferences.approval_policy,
        sandbox_mode: preferences.sandbox_mode,
        permission_profile: preferences.permission_profile,
        additional_writable_roots: preferences.additional_writable_roots,
        codex_feature_overrides: preferences.codex_feature_overrides,
        codex_config_overrides: preferences.codex_config_overrides,
        codex_profile: preferences.codex_profile,
        strict_config: preferences.strict_config,
        output_schema: preferences.output_schema,
        team_preset: preferences.team_preset,
        personality: preferences.personality
      };
      const existing = targetConversation ?? (activeSurface === "threads" && selectedChatId
        ? conversations.find((conversation) => conversation.id === selectedChatId)
        : undefined);
      const conversation = existing ?? await createConversation({
        title: content.trim().slice(0, 96),
        project_id: selectedProject?.project_id,
        execution_preferences: executionPreferences
      });
      if (!existing) setConversations((current) => [conversation, ...current.filter((candidate) => candidate.id !== conversation.id)]);
      setSelectedChatId(conversation.id);
      navigateSurface("threads");
      const optimisticTurnId = crypto.randomUUID();
      const optimisticView: ConversationView = conversationViews[conversation.id] ?? { conversation, turns: [], messages: [], activities: [] };
      setConversationViews((current) => ({
        ...current,
        [conversation.id]: {
          ...optimisticView,
          turns: [...optimisticView.turns, {
            schema_version: 1,
            id: optimisticTurnId,
            conversation_id: conversation.id,
            state: "pending",
            execution_preferences: executionPreferences,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString()
          }],
          messages: [...optimisticView.messages, {
            schema_version: 1,
            id: crypto.randomUUID(),
            conversation_id: conversation.id,
            turn_id: optimisticTurnId,
            role: "user",
            content: content.trim(),
            mentions: [],
            created_at: new Date().toISOString()
          }]
        }
      }));
      pollHandle = window.setInterval(() => {
        void Promise.all([getConversation(conversation.id), listApprovals()])
          .then(([streamedView, approvalList]) => {
            setConversationViews((current) => ({ ...current, [conversation.id]: streamedView }));
            setApprovals(approvalList.approvals);
          })
          .catch(() => undefined);
      }, 350);
      const view = await sendConversationMessage(conversation.id, { content, execution_preferences: executionPreferences, mentions });
      setConversationViews((current) => ({ ...current, [conversation.id]: view }));
      setConversations((current) => [view.conversation, ...current.filter((candidate) => candidate.id !== view.conversation.id)]);
      return true;
    } catch (error) {
      setTaskError(errorMessage(error));
      return false;
    } finally {
      if (pollHandle !== undefined) window.clearInterval(pollHandle);
      setTaskBusy(false);
    }
  }

  async function loadMentionCandidates(query: string) {
    const project = activeSurface === "threads" ? workspaceProject : selectedProject;
    const pluginCandidates: MentionCandidate[] = plugins
      .filter((plugin) => plugin.installation.enabled)
      .map((plugin) => ({
        kind: "plugin" as const,
        label: plugin.active_revision.manifest.display_name,
        resource: plugin.installation.plugin_id,
        detail: `Plugin ${plugin.active_revision.manifest.version}`,
        plugin_revision_id: plugin.active_revision.id,
        installation_revision: plugin.installation.revision
      }))
      .filter((candidate) => `${candidate.label} ${candidate.resource}`.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
    try {
      const projectCandidates = project
        ? (await searchProjectMentions(project.project_id, query, 50)).candidates
        : [];
      setMentionCandidates([...projectCandidates, ...pluginCandidates].slice(0, 60));
    } catch (error) {
      setMentionCandidates(pluginCandidates);
      setTaskError(errorMessage(error));
    }
  }

  function updateComposerValue(value: string, caret = value.length) {
    setTaskGoal(value);
    setComposerMentions((current) => current.filter((mention) => value.includes(mention.token)));
    const trigger = mentionTrigger(value, caret);
    if (trigger) {
      setMentionQuery(trigger.query);
      setMentionPaletteOpen(true);
      setAppPaletteOpen(false);
      setSlashPaletteOpen(false);
      void loadMentionCandidates(trigger.query);
    } else {
      setMentionPaletteOpen(false);
      const appTrigger = appReferenceTrigger(value, caret);
      if (appTrigger) {
        setAppQuery(appTrigger.query);
        setAppPaletteOpen(true);
        setSlashPaletteOpen(false);
      } else {
        setAppPaletteOpen(false);
        const slashOpen = slashCommandMatches(value).length > 0;
        setSlashPaletteOpen(slashOpen);
      }
    }
  }

  function chooseMention(candidate: MentionCandidate) {
    const textarea = document.querySelector<HTMLTextAreaElement>(activeSurface === "threads" ? "#chat-message" : "#goal");
    const caret = textarea?.selectionStart ?? taskGoal.length;
    const trigger = mentionTrigger(taskGoal, caret);
    if (!trigger) return;
    const token = `@${candidate.label}`;
    const next = `${taskGoal.slice(0, trigger.start)}${token} ${taskGoal.slice(trigger.end)}`;
    setTaskGoal(next);
    setComposerMentions((current) => [
      ...current.filter((mention) => `${mention.candidate.kind}:${mention.candidate.resource}` !== `${candidate.kind}:${candidate.resource}`),
      { id: crypto.randomUUID(), candidate, token }
    ]);
    setMentionPaletteOpen(false);
    setMentionQuery("");
    window.requestAnimationFrame(() => {
      const nextCaret = trigger.start + token.length + 1;
      textarea?.focus();
      textarea?.setSelectionRange(nextCaret, nextCaret);
    });
  }

  async function attachMentionFiles() {
    try {
      const paths = await pickMentionFiles();
      for (const path of paths.slice(0, 20)) {
        const label = path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
        const prefix = taskGoal && !/\s$/u.test(taskGoal) ? " " : "";
        setTaskGoal((current) => `${current}${prefix}@${label} `);
        setComposerMentions((current) => [...current, { id: crypto.randomUUID(), candidate: { kind: "file", label, resource: path, detail: path }, token: `@${label}` }]);
      }
    } catch (error) { setTaskError(errorMessage(error)); }
  }

  async function attachMentionFolder() {
    try {
      const path = await pickMentionFolder();
      if (!path) return;
      const label = path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
      const prefix = taskGoal && !/\s$/u.test(taskGoal) ? " " : "";
      setTaskGoal((current) => `${current}${prefix}@${label} `);
      setComposerMentions((current) => [...current, { id: crypto.randomUUID(), candidate: { kind: "folder", label, resource: path, detail: path }, token: `@${label}` }]);
    } catch (error) { setTaskError(errorMessage(error)); }
  }

  function openPluginMentions() {
    const prefix = taskGoal && !/\s$/u.test(taskGoal) ? " " : "";
    const next = `${taskGoal}${prefix}@`;
    setTaskGoal(next);
    setMentionQuery("");
    setMentionPaletteOpen(true);
    setAppPaletteOpen(false);
    setSlashPaletteOpen(false);
    void loadMentionCandidates("");
  }

  function openAppReferences() {
    const prefix = taskGoal && !/\s$/u.test(taskGoal) ? " " : "";
    const next = `${taskGoal}${prefix}$`;
    setTaskGoal(next);
    setAppQuery("");
    setAppPaletteOpen(true);
    setMentionPaletteOpen(false);
    setSlashPaletteOpen(false);
    window.requestAnimationFrame(() => {
      const textarea = document.querySelector<HTMLTextAreaElement>(activeSurface === "threads" ? "#chat-message" : "#goal");
      textarea?.focus();
      textarea?.setSelectionRange(next.length, next.length);
    });
  }

  function chooseAppReference(app: CodexAppInfo) {
    const textarea = document.querySelector<HTMLTextAreaElement>(activeSurface === "threads" ? "#chat-message" : "#goal");
    const caret = textarea?.selectionStart ?? taskGoal.length;
    const trigger = appReferenceTrigger(taskGoal, caret);
    if (!trigger) return;
    const token = `$${app.id}`;
    const next = `${taskGoal.slice(0, trigger.start)}${token} ${taskGoal.slice(trigger.end)}`;
    setTaskGoal(next);
    setAppPaletteOpen(false);
    setAppQuery("");
    window.requestAnimationFrame(() => {
      const nextCaret = trigger.start + token.length + 1;
      textarea?.focus();
      textarea?.setSelectionRange(nextCaret, nextCaret);
    });
  }

  function showCommandNotice(title: string, detail: string) {
    setCommandNotice({ title, detail });
    setCommandDialog("notice");
  }

  function chooseSlashCommand(name: SlashCommandName) {
    setTaskGoal(`/${name} `);
    setSlashPaletteOpen(false);
    if (name === "apps") {
      setAppQuery("");
      setAppPaletteOpen(true);
    } else {
      setAppPaletteOpen(false);
    }
    setTaskError(null);
    window.requestAnimationFrame(() => {
      const textarea = document.querySelector<HTMLTextAreaElement>(activeSurface === "threads" ? "#chat-message" : "#goal");
      textarea?.focus();
      textarea?.setSelectionRange(textarea.value.length, textarea.value.length);
    });
  }

  async function executeSlashCommand(content: string): Promise<boolean> {
    const command = parseSlashCommand(content);
    if (!command) return false;
    const project = currentWorkspaceProject() ?? selectedProject ?? undefined;
    const conversation = selectedChatId ? conversationViews[selectedChatId]?.conversation : undefined;
    const argument = command.argument;
    switch (command.name) {
      case "apps":
        if ((codexStableDiscovery?.apps ?? []).filter((app) => app.isEnabled !== false && app.isAccessible !== false).length === 0) {
          showCommandNotice("No enabled apps", "Connect or enable a Codex app before inserting an app reference.");
        } else {
          setTaskGoal(`/apps${argument ? ` ${argument}` : " "}`);
          setAppQuery(argument);
          setAppPaletteOpen(true);
        }
        return true;
      case "plan":
        if (selectedChatId && conversationIsActive) {
          await changeConversationMode("plan");
          if (argument) queueMessageContent(argument);
          setWorkspaceActionStatus(argument ? "Plan mode enabled; the planning request is queued." : "Plan mode enabled for the active chat.");
          return true;
        }
        return false;
      case "goal":
        if (selectedChatId && conversationIsActive && argument) {
          await changeConversationGoal(argument, "active");
          setWorkspaceActionStatus("Goal updated. It will govern the next continuation after this round finishes.");
          return true;
        }
        return false;
      case "approve": {
        const pending = approvals.find((approval) => approval.state === "pending" && (!conversation || approval.task_id === activeConversationTurn?.task_id));
        if (!pending) {
          showCommandNotice("Nothing to approve", "There is no recent automatic-review denial or pending permission request in this chat.");
          return true;
        }
        await resolveConversationApproval(pending.id, true);
        setWorkspaceActionStatus("Approved the most recent pending request once.");
        return true;
      }
      case "compact":
        if (!selectedChatId) {
          showCommandNotice("Start the chat first", "Context can be compacted after MetaCog has created the local Codex thread.");
        } else {
          await compactOwnedCodexThread(selectedChatId);
          setWorkspaceActionStatus("Context compaction started.");
        }
        return true;
      case "fast": {
        if (newChatRunPreferences.model_provider) {
          showCommandNotice("Fast mode unavailable", "Local Ollama and LM Studio runs do not expose an OpenAI subscription service tier.");
          return true;
        }
        const selectedModel = codexModels.find((model) => model.id === newChatRunPreferences.model_id) ?? codexModels.find((model) => model.isDefault);
        const fastTier = selectedModel?.serviceTiers.find((tier) => /fast|priority/i.test(`${tier.id} ${tier.name}`))?.id ?? "fast";
        const enabled = Boolean(newChatRunPreferences.service_tier);
        setNewChatRunPreferences((current) => ({ ...current, service_tier: enabled ? undefined : fastTier, speed: enabled ? "standard" : "fast" }));
        setWorkspaceActionStatus(`Fast responses ${enabled ? "off" : "on"}.`);
        return true;
      }
      case "feedback":
        setCommandDialog("feedback");
        return true;
      case "fork":
        if (!selectedChatId) showCommandNotice("Start the chat first", "A local thread exists after the first message and can then be forked.");
        else {
          await forkOwnedCodexThread(selectedChatId);
          setWorkspaceActionStatus("Forked the local Codex context for this MetaCog chat.");
        }
        return true;
      case "init":
        if (!project) {
          showCommandNotice("Choose a project first", "/init writes an AGENTS.md scaffold in the selected project.");
          return true;
        }
        await sendMessageContent("Create or update AGENTS.md for this project. Inspect the repository first, preserve existing instructions, and add concise, project-specific guidance for future coding agents.", conversation);
        return true;
      case "local":
        showCommandNotice("Local workspace selected", project ? `This chat runs locally with ${projectDisplayName(project)}.` : "This project-free chat runs locally in MetaCog's read-only scratch workspace.");
        return true;
      case "mcp":
        navigateSurface("mcp");
        return true;
      case "model": {
        if (!argument) {
          setCommandDialog("model");
          return true;
        }
        const model = codexModels.find((candidate) => candidate.id.toLocaleLowerCase() === argument.toLocaleLowerCase() || candidate.displayName.toLocaleLowerCase() === argument.toLocaleLowerCase());
        if (!model) {
          setTaskError(`Model “${argument}” is not available for this account.`);
          return true;
        }
        setNewChatRunPreferences((current) => ({ ...current, provider_id: "codex", model_provider: undefined, model_id: model.id, model_label: model.displayName, reasoning_effort: model.defaultReasoningEffort ?? current.reasoning_effort, service_tier: model.defaultServiceTier }));
        setWorkspaceActionStatus(`Model changed to ${model.displayName}.`);
        return true;
      }
      case "reasoning": {
        if (!argument) {
          setCommandDialog("reasoning");
          return true;
        }
        const normalized = argument.toLocaleLowerCase().replaceAll(" ", "");
        const effort = normalized === "extrahigh" ? "xhigh" : normalized;
        const model = codexModels.find((candidate) => candidate.id === newChatRunPreferences.model_id) ?? codexModels.find((candidate) => candidate.isDefault);
        if (model && !model.supportedReasoningEfforts.some((item) => item.reasoningEffort === effort)) {
          setTaskError(`Reasoning effort “${argument}” is not supported by ${model.displayName}.`);
          return true;
        }
        setNewChatRunPreferences((current) => ({ ...current, reasoning_effort: effort as ReasoningEffort }));
        setWorkspaceActionStatus(`Reasoning effort changed to ${argument}.`);
        return true;
      }
      case "personality": {
        if (!argument) {
          setCommandDialog("personality");
          return true;
        }
        const personality = argument.toLocaleLowerCase();
        if (!["default", "concise", "friendly", "professional"].includes(personality)) {
          setTaskError("Choose default, concise, friendly, or professional.");
          return true;
        }
        setNewChatRunPreferences((current) => ({ ...current, personality: personality === "default" ? undefined : personality }));
        setWorkspaceActionStatus(`Response personality changed to ${personality}.`);
        return true;
      }
      case "project":
        startNewChat(project ?? null);
        window.requestAnimationFrame(() => document.querySelector<HTMLDetailsElement>(".new-chat-project-selector")?.setAttribute("open", ""));
        return true;
      case "review":
        if (!project?.repository.is_git) {
          showCommandNotice("Choose a Git project first", "/review requires a selected Git-backed project.");
          return true;
        }
        await sendMessageContent(`${CODE_REVIEW_GOAL_PREFIX} in ${projectDisplayName(project)}: review ${argument || "all staged, unstaged, and untracked changes"}. Prioritize correctness, regressions, security, missing tests, and repository instructions. Report concise, actionable findings with file and line references, ordered by severity. Do not modify the working tree during the review.`, conversation);
        return true;
      case "side":
        startNewChat(project ?? null);
        setWorkspaceActionStatus("Side chat ready. The previous chat continues independently.");
        return true;
      case "status":
        setCommandDialog("status");
        return true;
      case "task":
        startNewChat(null);
        setWorkspaceActionStatus("Started a project-free chat.");
        return true;
      case "worktree":
        if (!project?.repository.is_git) {
          showCommandNotice("Choose a Git project first", "/worktree requires a selected Git-backed project.");
          return true;
        }
        if (!argument) {
          setTaskError("Add the coding objective after /worktree.");
          return true;
        }
        await sendMessageContent(`Complete this coding task through a managed Git worktree: ${argument}`, conversation);
        return true;
      case "cloud":
      case "cloud-environment":
        showCommandNotice("Cloud execution unavailable", "No Codex cloud environment is connected to MetaCog. Local projects and governed local worktrees remain available.");
        return true;
      case "ide-context":
        showCommandNotice("Shared IDE context unavailable", "MetaCog is a standalone desktop app and does not currently receive editor-selection context.");
        return true;
      case "memories":
        openSettings("general");
        setWorkspaceActionStatus("Opened memory settings. Automatic cross-chat memories are currently off.");
        return true;
      case "pet":
        showCommandNotice("Desktop pet unavailable", "MetaCog does not ship a desktop pet. This conditional Codex command remains listed for parity.");
        return true;
    }
  }

  async function addTask() {
    const content = taskGoal.trim();
    const mentions = materializeComposerMentions(content, composerMentions);
    if (!content) {
      setTaskError("Write a message to continue.");
      return;
    }
    setTaskGoal("");
    setComposerMentions([]);
    setMentionPaletteOpen(false);
    setAppPaletteOpen(false);
    if (content.startsWith("/") && !parseSlashCommand(content)) {
      setTaskError("Unknown slash command. Type / to browse available commands.");
      setTaskGoal(content);
      return;
    }
    try {
      if (await executeSlashCommand(content)) return;
    } catch (error) {
      setTaskError(errorMessage(error));
      setTaskGoal(content);
      return;
    }
    const sent = await sendMessageContent(content, undefined, newChatRunPreferences, mentions);
    if (!sent) {
      setTaskGoal((current) => current || content);
      setComposerMentions(composerMentions);
    }
  }

  async function changeConversationMode(mode: "default" | "plan") {
    if (!selectedChatId) return;
    setTaskError(null);
    try {
      const view = await updateConversationMode(selectedChatId, mode);
      setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      setConversations((current) => [view.conversation, ...current.filter((item) => item.id !== selectedChatId)]);
    } catch (error) {
      setTaskError(errorMessage(error));
    }
  }

  async function changeConversationGoal(objective?: string, status?: "active" | "paused") {
    if (!selectedChatId) return;
    setTaskError(null);
    try {
      const view = await updateConversationGoal(selectedChatId, { objective, status });
      setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      setConversations((current) => [view.conversation, ...current.filter((item) => item.id !== selectedChatId)]);
      setGoalEditOpen(false);
    } catch (error) {
      setTaskError(errorMessage(error));
    }
  }

  async function deleteConversationGoal() {
    if (!selectedChatId) return;
    setTaskError(null);
    try {
      const view = await clearConversationGoal(selectedChatId);
      setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      setConversations((current) => [view.conversation, ...current.filter((item) => item.id !== selectedChatId)]);
      setGoalEditOpen(false);
    } catch (error) {
      setTaskError(errorMessage(error));
    }
  }

  async function submitPlanningAnswers(request: ConversationInputRequest) {
    if (!selectedChatId) return;
    const answers = Object.fromEntries(request.questions.map((question) => [question.id, {
      answers: [planningAnswers[`${request.itemId}:${question.id}`]?.trim() ?? ""]
    }]));
    if (Object.values(answers).some((answer) => !answer.answers[0])) {
      setTaskError("Answer each planning question before continuing.");
      return;
    }
    setTaskError(null);
    try {
      const view = await answerConversationInput(selectedChatId, request.requestId, answers);
      setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      setPlanningAnswers((current) => Object.fromEntries(Object.entries(current).filter(([key]) => !key.startsWith(`${request.itemId}:`))));
    } catch (error) {
      setTaskError(errorMessage(error));
    }
  }

  async function recoverConversation(action: "resume" | "fork" | "retry") {
    if (!selectedChatId || !visibleConversation) return;
    const uncertainTurn = visibleConversation.turns.slice().reverse().find((turn) => turn.state === "indeterminate");
    if (!uncertainTurn) return;
    setCodexRecoveryBusy(true);
    setTaskError(null);
    try {
      if (action === "resume") {
        await reconcileOwnedCodexThread(selectedChatId);
      } else if (action === "fork") {
        await forkOwnedCodexThread(selectedChatId);
      } else {
        const original = visibleConversation.messages.find((message) => message.turn_id === uncertainTurn.id && message.role === "user");
        if (!original) throw new Error("The original user message is unavailable for an explicit retry.");
        await sendMessageContent(original.content, visibleConversation.conversation, {
          ...newChatRunPreferences,
          model_provider: uncertainTurn.execution_preferences.model_provider,
          model_id: uncertainTurn.execution_preferences.model_id,
          reasoning_effort: uncertainTurn.execution_preferences.reasoning_effort ?? newChatRunPreferences.reasoning_effort,
          service_tier: uncertainTurn.execution_preferences.service_tier,
          approval_mode: uncertainTurn.execution_preferences.approval_mode ?? newChatRunPreferences.approval_mode,
          approval_policy: uncertainTurn.execution_preferences.approval_policy,
          sandbox_mode: uncertainTurn.execution_preferences.sandbox_mode,
          permission_profile: uncertainTurn.execution_preferences.permission_profile,
          additional_writable_roots: uncertainTurn.execution_preferences.additional_writable_roots ?? [],
          codex_feature_overrides: uncertainTurn.execution_preferences.codex_feature_overrides ?? {},
          codex_config_overrides: uncertainTurn.execution_preferences.codex_config_overrides ?? [],
          codex_profile: uncertainTurn.execution_preferences.codex_profile,
          strict_config: uncertainTurn.execution_preferences.strict_config ?? false,
          output_schema: uncertainTurn.execution_preferences.output_schema,
          team_preset: uncertainTurn.execution_preferences.team_preset ?? newChatRunPreferences.team_preset
        });
      }
      const refreshed = await getConversation(selectedChatId);
      setConversationViews((current) => ({ ...current, [selectedChatId]: refreshed }));
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setCodexRecoveryBusy(false);
    }
  }

  async function startConversationReview() {
    if (!selectedChatId || !workspaceProject || !visibleConversation) return;
    setCodexReviewBusy(true);
    setTaskError(null);
    try {
      const projectName = projectDisplayName(workspaceProject);
      const sent = await sendMessageContent(
        `${CODE_REVIEW_GOAL_PREFIX} in ${projectName}: review all staged, unstaged, and untracked changes. Prioritize correctness, regressions, security, missing tests, and repository instructions. Report concise, actionable findings with file and line references, ordered by severity. Do not modify the working tree during the review.`,
        visibleConversation.conversation
      );
      if (sent) setWorkspaceActionStatus("The uncommitted-changes review was added to this chat.");
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setCodexReviewBusy(false);
    }
  }

  function queueMessageContent(value: string) {
    const content = value.trim();
    if (!content || !selectedChatId) return;
    setQueuedChatMessages((current) => [...current, {
      id: crypto.randomUUID(),
      conversation_id: selectedChatId,
      content,
      preferences: { ...newChatRunPreferences },
      mentions: materializeComposerMentions(content, composerMentions),
      created_at: new Date().toISOString()
    }]);
  }

  function queueDraftMessage() {
    const content = taskGoal.trim();
    if (!content || !selectedChatId) return;
    queueMessageContent(content);
    setTaskGoal("");
    setComposerMentions([]);
    setTaskError(null);
  }

  async function steerMessage(content: string, queuedMessageId?: string, queuedMentions?: ComposerMentionInput[]) {
    if (!selectedChatId || !activeConversationTurn || !content.trim()) return;
    const operationId = queuedMessageId ?? "draft";
    setSteeringMessageId(operationId);
    setTaskError(null);
    try {
      let turn = activeConversationTurn;
      for (let attempt = 0; attempt < 30 && (!turn.codex_thread_id || !turn.codex_turn_id); attempt += 1) {
        await new Promise((resolve) => window.setTimeout(resolve, 100));
        const refreshed = await getConversation(selectedChatId);
        setConversationViews((current) => ({ ...current, [selectedChatId]: refreshed }));
        turn = refreshed.turns.find((candidate) => candidate.id === activeConversationTurn.id) ?? turn;
      }
      if (!turn.codex_turn_id) throw new Error("The active Codex turn is still starting. Try steering again in a moment.");
      const mentions = queuedMentions ?? materializeComposerMentions(content.trim(), composerMentions);
      const view = await steerConversationTurn(selectedChatId, turn.id, content.trim(), mentions);
      setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      if (queuedMessageId) {
        setQueuedChatMessages((current) => current.filter((item) => item.id !== queuedMessageId));
      } else {
        setTaskGoal("");
        setComposerMentions([]);
      }
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setSteeringMessageId(null);
    }
  }

  function handleComposerKeyDown(event: ReactKeyboardEvent<HTMLTextAreaElement>) {
    if (mentionPaletteOpen) {
      if (event.key === "Escape") {
        event.preventDefault();
        setMentionPaletteOpen(false);
        return;
      }
      if ((event.key === "Enter" || event.key === "Tab") && mentionCandidates.length > 0 && !event.shiftKey && !event.nativeEvent.isComposing) {
        event.preventDefault();
        chooseMention(mentionCandidates[0]);
        return;
      }
    }
    const appCandidates = (codexStableDiscovery?.apps ?? [])
      .filter((app) => app.isEnabled !== false && app.isAccessible !== false)
      .filter((app) => !appQuery || `${app.name} ${app.id} ${app.description ?? ""}`.toLocaleLowerCase().includes(appQuery.toLocaleLowerCase()));
    if (appPaletteOpen) {
      if (event.key === "Escape") {
        event.preventDefault();
        setAppPaletteOpen(false);
        return;
      }
      if ((event.key === "Enter" || event.key === "Tab") && !event.shiftKey && !event.nativeEvent.isComposing) {
        event.preventDefault();
        if (appCandidates.length > 0) chooseAppReference(appCandidates[0]);
        else setTaskError("No matching enabled apps are available.");
        return;
      }
    }
    const commandMatches = slashCommandMatches(taskGoal);
    if ((event.key === "Enter" || event.key === "Tab") && commandMatches.length > 0 && !parseSlashCommand(taskGoal) && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      chooseSlashCommand(commandMatches[0].name);
      return;
    }
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
    event.preventDefault();
    if (parseSlashCommand(taskGoal)) {
      void addTask();
      return;
    }
    const active = activeSurface === "threads" && Boolean(activeConversationTurn || taskBusy);
    if (active && (event.ctrlKey || event.metaKey)) {
      void steerMessage(taskGoal);
    } else if (active) {
      queueDraftMessage();
    } else {
      void addTask();
    }
  }

  async function startTask(taskId: string) {
    setTaskBusy(true);
    setTaskError(null);
    setTaskLaunch((statuses) => ({ ...statuses, [taskId]: "Planning and starting governed work..." }));
    try {
      const launched = await launchTask(taskId, taskOrganizationIds[taskId] || undefined, taskRunPreferences[taskId] ?? newChatRunPreferences);
      replaceTask(launched.task);
      setTaskRunIds((runs) => ({ ...runs, [taskId]: launched.run.run.id }));
      setTaskLaunch((statuses) => ({ ...statuses, [taskId]: launched.execution.detail }));
      setAgentRuns((runs) => [launched.run, ...runs.filter((run) => run.run.id !== launched.run.run.id)]);
      await refreshAgents();
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
      const retried = await retryTask(taskId, taskRunPreferences[taskId] ?? newChatRunPreferences);
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
      const resumed = await resumeTask(taskId, taskRunPreferences[taskId] ?? newChatRunPreferences);
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

  async function resolveConversationApproval(approvalId: string, approved: boolean) {
    setConversationApprovalBusy(true);
    setTaskError(null);
    try {
      await (approved ? approveRequest(approvalId) : denyRequest(approvalId));
      setApprovals((current) => current.filter((approval) => approval.id !== approvalId));
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setConversationApprovalBusy(false);
    }
  }

  async function openApprovalSource(approvalId: string) {
    setApprovalSourceBusy(approvalId);
    setApprovalSourceError(null);
    try {
      const approval = await getApproval(approvalId);
      const conversationId = approval.origin?.conversation?.locator;
      if (!conversationId) throw new Error("This approval predates exact source binding or was created outside a conversation.");
      if (!conversations.some((conversation) => conversation.id === conversationId)) {
        throw new Error(`Originating conversation ${conversationId} is not available in the current compatibility projection.`);
      }
      const view = conversationViews[conversationId] ?? await getConversation(conversationId);
      if (view.conversation.id !== conversationId) {
        throw new Error(`Conversation inspection returned ${view.conversation.id} instead of exact origin ${conversationId}.`);
      }
      setConversationViews((current) => ({ ...current, [conversationId]: view }));
      setApprovals((current) => current.some((candidate) => candidate.id === approval.id)
        ? current.map((candidate) => candidate.id === approval.id ? approval : candidate)
        : [...current, approval]);
      setFocusedApprovalSource(approval);
      openChat(conversationId);
    } catch (error) {
      setApprovalSourceError(errorMessage(error));
    } finally {
      setApprovalSourceBusy(null);
    }
  }

  async function openCanonicalInvocationSource(source: CanonicalSourceRef) {
    setInvocationBusy(source.locator);
    setInvocationError(null);
    setApprovalSourceError(null);
    try {
      if (source.source_type !== "mcg.invocation_manifest.v1") {
        throw new Error(`Invocation source type ${source.source_type} is not a canonical InvocationManifest.`);
      }
      const inspection = await getProtectedRootInvocation(source.locator);
      if (!inspection) throw new Error(`Exact invocation ${source.locator} is unavailable from Protected Root.`);
      if (inspection.manifest.id !== source.locator) {
        throw new Error(`Invocation inspection returned ${inspection.manifest.id} instead of exact origin ${source.locator}.`);
      }
      const actualHash = inspection.manifest_hash;
      if (actualHash.algorithm !== source.content_hash.algorithm
        || actualHash.domain !== source.content_hash.domain
        || actualHash.digest_hex !== source.content_hash.digest_hex) {
        throw new Error(`Invocation ${source.locator} no longer matches its immutable approval binding.`);
      }
      setInvocationInspections((current) => ({ ...current, [source.locator]: inspection }));
      setFocusedInvocationId(source.locator);
      setAgentTab("overview");
      navigateSurface("agents");
      window.requestAnimationFrame(() => {
        document.getElementById(`invocation-${source.locator}`)?.scrollIntoView?.({ block: "center" });
      });
    } catch (error) {
      const message = errorMessage(error);
      setInvocationError(message);
      setApprovalSourceError(message);
    } finally {
      setInvocationBusy(null);
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
      setMemoryError("Name the fact you want MetaCog to remember.");
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

  async function duplicateSchedule(schedule: ScheduleDefinition) {
    setScheduleBusy(true);
    setScheduleError(null);
    try {
      const duplicate = await createSchedule({
        schedule_id: freshScheduleId(),
        name: `${schedule.name} copy`,
        project_id: schedule.project_id,
        goal: schedule.goal,
        constraints: [],
        success_conditions: [],
        risk: "medium",
        trigger: schedule.trigger,
        context_token_budget: 8192,
        requested_tools: [],
        estimated_output_tokens: 8192,
        max_model_calls: 16
      });
      replaceSchedule(duplicate);
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
      setSelectedBrowserSessionId(sessionId);
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
      setSelectedBrowserSessionId(sessionId);
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

  async function refreshAgents() {
    const [overview, runs, sessions, policy, worktrees] = await Promise.all([
      getGovernorOverview(),
      listAgentRuns(),
      listGlobalAgentSessions(),
      getGovernorControlPolicy(),
      listManagedWorktrees()
    ]);
    const proposals = await listGovernorProposals(overview.active_organization.organization_id);
    setGovernorOverview(overview);
    setOrganizationAssignments(overview.assignments);
    setAgentRuns(runs.runs);
    setGlobalAgentSessions(sessions.sessions);
    setGovernorPolicy(policy);
    setGovernorProposals(proposals.proposals);
    setManagedWorktrees(worktrees.worktrees);
  }

  async function refreshManagedWorktrees() {
    try {
      const response = await listManagedWorktrees();
      setManagedWorktrees(response.worktrees);
    } catch (error) {
      setTaskError(errorMessage(error));
    }
  }

  async function assignGlobalOrganization(organizationId: string) {
    setAgentBusy(true);
    setAgentError(null);
    try {
      const response = await setGlobalOrganizationAssignment(organizationId);
      setOrganizationAssignments(response.assignments);
      await refreshAgents();
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setAgentBusy(false);
    }
  }

  async function assignProjectOrganization(projectId: string, organizationId?: string) {
    setAgentBusy(true);
    setAgentError(null);
    try {
      const response = await setProjectOrganizationAssignment(projectId, organizationId);
      setOrganizationAssignments(response.assignments);
      await refreshAgents();
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setAgentBusy(false);
    }
  }

  function editOrganization(organization: OrganizationGenome) {
    setOrganizationDraft(structuredClone(organization));
    setOrganizationPreview(null);
    setOrganizationChangeSummary("");
    setAgentError(null);
    setAgentTab("teams");
    navigateSurface("agents");
  }

  async function previewOrganizationChanges() {
    if (!organizationDraft) return;
    setAgentBusy(true);
    setAgentError(null);
    try {
      setOrganizationPreview(await previewOrganizationRevision(organizationDraft.organization_id, organizationDraft));
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setAgentBusy(false);
    }
  }

  async function publishOrganizationChanges() {
    if (!organizationPreview || !organizationChangeSummary.trim()) return;
    setAgentBusy(true);
    setAgentError(null);
    try {
      const mutation = await publishOrganizationRevision(organizationPreview, organizationChangeSummary);
      setOrganizationDraft(null);
      setOrganizationPreview(null);
      setOrganizationChangeSummary("");
      await Promise.all([refreshOrganizations(mutation.organization.organization_id), refreshAgents()]);
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setAgentBusy(false);
    }
  }

  async function cancelRun(runId: string) {
    setAgentBusy(true);
    setAgentError(null);
    try {
      await cancelAgentRun(runId);
      await refreshAgents();
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setAgentBusy(false);
    }
  }

  async function decideGovernor(proposal: GovernorProposal, approve: boolean) {
    setAgentBusy(true);
    setAgentError(null);
    try {
      await decideGovernorProposal(proposal.id, approve);
      const response = await listGovernorProposals(proposal.organization_id);
      setGovernorProposals(response.proposals);
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setAgentBusy(false);
    }
  }

  async function handGovernorProposalToChat(proposal: GovernorProposal) {
    const conversationId = governorConversationId
      || (selectedChatId && conversations.some((conversation) => conversation.id === selectedChatId) ? selectedChatId : "");
    if (!conversationId) {
      setAgentError("Choose a project conversation for the approved implementation request.");
      return;
    }
    setAgentBusy(true);
    setAgentError(null);
    try {
      await requestGovernorImplementation(proposal.id, conversationId);
      const response = await listGovernorProposals(proposal.organization_id);
      setGovernorProposals(response.proposals);
      await openChat(conversationId);
    } catch (error) {
      setAgentError(errorMessage(error));
    } finally {
      setAgentBusy(false);
    }
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
  const conversationChatSummaries = conversations
    .filter((conversation): conversation is Conversation => Boolean(conversation) && conversation.state === "active" && (!normalizedChatSearch || conversation.title.toLocaleLowerCase().includes(normalizedChatSearch)))
    .map((conversation) => {
      const linkedTaskIds = new Set((conversationViews[conversation.id]?.turns ?? []).flatMap((turn) => turn.task_id ? [turn.task_id] : []));
      return { id: conversation.id, title: conversation.title || "Chat", conversation, tasks: tasks.filter((task) => linkedTaskIds.has(task.id)) };
    });
  // Schema migration normally materializes legacy task-backed chats as conversations.
  // Keep a projection fallback so an interrupted upgrade (and older API clients) never
  // makes durable work disappear from the sidebar before that migration is observed.
  // A task whose project has been removed from the catalog is retained as runtime
  // evidence, but must not be resurrected as an unrelated chat under General.
  const conversationIds = new Set(conversationChatSummaries.map((chat) => chat.id));
  const knownSidebarProjectIds = new Set([...projects, ...archivedProjects].map((project) => project.project_id));
  const legacyTaskChatSummaries = tasks
    .filter((task) => !conversationIds.has(chatIdForTask(task)))
    .filter((task) => !task.project_id || knownSidebarProjectIds.has(task.project_id))
    .filter((task) => !normalizedChatSearch || task.goal.toLocaleLowerCase().includes(normalizedChatSearch))
    .map((task) => ({
      id: chatIdForTask(task),
      title: task.goal,
      conversation: {
        schema_version: 1,
        id: chatIdForTask(task),
        title: task.goal,
        project_id: task.project_id,
        state: "active" as const,
        execution_preferences: {},
        created_at: task.created_at,
        updated_at: task.updated_at
      },
      tasks: [task]
    }));
  const chatSummaries = [...conversationChatSummaries, ...legacyTaskChatSummaries];
  const isChatPinned = (chat: typeof chatSummaries[number]) => pinnedTaskIds.includes(chat.id) || chat.tasks.some((task) => pinnedTaskIds.includes(task.id));
  const pinnedChats = chatSummaries
    .filter(isChatPinned)
    .sort((left, right) => {
      const leftIndex = Math.max(0, pinnedTaskIds.findIndex((id) => id === left.id || left.tasks.some((task) => task.id === id)));
      const rightIndex = Math.max(0, pinnedTaskIds.findIndex((id) => id === right.id || right.tasks.some((task) => task.id === id)));
      return leftIndex - rightIndex;
    });
  const knownSidebarProjects = [...projects, ...archivedProjects];
  const unassignedChats = chatSummaries.filter((chat) => !chat.conversation.project_id || !knownSidebarProjects.some((project) => project.project_id === chat.conversation.project_id));
  const recentChats = chatSummaries
    .slice()
    .sort((left, right) => Date.parse(right.conversation.updated_at) - Date.parse(left.conversation.updated_at))
    .slice(0, 7);
  const archivedConversations = conversations
    .filter((conversation) => conversation.state === "archived" && (!normalizedChatSearch || conversation.title.toLocaleLowerCase().includes(normalizedChatSearch)))
    .sort((left, right) => Date.parse(right.updated_at) - Date.parse(left.updated_at));
  const contextChat = chatContextMenu ? chatSummaries.find((chat) => chat.id === chatContextMenu.chatId) : undefined;
  const visibleTasks = tasks;
  const visibleConversation = selectedChatId ? conversationViews[selectedChatId] : undefined;
  const managedConversationWorktrees = managedWorktrees.map((worktree) => ({
    worktree,
    conversationId: worktree.conversation_id,
    delegation: conversationViews[worktree.conversation_id]?.team?.delegations.find((candidate) => candidate.id === worktree.delegation_id)
  }));
  const activeConversationTurn = visibleConversation?.turns
    .slice()
    .reverse()
    .find((turn) => ["pending", "streaming", "waiting_for_approval", "recovering"].includes(turn.state));
  const indeterminateConversationTurn = visibleConversation?.turns
    .slice()
    .reverse()
    .find((turn) => turn.state === "indeterminate");
  const visibleQueuedMessages = selectedChatId
    ? queuedChatMessages.filter((message) => message.conversation_id === selectedChatId)
    : [];
  const conversationIsActive = Boolean(activeConversationTurn || taskBusy);
  const conversationTeamIsRunning = conversationTeamHasPendingDelivery(visibleConversation?.team);
  const conversationGoal = visibleConversation?.conversation.goal;
  const conversationGoalElapsed = conversationGoal
    ? conversationGoal.timeUsedSeconds + (conversationGoal.status === "active"
      ? Math.max(0, Math.floor(Date.now() / 1000) - conversationGoal.updatedAt)
      : 0)
    : 0;

  useEffect(() => {
    if (activeSurface !== "threads" || !selectedChatId || !conversationGoal) return;
    const refresh = () => {
      setGoalClock((value) => value + 1);
      void getConversation(selectedChatId).then((view) => {
        setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
        setConversations((current) => [view.conversation, ...current.filter((item) => item.id !== selectedChatId)]);
      }).catch(() => undefined);
    };
    const timer = window.setInterval(refresh, 1000);
    return () => window.clearInterval(timer);
  }, [activeSurface, selectedChatId, conversationGoal?.threadId, conversationGoal?.status]);

  useEffect(() => {
    if (activeSurface !== "threads" || !selectedChatId || !conversationTeamIsRunning) return;
    const refresh = () => {
      void getConversation(selectedChatId).then((view) => {
        setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      }).catch(() => undefined);
    };
    const timer = window.setInterval(refresh, 1000);
    return () => window.clearInterval(timer);
  }, [activeSurface, selectedChatId, conversationTeamIsRunning]);

  useEffect(() => {
    const next = visibleQueuedMessages[0];
    if (
      activeSurface !== "threads"
      || !selectedChatId
      || conversationIsActive
      || !next
      || queueDispatchingRef.current
      || queueDispatchErrorId === next.id
    ) return;
    const conversation = conversations.find((candidate) => candidate.id === selectedChatId);
    if (!conversation) return;
    queueDispatchingRef.current = next.id;
    void sendMessageContent(next.content, conversation, next.preferences, next.mentions).then((sent) => {
      if (sent) {
        setQueuedChatMessages((current) => current.filter((item) => item.id !== next.id));
        setQueueDispatchErrorId(null);
      } else {
        setQueueDispatchErrorId(next.id);
      }
    }).finally(() => {
      queueDispatchingRef.current = null;
    });
  }, [activeSurface, selectedChatId, conversationIsActive, visibleQueuedMessages, conversations, queueDispatchErrorId]);

  useEffect(() => {
    if (activeSurface !== "threads" || !visibleConversation) return;
    chatShouldFollowRef.current = true;
    const viewport = chatViewportRef.current;
    if (!viewport) return;
    const trackChatPosition = () => {
      chatShouldFollowRef.current = isChatViewportNearBottom(viewport);
    };
    viewport.addEventListener("scroll", trackChatPosition, { passive: true });
    return () => viewport.removeEventListener("scroll", trackChatPosition);
  }, [activeSurface, selectedChatId, visibleConversation?.conversation.id]);

  useEffect(() => {
    if (activeSurface !== "threads" || !visibleConversation || !chatShouldFollowRef.current) return;
    const frame = window.requestAnimationFrame(() => {
      const viewport = chatViewportRef.current;
      if (viewport && chatShouldFollowRef.current) viewport.scrollTop = viewport.scrollHeight;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [activeSurface, selectedChatId, visibleConversation]);

  const activeConversationApproval = activeConversationTurn?.task_id
    ? approvals.find((approval) => approval.task_id === activeConversationTurn.task_id)
    : undefined;
  const focusedApprovalOrigin = focusedApprovalSource?.origin;
  const focusedApprovalConversationMatches = Boolean(
    visibleConversation
    && focusedApprovalOrigin?.conversation?.locator === visibleConversation.conversation.id
  );
  const focusedApprovalAgent = focusedApprovalConversationMatches && focusedApprovalOrigin?.agent
    ? visibleConversation?.team?.sessions.find((session) => session.id === focusedApprovalOrigin.agent?.locator)
    : undefined;
  const focusedApprovalCommand = focusedApprovalConversationMatches && focusedApprovalOrigin?.command
    ? visibleConversation?.activities.find((activity) => activity.codex_item_id === focusedApprovalOrigin.command?.locator)
    : undefined;
  const focusedInvocationInspection = focusedInvocationId
    ? invocationInspections[focusedInvocationId]
    : undefined;
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
  const signedInWithOpenAi = (Boolean(codexAccount?.authenticated) && !codexAccountDisconnected) || openAiOAuthConnections.length > 0;
  const openAiAccount = openAiOAuthConnections[0];
  const openAiProfile = openAiAccount?.profile;
  const openAiAccountKey = codexAccount?.email || openAiProfile?.email || openAiProfile?.username || openAiProfile?.display_name || openAiAccount?.connection.id || "";
  const localOpenAiAvatarUrl = storedAccountAvatar?.accountKey === openAiAccountKey ? storedAccountAvatar.dataUrl : undefined;
  const openAiAvatarUrl = localOpenAiAvatarUrl || openAiProfile?.avatar_url;
  const openAiAccountName = openAiProfile?.display_name || openAiProfile?.username || codexAccount?.email || openAiProfile?.email || "OpenAI account";
  const openAiAccountDetail = openAiProfile?.username
    ? openAiProfile.username
    : codexAccount?.email || openAiProfile?.email || "Using shared Codex account";
  const openAiPlanLabel = accountPlanLabel(codexAccount?.plan_type || openAiProfile?.plan_type);
  const codexUsageWindows = [
    openAiProfile?.usage?.primary_window,
    openAiProfile?.usage?.secondary_window
  ].filter((window): window is CodexUsageWindow => Boolean(window));
  const nonSparkUsage = codexRateLimits?.primary
    ? { used_percent: codexRateLimits.primary.used_percent, remaining_percent: codexRateLimits.primary.remaining_percent, reset_at: codexRateLimits.primary.resets_at }
    : openAiProfile?.usage?.primary_window ?? openAiProfile?.usage?.secondary_window;
  const browserCapability = platformCapabilities?.capabilities.find((capability) => capability.kind === "browser_automation");
  const computerCapability = platformCapabilities?.capabilities.find((capability) => capability.kind === "computer_use");
  const activeBrowserSessions = platformSessions.filter((session) => session.kind === "browser" && ["requested", "active", "handed_off"].includes(session.state));
  const selectedBrowserSession = activeBrowserSessions.find((session) => session.id === selectedBrowserSessionId && session.state === "active")
    ?? activeBrowserSessions.find((session) => session.state === "active");
  const selectedBrowserObservation = selectedBrowserSession ? platformBrowsers[selectedBrowserSession.id] : undefined;
  const selectedBrowserPreviewUrl = selectedBrowserSession
    ? (selectedBrowserObservation?.safe_url && selectedBrowserObservation.safe_url !== "about:blank"
      ? selectedBrowserObservation.safe_url
      : selectedBrowserSession.allowed_origins[0])
    : undefined;
  const activeComputerSessions = platformSessions.filter((session) => session.kind === "computer_use" && ["requested", "active", "handed_off"].includes(session.state));
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
  const normalizedShortcutSearch = shortcutSearch.trim().toLocaleLowerCase();
  const filteredShortcuts = METACOG_SHORTCUTS.filter(([name, description, shortcut]) =>
    !normalizedShortcutSearch || `${name} ${description} ${shortcut}`.toLocaleLowerCase().includes(normalizedShortcutSearch)
  );
  const selectedAgentRun = agentRuns.find((run) => run.run.id === selectedAgentRunId) ?? null;
  const normalizedGlobalAgentSearch = globalAgentSearch.trim().toLocaleLowerCase();
  const visibleGlobalAgentSessions = globalAgentSessions.filter((session) =>
    !normalizedGlobalAgentSearch || [
      session.display_name,
      session.conversation_title,
      session.kind,
      session.state,
      session.session_id,
      session.conversation_id,
      session.model ?? ""
    ].some((value) => value.toLocaleLowerCase().includes(normalizedGlobalAgentSearch))
  );
  const selectedAgentTask = selectedAgentRun
    ? tasks.find((task) => task.id === selectedAgentRun.run.task_id) ?? null
    : null;
  const selectedAgentApproval = selectedAgentTask
    ? approvals.find((approval) => approval.task_id === selectedAgentTask.id && approval.state === "pending") ?? null
    : null;
  const activeOrganization = governorOverview?.active_organization
    ?? organizations.find((organization) => organization.organization_id === organizationAssignments?.global_organization_id)
    ?? organizations[0];
  const discoveredModels = providers.flatMap((provider) => provider.catalog?.models ?? []);
  const activeSurfaceTitle = activeSurface === "tasks"
    ? "New chat"
    : activeSurface === "threads"
      ? chatSummaries.find((chat) => chat.id === selectedChatId)?.title ?? "Chats"
      : activeSurface === "mcp"
        ? "Tools"
        : activeSurface === "ssh"
          ? "SSH"
          : activeSurface === "pull-requests"
            ? "Pull requests"
            : activeSurface === "improvement"
              ? "Agents"
            : activeSurface.charAt(0).toUpperCase() + activeSurface.slice(1);
  const workspaceTask = selectedChatId ? tasks.find((task) => chatIdForTask(task) === selectedChatId) : undefined;
  const workspaceConversation = selectedChatId ? conversations.find((conversation) => conversation.id === selectedChatId) : undefined;
  const workspaceProjectId = visibleConversation?.conversation.project_id ?? workspaceConversation?.project_id ?? workspaceTask?.project_id;
  const workspaceProject = projects.find((project) => project.project_id === workspaceProjectId) ?? (selectedChatId ? undefined : selectedProject ?? undefined);
  const workspaceProjectName = workspaceProject?.root.split(/[\\/]/).filter(Boolean).at(-1) ?? "No project selected";
  const discoveredHookCount = codexStableDiscovery?.hooks.reduce((total, entry) => total + entry.hooks.length, 0) ?? 0;
  const selectedCommandLocalProvider = newChatRunPreferences.model_provider
    ? codexConfigurationStatus?.local_providers?.providers.find((provider) => provider.id === newChatRunPreferences.model_provider)
    : undefined;
  const commandLocalModels = selectedCommandLocalProvider
    ? Array.from(new Set([selectedCommandLocalProvider.default_model, ...selectedCommandLocalProvider.models]))
    : [];
  const activeLocalSites = sites.filter((site) => site.state !== "archived" && isChatGptSiteUrl(site.public_url));
  const localSitesByUrl = new Map(activeLocalSites.map((site) => [comparableSiteUrl(site.public_url), site]));
  const accountSiteUrls = new Set(codexSites.map((site) => comparableSiteUrl(site.public_url)));
  const siteLibraryEntries: SiteLibraryEntry[] = [
    ...codexSites.filter((site) => !["archived", "deleted"].includes(site.status.toLowerCase())).map((site) => ({
      key: `account-${site.id}`,
      displayName: site.display_name,
      description: site.description,
      publicUrl: site.public_url,
      screenshotUrl: site.screenshot_url,
      localSite: localSitesByUrl.get(comparableSiteUrl(site.public_url)),
      accountSite: site
    })),
    ...activeLocalSites.filter((site) => !accountSiteUrls.has(comparableSiteUrl(site.public_url))).map((site) => ({
      key: `local-${site.id}`,
      displayName: site.display_name,
      publicUrl: site.public_url,
      localSite: site
    }))
  ];
  const contextSite = siteContextMenu
    ? siteLibraryEntries.find((site) => site.key === siteContextMenu.key)
    : undefined;

  useEffect(() => {
    if (activeSurface !== "threads" || !selectedChatId || !workspaceConversation?.project_id) return;
    let watching = true;
    void watchConversationProject(selectedChatId).catch((error) => {
      if (watching) setTaskError(`Project watch unavailable. ${errorMessage(error)}`);
    });
    return () => {
      watching = false;
      void unwatchConversationProject(selectedChatId).catch(() => undefined);
    };
  }, [activeSurface, selectedChatId, workspaceConversation?.project_id]);

  const dedicatedPullRequestWorkspace = activeSurface === "pull-requests" && !advanced;
  const workspacePanelsVisible = activeSurface !== "settings" && !dedicatedPullRequestWorkspace;
  const workspaceLayoutStyle = {
    "--sidebar-width": `${sidebarWidth}px`,
    "--settings-nav-width": `${settingsNavWidth}px`,
    "--workspace-side-width": `${workspaceSideWidth}px`,
    "--workspace-bottom-height": `${workspaceBottomHeight}px`
  } as CSSProperties;
  const activeSiteBrowserTab = siteBrowserTabs.find((tab) => tab.id === activeSiteBrowserTabId) ?? null;
  const codeReviewTasks = tasks.filter((task) => task.goal.startsWith(CODE_REVIEW_GOAL_PREFIX));
  const filteredCodeReviewTasks = codeReviewTasks.filter((task) => {
    const matchesTab = codeReviewTab === "all"
      || codeReviewTab === "authored"
      || (codeReviewTab === "reviewing" && !(["completed", "failed", "cancelled"] as string[]).includes(task.state));
    if (!matchesTab) return false;
    const terminal = (["completed", "failed", "cancelled"] as string[]).includes(task.state);
    if (codeReviewStatusFilter === "open" && terminal) return false;
    if (codeReviewStatusFilter === "finished" && !terminal) return false;
    const project = projects.find((candidate) => candidate.project_id === task.project_id);
    const searchable = `${task.goal} ${project?.root ?? ""}`.toLocaleLowerCase();
    return searchable.includes(codeReviewSearch.trim().toLocaleLowerCase());
  });
  const selectedCodeReviewTask = codeReviewTasks.find((task) => task.id === selectedCodeReviewTaskId) ?? null;

  const renderChatRow = (chat: typeof chatSummaries[number], source: ChatMenuSource) => {
    const pinned = isChatPinned(chat);
    const durable = conversations.some((conversation) => conversation.id === chat.id);
    const legacyTaskIds = chat.tasks.map((task) => task.id);
    return <div
      className={`chat-row${selectedChatId === chat.id ? " active" : ""}${unreadChatIds.includes(chat.id) ? " unread" : ""}`}
      data-chat-source={source}
      onContextMenu={(event) => openChatContextMenu(event, chat.id, source)}
      key={`${source}-${chat.id}`}
    >
      <button className="chat-link" type="button" aria-label={source === "recent" ? `${chat.title} (recent)` : undefined} onClick={() => openChat(chat.id)}>{chat.title}</button>
      <div className="chat-row-actions">
        <button type="button" className="chat-row-action" aria-label={pinned ? `Unpin ${chat.title}` : `Pin ${chat.title}`} title={pinned ? "Unpin" : "Pin"} onClick={(event) => { event.stopPropagation(); togglePinnedTask(chat.id, legacyTaskIds); }}><PinIcon solid={pinned} /></button>
        <button type="button" className="chat-row-action" aria-label={`Archive ${chat.title}`} title={durable ? "Archive" : "Archive is available after chat migration"} disabled={!durable || chatMutationBusy} onClick={(event) => { event.stopPropagation(); void setChatArchived(chat.id, true); }}><ArchiveIcon /></button>
      </div>
    </div>;
  };

  const renderArchivedChatRow = (conversation: Conversation) => <div className="chat-row archived-chat-row" key={`archived-${conversation.id}`}>
    <button className="chat-link" type="button" title="Restore and open" onClick={() => void restoreArchivedChat(conversation.id, true)}>{conversation.title || "Chat"}</button>
    <div className="chat-row-actions">
      <button type="button" className="chat-row-action" aria-label={`Restore ${conversation.title || "Chat"}`} title="Restore" disabled={chatMutationBusy} onClick={(event) => { event.stopPropagation(); void restoreArchivedChat(conversation.id); }}><span aria-hidden="true">↶</span></button>
    </div>
  </div>;

  return (
    <div ref={scrollChainRootRef} className={`desktop-frame${sidebarOpen ? "" : " sidebar-collapsed"}${workspaceSidePanelOpen && workspacePanelsVisible ? " side-panel-open" : ""}${workspaceBottomPanelOpen && workspacePanelsVisible ? " bottom-panel-open" : ""}`} style={workspaceLayoutStyle}>
      <header className="desktop-menubar" aria-label="Application menu">
        <div className="desktop-titlebar-brand" data-tauri-drag-region aria-label="MetaCog">
          <img src={metaCogLogo} alt="" />
          <strong>MetaCog</strong>
        </div>
        <button type="button" className="chrome-button sidebar-toggle" aria-label={sidebarOpen ? "Hide sidebar" : "Show sidebar"} aria-pressed={sidebarOpen} onClick={() => setSidebarOpen((current) => !current)}><PanelLeft aria-hidden="true" /></button>
        <button type="button" className="chrome-button" aria-label="Back" title="Back" onClick={() => window.history.back()}><ArrowLeft aria-hidden="true" /></button>
        <button type="button" className="chrome-button" aria-label="Forward" title="Forward" onClick={() => window.history.forward()}><ArrowRight aria-hidden="true" /></button>
        <div className="desktop-menu-list" data-transient-popover-root="desktop-menu">
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
                <button type="button" onClick={() => void closeCurrentWindow(true)}><span>Quit MetaCog</span><span className="menu-shortcut">Ctrl+Q</span></button>
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
                <button type="button" onClick={() => { setWorkspaceBottomPanelOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Bottom Panel</span><span className="menu-shortcut">Ctrl+J</span></button>
                <button type="button" onClick={() => { setWorkspacePinnedSummaryOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Pinned Summary</span></button>
                <button type="button" onClick={() => { setWorkspaceBottomPanelOpen(true); setDesktopMenu(null); window.requestAnimationFrame(() => terminalInputRef.current?.focus()); }}><span>Open Terminal</span><span className="menu-shortcut">Ctrl+`</span></button>
                <button type="button" onClick={() => { setProjectTreeOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle File Tree</span><span className="menu-shortcut">Ctrl+Shift+E</span></button>
                <button type="button" onClick={() => { setWorkspaceSidePanelOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Side Panel</span><span className="menu-shortcut">Ctrl+Alt+B</span></button>
                <button type="button" disabled={!userSettings || settingsBusy} onClick={() => { setDesktopMenu(null); void cycleInterfaceComplexity(); }}><span>Interface: {userSettings?.interface_complexity ?? "simple"}</span></button>
                <hr />
                <div className="desktop-menu-submenu">
                  <button type="button" aria-haspopup="menu"><span>Browser</span><span className="menu-arrow" aria-hidden="true">›</span></button>
                  <div role="menu" aria-label="Browser submenu"><button type="button" role="menuitem" onClick={() => openSettings("browser")}><span>Browser Settings</span></button><button type="button" role="menuitem" onClick={() => navigateSurface("platform")}><span>Open Browser Workspace</span></button></div>
                </div>
                <hr />
                <button type="button" onClick={() => { setSidebarOpen(true); setSidebarSearchOpen(true); setDesktopMenu(null); window.requestAnimationFrame(() => chatSearchRef.current?.focus()); }}><span>Find</span><span className="menu-shortcut">Ctrl+F</span></button>
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
                <button type="button" disabled title="MetaCog documentation is being prepared"><span>Documentation</span></button>
                <button type="button" onClick={() => openSettings("shortcuts")}><span>Keyboard Shortcuts</span><span className="menu-shortcut">Ctrl+/</span></button>
                <button type="button" disabled title="Release notes are not available yet"><span>What's New</span></button>
                <hr />
                <button type="button" onClick={() => openSettings("configuration")}><span>Troubleshooting</span></button>
                <button type="button" onClick={() => openSettings("configuration")}><span>System Status</span></button>
                <button type="button" disabled title="A feedback destination is not configured"><span>Send Feedback</span></button>
                <hr />
                <button type="button" onClick={() => navigateSurface("threads")}><span>Chat history</span></button>
                <button type="button" disabled title="Performance tracing is not available yet"><span>Start Performance Trace</span></button>
                <hr />
                <button type="button" disabled={updateBusy} onClick={() => { openSettings("configuration"); void checkUpdates(); }}><span>{updateBusy ? "Checking for Updates…" : "Check for Updates…"}</span></button>
                <button type="button" onClick={() => openSettings("configuration")}><span>About MetaCog</span></button>
              </>}
            </div>
          </div>)}
        </div>
        <div className="window-drag-region" data-tauri-drag-region onDoubleClick={() => void toggleCurrentWindowMaximized()} />
        <div className="window-controls" aria-label="Window controls">
          <button type="button" aria-label="Minimize" title="Minimize" onClick={() => void minimizeCurrentWindow()}><Minus aria-hidden="true" /></button>
          <button type="button" aria-label="Maximize or restore" title="Maximize or restore" onClick={() => void toggleCurrentWindowMaximized()}><Maximize2 aria-hidden="true" /></button>
          <button className="window-close" type="button" aria-label="Close window" title="Close" onClick={() => void closeCurrentWindow()}><X aria-hidden="true" /></button>
        </div>
      </header>
      <div
        className={`app-shell ${advanced ? "advanced-mode" : "simple-mode"}`}
        data-active-surface={activeSurface}
        data-interface-complexity={userSettings?.interface_complexity ?? "simple"}
      >
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="sidebar-heading">
          <div className="sidebar-brand-row"><div className="brand" aria-label="MetaCog"><img className="brand-mark" src={metaCogLogo} alt="" /><span>MetaCog</span></div><div className="sidebar-brand-actions">
            <button type="button" aria-label="Search chats" title="Search" aria-pressed={sidebarSearchOpen} onClick={() => { setSidebarSearchOpen((current) => !current); window.requestAnimationFrame(() => chatSearchRef.current?.focus()); }}><Search aria-hidden="true" /></button>
            <button type="button" aria-label="Open activity" title="Activity" onClick={() => { setWorkspaceSidePanelTab("activity"); setWorkspaceSidePanelOpen(true); }}><Activity aria-hidden="true" /></button>
          </div></div>
          <label className={`chat-search${sidebarSearchOpen || chatSearch ? " open" : ""}`}><Search aria-hidden="true" /><span className="sr-only">Search chats</span><input ref={chatSearchRef} aria-label="Search chats" value={chatSearch} onChange={(event) => setChatSearch(event.target.value)} onBlur={() => { if (!chatSearch) setSidebarSearchOpen(false); }} placeholder="Search" /></label>
        </div>
        <nav className="sidebar-shortcuts" aria-label="Workspace pages">
          <button className={activeSurface === "tasks" ? "nav-item active" : "nav-item"} type="button" onClick={() => startNewChat()}><SquarePen className="shortcut-glyph" aria-hidden="true" />New chat</button>
          {([
            { surface: "pull-requests", label: "Pull requests", Icon: GitPullRequest },
            { surface: "projects", label: "Projects", Icon: FolderOpen },
            { surface: "sites", label: "Sites", Icon: PanelsTopLeft },
            { surface: "scheduler", label: "Scheduled", Icon: Clock3 },
            { surface: "agents", label: "Agents", Icon: Workflow },
            { surface: "plugins", label: "Plugins", Icon: Blocks }
          ]).map(({ surface, label, Icon }) => {
            const selected = activeSurface === surface || (surface === "agents" && activeSurface === "improvement");
            return <a className={`nav-item${selected ? " active" : ""}`} href={`#${surface}`} aria-current={selected ? "page" : undefined} onClick={(event) => openSurface(event, surface)} key={surface}><Icon className="shortcut-glyph" aria-hidden="true" />{label}</a>;
          })}
        </nav>
        <div className="sidebar-library">
          {pinnedSummaryOpen && <section className="sidebar-section" aria-label="Pinned chats"><h2>Pinned</h2>{pinnedChats.length === 0 ? <p>No pinned chats</p> : pinnedChats.map((chat) => renderChatRow(chat, "pinned"))}</section>}
          {projectTreeOpen && <section className="sidebar-section project-tree" aria-label="Projects"><h2>Projects</h2>{projects.length === 0 && <p>No active projects</p>}{projects.map((project) => {
            const name = project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root;
            const projectChats = chatSummaries.filter((chat) => chat.conversation.project_id === project.project_id);
            return <div className="project-tree-item" key={project.project_id}><div className="project-tree-heading"><button className="project-tree-name" type="button" onClick={() => startNewChat(project)}><span aria-hidden="true">▱</span>{name}</button><div className="project-tree-actions"><button className="project-action" type="button" aria-label={`New chat in ${name}`} title={`New chat in ${name}`} disabled={projectMutationBusy === project.project_id} onClick={() => startNewChat(project)}><span aria-hidden="true">＋</span></button><button className="project-action" type="button" aria-label={`Archive ${name}`} title="Archive project" disabled={projectMutationBusy === project.project_id} onClick={() => void setProjectArchived(project, true)}><ArchiveIcon /></button><button className="project-action project-delete" type="button" aria-label={`Delete ${name}`} title="Remove project from MetaCog" disabled={projectMutationBusy === project.project_id} onClick={() => void deleteProject(project)}><TrashIcon /></button></div></div>{projectChats.length === 0 ? <p>No chats</p> : projectChats.map((chat) => renderChatRow(chat, "project"))}</div>;
          })}{archivedProjects.length > 0 && <div className="archived-projects"><button className="sidebar-section-toggle" type="button" aria-expanded={archivedProjectsOpen} onClick={() => setArchivedProjectsOpen((current) => { window.localStorage.setItem("mcg.sidebar-archived-projects-open", String(!current)); return !current; })}><span>Archived projects</span><span aria-hidden="true">{archivedProjectsOpen ? "⌄" : "›"}</span></button>{archivedProjectsOpen && archivedProjects.map((project) => { const name = project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root; return <div className="project-tree-heading archived-project-row" key={`archived-project-${project.project_id}`}><span className="project-tree-name"><span aria-hidden="true">▱</span>{name}</span><div className="project-tree-actions"><button className="project-action" type="button" aria-label={`Restore ${name}`} title="Restore project" disabled={projectMutationBusy === project.project_id} onClick={() => void setProjectArchived(project, false)}><span aria-hidden="true">↶</span></button><button className="project-action project-delete" type="button" aria-label={`Delete ${name}`} title="Remove project from MetaCog" disabled={projectMutationBusy === project.project_id} onClick={() => void deleteProject(project)}><TrashIcon /></button></div></div>; })}</div>}{unassignedChats.length > 0 && <div className="project-tree-item"><button type="button" onClick={() => navigateSurface("threads")}><span aria-hidden="true">▱</span>General</button>{unassignedChats.map((chat) => renderChatRow(chat, "project"))}</div>}</section>}
          <section className="sidebar-section" aria-label="Recent chats">
            <button className="sidebar-section-toggle" type="button" aria-expanded={recentsOpen} onClick={() => setRecentsOpen((current) => { window.localStorage.setItem("mcg.sidebar-recents-open", String(!current)); return !current; })}><span>Recents</span><span aria-hidden="true">{recentsOpen ? "⌄" : "›"}</span></button>
            {recentsOpen && (recentChats.length === 0 ? <p>No recent chats</p> : recentChats.map((chat) => renderChatRow(chat, "recent")))}
          </section>
          <section className="sidebar-section" aria-label="Archived chats">
            <button className="sidebar-section-toggle" type="button" aria-expanded={archivedChatsOpen} onClick={() => setArchivedChatsOpen((current) => { window.localStorage.setItem("mcg.sidebar-archived-open", String(!current)); return !current; })}><span>Archived</span><span aria-hidden="true">{archivedChatsOpen ? "⌄" : "›"}</span></button>
            {archivedChatsOpen && (archivedConversations.length === 0 ? <p>No archived chats</p> : archivedConversations.map(renderArchivedChatRow))}
          </section>
        </div>
        <div className="sidebar-account-area" data-transient-popover-root="owner-menu">
          <button className="owner-menu-button" type="button" aria-label="Open owner menu" aria-expanded={ownerMenuOpen} onClick={() => setOwnerMenuOpen((current) => !current)}><AccountAvatar className="owner-avatar" profile={signedInWithOpenAi ? openAiProfile : undefined} avatarUrl={signedInWithOpenAi ? openAiAvatarUrl : undefined} fallback={signedInWithOpenAi ? "O" : "M"} /><span><strong>{signedInWithOpenAi ? openAiAccountName : "Local owner"}</strong><small>{signedInWithOpenAi ? openAiAccountDetail : "Not signed in"}</small></span><span aria-hidden="true">⋯</span></button>
          <div className={`owner-menu${ownerMenuOpen ? " open" : ""}`} aria-label="Owner menu">
            <div className="owner-menu-identity"><AccountAvatar className="owner-avatar" profile={signedInWithOpenAi ? openAiProfile : undefined} avatarUrl={signedInWithOpenAi ? openAiAvatarUrl : undefined} fallback={signedInWithOpenAi ? "O" : "M"} /><span><strong>{signedInWithOpenAi ? openAiAccountName : "Local owner"}</strong><small>{signedInWithOpenAi ? openAiAccountDetail : "MetaCog workspace"}</small></span></div>
            <button
              className="owner-menu-usage"
              type="button"
              aria-label={nonSparkUsage ? `Usage, ${nonSparkUsage.remaining_percent}% remaining, next reset ${usageResetDateLabel(nonSparkUsage.reset_at)}` : "Usage"}
              onClick={() => { setSettingsTab("usage"); navigateSurface("settings"); }}
            >
              <span aria-hidden="true">◔</span>
              <span className="owner-menu-usage-copy">
                <span className="owner-menu-usage-heading"><span>Usage</span>{nonSparkUsage && <strong>{nonSparkUsage.remaining_percent}% left</strong>}</span>
                {nonSparkUsage
                  ? <>
                    <small>Next Reset: {usageResetDateLabel(nonSparkUsage.reset_at)}</small>
                    <i className="owner-menu-usage-meter" role="progressbar" aria-label="Usage remaining" aria-valuemin={0} aria-valuemax={100} aria-valuenow={nonSparkUsage.remaining_percent}><b style={{ width: `${nonSparkUsage.remaining_percent}%` }} /></i>
                  </>
                  : <small>{signedInWithOpenAi ? "Usage unavailable" : "View usage"}</small>}
              </span>
            </button>
            <button type="button" aria-label="Settings" onClick={() => { setSettingsTab("general"); navigateSurface("settings"); }}><span aria-hidden="true">⚙</span><span>Settings</span><small>Ctrl+,</small></button>
            <button className="owner-menu-mode" type="button" aria-label="Cycle interface complexity from owner menu" disabled={!userSettings || settingsBusy} onClick={() => { setOwnerMenuOpen(false); void cycleInterfaceComplexity(); }}><span aria-hidden="true">◐</span><span>Interface: {userSettings?.interface_complexity ?? "simple"}</span></button>
            {signedInWithOpenAi
              ? <button className="owner-menu-session" type="button" disabled={busy} onClick={() => void logOutOfOpenAi()}><span aria-hidden="true">↪</span><span>{busy ? "Logging out…" : "Log out"}</span></button>
              : <button className="owner-menu-session" type="button" disabled={busy || Boolean(login || codexLogin)} onClick={() => { setOwnerMenuOpen(false); setSettingsTab("account"); navigateSurface("settings"); void startLogin("browser"); }}><span aria-hidden="true">↪</span><span>{busy ? "Opening sign-in…" : "Sign in with ChatGPT"}</span></button>}
          </div>
        </div>
        <div
          className="sidebar-resizer"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize main sidebar"
          aria-valuemin={190}
          aria-valuemax={440}
          aria-valuenow={sidebarWidth}
          tabIndex={0}
          onPointerDown={beginSidebarResize}
          onDoubleClick={() => setSidebarWidth(256)}
          onKeyDown={(event) => {
            if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
            event.preventDefault();
            setSidebarWidth((current) => Math.min(440, Math.max(190, current + (event.key === "ArrowRight" ? 16 : -16))));
          }}
        />
      </aside>

      {chatContextMenu && contextChat && <div
        className="chat-context-menu"
        data-transient-popover-root="context-menu"
        role="menu"
        aria-label={`${chatContextMenu.source === "pinned" ? "Pinned" : "Project"} chat menu`}
        style={{ left: chatContextMenu.x, top: chatContextMenu.y }}
        onPointerDown={(event) => event.stopPropagation()}
        onContextMenu={(event) => event.preventDefault()}
      >
        {chatContextMenu.source === "pinned" && <>
          <button role="menuitem" type="button" disabled={pinnedChats.findIndex((chat) => chat.id === contextChat.id) <= 0} onClick={() => movePinnedChat(contextChat.id, -1)}><span aria-hidden="true">↑</span>Move up</button>
          <button role="menuitem" type="button" disabled={pinnedChats.findIndex((chat) => chat.id === contextChat.id) === pinnedChats.length - 1} onClick={() => movePinnedChat(contextChat.id, 1)}><span aria-hidden="true">↓</span>Move down</button>
        </>}
        <button role="menuitem" type="button" onClick={() => togglePinnedTask(contextChat.id, contextChat.tasks.map((task) => task.id))}><PinIcon solid={chatContextMenu.source === "pinned"} />{isChatPinned(contextChat) ? "Unpin" : "Pin"}</button>
        <button role="menuitem" type="button" disabled={!conversations.some((conversation) => conversation.id === contextChat.id) || chatMutationBusy} onClick={() => void renameChat(contextChat.id, contextChat.title, contextChat.conversation.project_id)}><span aria-hidden="true">✎</span>Rename</button>
        <button role="menuitem" type="button" onClick={() => { setUnreadChatIds((current) => current.includes(contextChat.id) ? current : [contextChat.id, ...current]); setChatContextMenu(null); }}><span aria-hidden="true">◉</span>Mark as unread</button>
        <button role="menuitem" type="button" disabled={!conversations.some((conversation) => conversation.id === contextChat.id) || chatMutationBusy} onClick={() => void setChatArchived(contextChat.id, true)}><ArchiveIcon />Archive</button>
        <hr />
        <div className="chat-context-submenu">
          <button role="menuitem" type="button" disabled={!conversations.some((conversation) => conversation.id === contextChat.id)}><span aria-hidden="true">▱</span>Project<span aria-hidden="true">›</span></button>
          <div role="menu" aria-label="Move chat to project">
            <button role="menuitem" type="button" onClick={() => void mutateConversation(contextChat.id, contextChat.title)}>General</button>
            {projects.map((project) => <button role="menuitem" type="button" key={project.project_id} onClick={() => void mutateConversation(contextChat.id, contextChat.title, project.project_id)}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</button>)}
          </div>
        </div>
        <hr />
        <button role="menuitem" type="button" onClick={() => void copyChatText(`${window.location.href.split("#")[0]}#threads/${encodeURIComponent(contextChat.id)}`, "Chat link copied")}><span aria-hidden="true">↥</span>Share</button>
        <button role="menuitem" type="button" onClick={() => void copyChatText(contextChat.title, "Chat title copied")}><span aria-hidden="true">▢</span>Copy title</button>
        <hr />
        <button role="menuitem" type="button" onClick={() => { setChatContextMenu(null); void openNewWindow(`${window.location.href.split("#")[0]}#threads/${encodeURIComponent(contextChat.id)}`); }}><span aria-hidden="true">▣</span>Open in new window</button>
      </div>}

      {siteContextMenu && contextSite && <div
        className="chat-context-menu site-context-menu"
        data-transient-popover-root="context-menu"
        role="menu"
        aria-label={`${contextSite.displayName} site menu`}
        style={{ left: siteContextMenu.x, top: siteContextMenu.y }}
        onPointerDown={(event) => event.stopPropagation()}
        onContextMenu={(event) => event.preventDefault()}
      >
        <button role="menuitem" type="button" title="Uses MetaCog's separate, persistent browser profile" onClick={() => void openSiteInApp(contextSite)}><span aria-hidden="true">◎</span>Open in MetaCog browser</button>
        <button role="menuitem" type="button" onClick={() => void openSiteExternally(contextSite)}><span aria-hidden="true">↗</span>Open in external browser</button>
      </div>}

      <main className="workspace" id="tasks" ref={workspaceRef} data-scroll-chain>
        <header className="topbar">
          <div className="topbar-title">
            <p className="eyebrow">METACOGNITIVE GOVERNOR</p>
            <h1><PanelsTopLeft aria-hidden="true" />{activeSurfaceTitle}</h1>
            <p className="topbar-subtitle">Build, review, and run work with a local AI team.</p>
          </div>
          <div className="workspace-toolbar" aria-label="Workspace controls">
            <button ref={modeButtonRef} className={`workspace-mode-toggle${advanced ? " advanced" : ""}`} type="button" aria-label="Cycle interface complexity" disabled={!userSettings || settingsBusy} onClick={() => void cycleInterfaceComplexity()}><SlidersHorizontal aria-hidden="true" />{userSettings?.interface_complexity === "developer" ? "Developer" : advanced ? "Advanced" : "Simple"}</button>
            <span className={`runtime-dot${localService && !localService.ready ? " status-error" : ""}`} title={localService?.detail} role="status"><i />{localService?.ready ? `v${localService.version}` : "Local runtime needs attention"}</span>
            <div className="workspace-toolbar-menu" data-transient-popover-root="workspace-menu">
              <button className="workspace-icon-button" type="button" aria-label="Share" title="Share" aria-expanded={workspacePopover === "share"} onClick={() => setWorkspacePopover((current) => current === "share" ? null : "share")}><Share2 aria-hidden="true" /></button>
              {workspacePopover === "share" && <div className="workspace-popover share-popover" role="menu" aria-label="Share workspace">
                <button type="button" role="menuitem" onClick={() => void shareWorkspaceSummary()}><Copy aria-hidden="true" /><span><strong>Copy share summary</strong><small>Chat, status, and project</small></span></button>
                <button type="button" role="menuitem" disabled={!workspaceProject} onClick={() => workspaceProject && void copyWorkspaceText(workspaceProject.root, "Project path copied")}><FolderOpen aria-hidden="true" /><span><strong>Copy project path</strong><small>{workspaceProjectName}</small></span></button>
              </div>}
            </div>
            <div className="workspace-toolbar-menu" data-transient-popover-root="workspace-menu">
              <button className="open-in-button" type="button" disabled={!workspaceProject} aria-haspopup="menu" aria-expanded={workspacePopover === "open-in"} title={workspaceProject ? `Open ${workspaceProjectName} in another application` : "Choose a project first"} onClick={() => setWorkspacePopover((current) => current === "open-in" ? null : "open-in")}><FolderOpen aria-hidden="true" />Open in<ChevronDown aria-hidden="true" /></button>
              {workspacePopover === "open-in" && <div className="workspace-popover open-in-popover" role="menu" aria-label="Open project in">
                <div className="workspace-popover-heading"><strong>Open {workspaceProjectName} in</strong></div>
                {openInApplications.filter((application) => application.available).map((application) => <button type="button" role="menuitem" key={application.id} onClick={() => void openCurrentWorkspace(application.id)}><span className="open-in-glyph" aria-hidden="true">{application.id === "file_explorer" ? "▱" : application.id === "terminal" || application.id === "git_bash" ? ">_" : "◆"}</span><span>{application.label}</span></button>)}
                {openInApplications.filter((application) => application.available).length === 0 && <p>Available in the installed desktop app.</p>}
              </div>}
            </div>
            <button className="workspace-icon-button" type="button" disabled={!selectedChatId || !workspaceProject || codexReviewBusy || conversationIsActive || taskBusy} aria-label="Review uncommitted changes" title="Review uncommitted project changes in this chat" onClick={() => void startConversationReview()}><Check aria-hidden="true" /></button>
            <button className={`workspace-icon-button${workspacePinnedSummaryOpen ? " active" : ""}`} type="button" aria-label="Toggle pinned summary" title="Toggle pinned summary" aria-pressed={workspacePinnedSummaryOpen} onClick={() => setWorkspacePinnedSummaryOpen((current) => !current)}><ListTree aria-hidden="true" /></button>
            <button className={`workspace-icon-button${workspaceBottomPanelOpen ? " active" : ""}`} type="button" aria-label="Toggle bottom panel" title="Toggle bottom panel (Ctrl+J)" aria-pressed={workspaceBottomPanelOpen} onClick={() => setWorkspaceBottomPanelOpen((current) => !current)}><PanelBottom aria-hidden="true" /></button>
            <button className={`workspace-icon-button${workspaceSidePanelOpen ? " active" : ""}`} type="button" aria-label="Toggle side panel" title="Toggle side panel (Ctrl+Alt+B)" aria-pressed={workspaceSidePanelOpen} onClick={() => setWorkspaceSidePanelOpen((current) => !current)}><PanelRight aria-hidden="true" /></button>
          </div>
        </header>

        {workspacePinnedSummaryOpen && workspacePanelsVisible && <aside className="pinned-summary-card" aria-label="Pinned summary">
          <div><strong>{workspaceTask?.goal ?? activeSurfaceTitle}</strong><button type="button" aria-label="Close pinned summary" onClick={() => setWorkspacePinnedSummaryOpen(false)}>×</button></div>
          <p>{workspaceTask ? `This conversation is ${workspaceTask.state.replaceAll("_", " ")}.` : "No chat is selected."} {workspaceProject ? `Working in ${workspaceProjectName}.` : "Choose a project to enable workspace tools."}</p>
          {workspaceTask && <dl><div><dt>Status</dt><dd>{workspaceTask.state.replaceAll("_", " ")}</dd></div><div><dt>Project</dt><dd>{workspaceProjectName}</dd></div></dl>}
        </aside>}
        {workspaceActionStatus && <button className="workspace-action-status" type="button" title="Dismiss" onClick={() => setWorkspaceActionStatus(null)}>{workspaceActionStatus}<span aria-hidden="true">×</span></button>}

        {(advanced || activeSurface === "tasks") && <section className="hero-card new-chat-surface" hidden={activeSurface !== "tasks"}>
          <div className="new-chat-heading">
            <img className="new-chat-mark" src={metaCogLogo} alt="" />
            <h2>{selectedProject ? <>What should we build in <span>{projectDisplayName(selectedProject)}</span>?</> : "What should we build?"}</h2>
          </div>
          <div className="new-chat-composer-shell">
            <div className="new-chat-context-bar" aria-label="Chat workspace context">
              <NewChatProjectSelector projects={projects} value={selectedProject} onChange={setSelectedProject} onOpenFolder={() => chooseProject("task")} disabled={taskBusy} />
              <NewChatWorkspaceSelector />
              {selectedProject?.repository.is_git && <NewChatBranchSelector project={selectedProject} />}
            </div>
            <div className="composer chat-input-composer">
              <label htmlFor="goal">Message</label>
              <textarea
                ref={goalInputRef}
                id="goal"
                value={taskGoal}
                onChange={(event) => updateComposerValue(event.target.value, event.target.selectionStart)}
                onKeyDown={handleComposerKeyDown}
                placeholder="Do anything"
              />
              <SlashCommandPalette value={taskGoal} open={slashPaletteOpen} onSelect={chooseSlashCommand} hasProject={Boolean(selectedProject)} hasConversation={false} />
              <MentionPalette open={mentionPaletteOpen} candidates={mentionCandidates} onSelect={chooseMention} />
              <AppReferencePalette open={appPaletteOpen} apps={codexStableDiscovery?.apps ?? []} query={appQuery} onSelect={chooseAppReference} />
              <div className="composer-actions">
                <NewChatAddMenu project={selectedProject} disabled={taskBusy} onAttachFiles={() => void attachMentionFiles()} onAttachFolder={() => void attachMentionFolder()} onApps={openAppReferences} onPlugins={openPluginMentions} onOpenFolder={() => chooseProject("task")} onClearProject={() => setSelectedProject(null)} onPlan={() => { setTaskGoal((value) => value.startsWith("/plan") ? value : `/plan ${value}`); window.requestAnimationFrame(() => goalInputRef.current?.focus()); }} onGoal={() => { setTaskGoal((value) => value.startsWith("/goal") ? value : `/goal ${value}`); window.requestAnimationFrame(() => goalInputRef.current?.focus()); }} onCommands={() => { setTaskGoal("/"); setMentionPaletteOpen(false); setAppPaletteOpen(false); setSlashPaletteOpen(true); window.requestAnimationFrame(() => goalInputRef.current?.focus()); }} />
                <PermissionSelector
                  value={newChatRunPreferences.approval_mode}
                  permissionProfile={newChatRunPreferences.permission_profile}
                  permissionProfiles={codexPermissionProfiles}
                  advanced={advanced}
                  onChange={(approval_mode) => setNewChatRunPreferences((current) => ({ ...current, approval_mode, approval_policy: undefined, sandbox_mode: undefined, permission_profile: undefined, additional_writable_roots: approval_mode === "read_only" ? [] : current.additional_writable_roots }))}
                  onPermissionProfileChange={(permission_profile) => setNewChatRunPreferences((current) => ({ ...current, permission_profile, approval_mode: "ask", approval_policy: undefined, sandbox_mode: undefined }))}
                  hasProject={Boolean(selectedProject)}
                />
                <span className="composer-action-spacer" />
                <ChatRunSelector providers={providers} codexModels={codexModels} codexConfigurationStatus={codexConfigurationStatus} codexFeatures={codexFeatures} advanced={advanced} value={newChatRunPreferences} onChange={setNewChatRunPreferences} discovering={modelDiscoveryBusy} discoveryError={modelDiscoveryError} onRefresh={() => void refreshCodexCatalog()} label="New chat model, effort, speed, and team" includeTeam hasProject={Boolean(selectedProject)} />
                <button className="primary new-chat-send" type="button" aria-label="Send" title="Send message" disabled={taskBusy || !taskGoal.trim()} onClick={() => void addTask()}>
                  {taskBusy ? <span aria-hidden="true">…</span> : <ArrowUp aria-hidden="true" />}
                </button>
              </div>
              {taskError && <p className="error-message" role="alert">{taskError}</p>}
              {projectError && projectErrorSurface === "task" && <p className="error-message" role="alert">{projectError}</p>}
            </div>
          </div>
        </section>}

        {(advanced || activeSurface === "threads" || (activeSurface === "tasks" && Boolean(workspaceTask))) && <section className="panel task-panel" hidden={activeSurface !== "threads" && !(activeSurface === "tasks" && Boolean(workspaceTask))} aria-label={activeSurface === "threads" ? "Chat conversation" : "Tasks"}>
          {activeSurface === "threads" ? <div className="chat-conversation">
            <div className="chat-message-list" ref={chatViewportRef} data-scroll-chain>
              {!visibleConversation || (visibleConversation.messages.length === 0 && visibleConversation.turns.length === 0)
                ? <p className="empty-state">This conversation has no messages yet.</p>
                : visibleConversation.turns.map((turn) => <ConversationTurnTranscript
                  key={turn.id}
                  turn={turn}
                  messages={visibleConversation.messages.filter((message) => message.turn_id === turn.id)}
                  activities={(visibleConversation.activities ?? []).filter((activity) => activity.turn_id === turn.id)}
                  active={activeConversationTurn?.id === turn.id}
                  batches={(visibleConversation.team?.coordination_batches ?? []).filter((batch) => batch.origin_turn_id === turn.id)}
                  team={visibleConversation.team}
                  conversationId={visibleConversation.conversation.id}
                  onView={(view) => setConversationViews((current) => ({ ...current, [view.conversation.id]: view }))}
                  onOpenPreview={openConversationPreview}
                />)}
              {focusedApprovalSource && focusedApprovalConversationMatches && focusedApprovalOrigin && <section className="approval-source-focus" aria-label="Approval source evidence">
                <div className="section-heading compact-heading">
                  <div><strong>Exact approval source</strong><p>{focusedApprovalSource.reason}</p></div>
                  <button className="secondary" type="button" onClick={() => setFocusedApprovalSource(null)}>Close</button>
                </div>
                <dl>
                  <div><dt>Approval</dt><dd><code>{focusedApprovalSource.id}</code> · {focusedApprovalSource.state}</dd></div>
                  {focusedApprovalOrigin.conversation && <div><dt>Conversation</dt><dd><code>{focusedApprovalOrigin.conversation.locator}</code> · hash {focusedApprovalOrigin.conversation.content_hash.digest_hex.slice(0, 12)}…</dd></div>}
                  {focusedApprovalOrigin.turn && <div><dt>Turn</dt><dd><code>{focusedApprovalOrigin.turn.locator}</code> · hash {focusedApprovalOrigin.turn.content_hash.digest_hex.slice(0, 12)}…</dd></div>}
                  {focusedApprovalOrigin.invocation && <div><dt>Invocation</dt><dd><code>{focusedApprovalOrigin.invocation.locator}</code> · hash {focusedApprovalOrigin.invocation.content_hash.digest_hex.slice(0, 12)}…</dd></div>}
                  {focusedApprovalOrigin.agent && <div><dt>Agent</dt><dd>{focusedApprovalAgent ? <><strong>{focusedApprovalAgent.display_name}</strong> · {focusedApprovalAgent.kind.replaceAll("_", " ")} · </> : <span className="warning-text">Exact agent projection unavailable · </span>}<code>{focusedApprovalOrigin.agent.locator}</code> · hash {focusedApprovalOrigin.agent.content_hash.digest_hex.slice(0, 12)}…</dd></div>}
                  {focusedApprovalOrigin.command && <div><dt>Command</dt><dd>{focusedApprovalCommand ? <><strong>{focusedApprovalCommand.command ?? focusedApprovalCommand.kind.replaceAll("_", " ")}</strong> · {focusedApprovalCommand.state} · </> : <span className="warning-text">Exact command projection unavailable · </span>}<code>{focusedApprovalOrigin.command.locator}</code> · hash {focusedApprovalOrigin.command.content_hash.digest_hex.slice(0, 12)}…</dd></div>}
                </dl>
                <p><strong>Resource</strong> <code>{focusedApprovalSource.resource}</code></p>
                {focusedApprovalOrigin.evidence.length > 0 && <ul>{focusedApprovalOrigin.evidence.map((source) => <li key={`${source.source_type}-${source.locator}`}><strong>Evidence</strong> <code>{source.locator}</code> · hash {source.content_hash.digest_hex.slice(0, 12)}…</li>)}</ul>}
                {focusedApprovalOrigin.invocation && <button className="link-button" type="button" disabled={invocationBusy !== null} onClick={() => void openCanonicalInvocationSource(focusedApprovalOrigin.invocation!)}>{invocationBusy === focusedApprovalOrigin.invocation.locator ? "Opening exact invocation…" : "Open exact Runtime invocation →"}</button>}
                <p className="form-note">These are the immutable locators and hashes captured when the request was persisted. Missing compatibility projections stay explicit and are never replaced by a nearby conversation, agent, or command.</p>
              </section>}
              {approvalSourceError && <p className="error-message" role="alert">{approvalSourceError}</p>}
              {activeConversationTurn?.state === "waiting_for_approval" && activeConversationApproval && <section className="chat-permission" aria-label="Codex permission request">
                <strong>Permission needed to continue</strong>
                <p>{activeConversationApproval.reason}</p>
                <code>{activeConversationApproval.resource}</code>
                <div>
                  {activeConversationApproval.origin?.conversation && <button className="secondary" type="button" disabled={approvalSourceBusy !== null} onClick={() => void openApprovalSource(activeConversationApproval.id)}>{approvalSourceBusy === activeConversationApproval.id ? "Opening…" : "View exact source"}</button>}
                  <button className="primary" type="button" disabled={conversationApprovalBusy} onClick={() => void resolveConversationApproval(activeConversationApproval.id, true)}>{conversationApprovalBusy ? "Applying…" : "Allow once"}</button>
                  <button className="secondary" type="button" disabled={conversationApprovalBusy} onClick={() => void resolveConversationApproval(activeConversationApproval.id, false)}>Deny</button>
                </div>
              </section>}
              {indeterminateConversationTurn && <section className="chat-recovery" aria-label="Codex turn recovery">
                <strong>Completion could not be proven</strong>
                <p>Codex accepted this turn before the harness disconnected. MetaCog will not resend it without your choice.</p>
                <div>
                  <button className="primary" type="button" disabled={codexRecoveryBusy} onClick={() => void recoverConversation("resume")}>Resume safely</button>
                  <button className="secondary" type="button" disabled={codexRecoveryBusy} onClick={() => void recoverConversation("retry")}>Retry as a new turn</button>
                  <button className="secondary" type="button" disabled={codexRecoveryBusy} onClick={() => void recoverConversation("fork")}>Fork</button>
                </div>
              </section>}
              {visibleConversation?.turns.filter((turn) => turn.error).map((turn) => <p className="error-message" role="alert" key={`${turn.id}-error`}>{turn.error}</p>)}
              {(visibleConversation?.conversation.pending_input_requests ?? []).map((request) => <section className="planning-question-card" aria-label="Planning questions" key={request.itemId}>
                <div className="planning-question-heading"><ListChecks aria-hidden="true" /><div><strong>Help shape the plan</strong><span>Choose one answer for each question. The recommended choice is listed first.</span></div></div>
                {request.questions.map((question) => {
                  const answerKey = `${request.itemId}:${question.id}`;
                  const selectedAnswer = planningAnswers[answerKey] ?? "";
                  return <fieldset key={question.id}>
                    <legend><span>{question.header}</span>{question.question}</legend>
                    {(question.options ?? []).map((option, index) => <label className={`planning-option${selectedAnswer === option.label ? " selected" : ""}`} key={option.label}>
                      <input type="radio" name={answerKey} value={option.label} checked={selectedAnswer === option.label} onChange={() => setPlanningAnswers((current) => ({ ...current, [answerKey]: option.label }))} />
                      <span><strong>{option.label}{index === 0 && <em>Recommended</em>}</strong>{option.description && <small>{option.description}</small>}</span>
                    </label>)}
                    {question.isOther && <label className="planning-other"><span>Other answer</span><input type={question.isSecret ? "password" : "text"} value={(question.options ?? []).some((option) => option.label === selectedAnswer) ? "" : selectedAnswer} onChange={(event) => setPlanningAnswers((current) => ({ ...current, [answerKey]: event.target.value }))} /></label>}
                  </fieldset>;
                })}
                <div className="planning-question-actions"><button className="primary" type="button" onClick={() => void submitPlanningAnswers(request)}>Continue planning</button></div>
              </section>)}
            </div>
            <div className={`chat-composer-stack${visibleQueuedMessages.length > 0 ? " has-queue" : ""}`}>
              {(conversationGoal || visibleConversation?.conversation.mode === "plan") && <section className="conversation-control-strip" aria-label="Conversation controls">
                {conversationGoal && <div className="conversation-goal-row">
                  <Target aria-hidden="true" />
                  {goalEditOpen ? <div className="conversation-goal-edit"><input aria-label="Goal objective" value={goalDraft} onChange={(event) => setGoalDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && goalDraft.trim()) void changeConversationGoal(goalDraft.trim()); }} /><button type="button" aria-label="Save goal" title="Save goal" disabled={!goalDraft.trim()} onClick={() => void changeConversationGoal(goalDraft.trim())}><Check aria-hidden="true" /></button><button type="button" aria-label="Cancel goal edit" title="Cancel" onClick={() => setGoalEditOpen(false)}><X aria-hidden="true" /></button></div> : <div className="conversation-goal-copy"><strong>Goal</strong><span title={conversationGoal.objective}>{conversationGoal.objective}</span><small>{conversationGoal.status.replaceAll(/([A-Z])/g, " $1").toLowerCase()} · {formatAgentElapsed(conversationGoalElapsed)}</small></div>}
                  {!goalEditOpen && <div className="conversation-goal-actions">
                    <button type="button" aria-label={conversationGoal.status === "paused" ? "Resume goal" : "Pause goal"} title={conversationGoal.status === "paused" ? "Resume goal" : "Pause after this round"} onClick={() => void changeConversationGoal(undefined, conversationGoal.status === "paused" ? "active" : "paused")}>{conversationGoal.status === "paused" ? <Play aria-hidden="true" /> : <Pause aria-hidden="true" />}</button>
                    <button type="button" aria-label="Edit goal" title="Edit goal" onClick={() => { setGoalDraft(conversationGoal.objective); setGoalEditOpen(true); }}><SquarePen aria-hidden="true" /></button>
                    <button type="button" aria-label="Delete goal" title="Delete goal" onClick={() => void deleteConversationGoal()}><Trash2 aria-hidden="true" /></button>
                  </div>}
                </div>}
                {visibleConversation?.conversation.mode === "plan" && <div className="conversation-mode-chip"><ListChecks aria-hidden="true" /><span><strong>Plan mode</strong><small>Read-only planning with guided questions</small></span><button type="button" aria-label="Exit plan mode" title="Exit plan mode" onClick={() => void changeConversationMode("default")}><X aria-hidden="true" /></button></div>}
              </section>}
              {visibleQueuedMessages.length > 0 && <section className="chat-queue-drawer" aria-label="Queued messages">
                <div className="chat-queue-heading">
                  <span aria-hidden="true">Ⅱ</span>
                  <strong>{conversationIsActive ? `${visibleQueuedMessages.length} queued` : queueDispatchErrorId ? "Queue paused" : "Sending next queued message"}</strong>
                </div>
                {visibleQueuedMessages.map((message) => <div className="chat-queue-row" key={message.id}>
                  <span className="chat-queue-icon" aria-hidden="true">↳</span>
                  <span className="chat-queue-content" title={message.content}>{message.content}</span>
                  {conversationIsActive
                    ? <button className="chat-queue-steer" type="button" disabled={steeringMessageId !== null} onClick={() => void steerMessage(message.content, message.id, message.mentions)}>{steeringMessageId === message.id ? "Steering…" : "↪ Steer"}</button>
                    : queueDispatchErrorId === message.id && <button className="chat-queue-steer" type="button" onClick={() => setQueueDispatchErrorId(null)}>Retry</button>}
                  <button className="chat-queue-remove" type="button" aria-label="Remove queued message" title="Remove" onClick={() => setQueuedChatMessages((current) => current.filter((item) => item.id !== message.id))}>⌫</button>
                </div>)}
              </section>}
              <div className="new-chat-composer-shell active-chat-composer-shell">
                <div className="new-chat-context-bar" aria-label="Chat workspace context">
                  <ActiveChatProjectSelector project={workspaceProject ?? null} />
                  <NewChatWorkspaceSelector />
                  {workspaceProject?.repository.is_git && <NewChatBranchSelector project={workspaceProject} />}
                </div>
                <div className="composer chat-input-composer chat-followup-composer">
                  <label className="visually-hidden" htmlFor="chat-message">Message</label>
                  <textarea id="chat-message" aria-label="Message" value={taskGoal} onChange={(event) => updateComposerValue(event.target.value, event.target.selectionStart)} onKeyDown={handleComposerKeyDown} placeholder="Do anything" />
                  <SlashCommandPalette value={taskGoal} open={slashPaletteOpen} onSelect={chooseSlashCommand} hasProject={Boolean(workspaceProject)} hasConversation={Boolean(selectedChatId)} />
                  <MentionPalette open={mentionPaletteOpen} candidates={mentionCandidates} onSelect={chooseMention} />
                  <AppReferencePalette open={appPaletteOpen} apps={codexStableDiscovery?.apps ?? []} query={appQuery} onSelect={chooseAppReference} />
                  <div className="composer-actions">
                    <NewChatAddMenu project={workspaceProject ?? null} disabled={taskBusy} existingChat onAttachFiles={() => void attachMentionFiles()} onAttachFolder={() => void attachMentionFolder()} onApps={openAppReferences} onPlugins={openPluginMentions} onOpenFolder={() => { startNewChat(); void chooseProject("task"); }} onClearProject={() => startNewChat()} onPlan={() => setTaskGoal((value) => value.startsWith("/plan") ? value : `/plan ${value}`)} onGoal={() => setTaskGoal((value) => value.startsWith("/goal") ? value : `/goal ${value}`)} onCommands={() => { setTaskGoal("/"); setMentionPaletteOpen(false); setAppPaletteOpen(false); setSlashPaletteOpen(true); window.requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>("#chat-message")?.focus()); }} />
                    <PermissionSelector
                      value={newChatRunPreferences.approval_mode}
                      permissionProfile={newChatRunPreferences.permission_profile}
                      permissionProfiles={codexPermissionProfiles}
                      advanced={advanced}
                      onChange={(approval_mode) => setNewChatRunPreferences((current) => ({ ...current, approval_mode, approval_policy: undefined, sandbox_mode: undefined, permission_profile: undefined, additional_writable_roots: approval_mode === "read_only" ? [] : current.additional_writable_roots }))}
                      onPermissionProfileChange={(permission_profile) => setNewChatRunPreferences((current) => ({ ...current, permission_profile, approval_mode: "ask", approval_policy: undefined, sandbox_mode: undefined }))}
                      hasProject={Boolean(workspaceProject)}
                    />
                    <span className="composer-action-spacer" />
                    <ChatRunSelector providers={providers} codexModels={codexModels} codexConfigurationStatus={codexConfigurationStatus} codexFeatures={codexFeatures} advanced={advanced} value={newChatRunPreferences} onChange={setNewChatRunPreferences} discovering={modelDiscoveryBusy} discoveryError={modelDiscoveryError} onRefresh={() => void refreshCodexCatalog()} label="Chat model, effort, speed, and team" includeTeam hasProject={Boolean(workspaceProject)} />
                    <button className="primary new-chat-send" type="button" aria-label={conversationIsActive && !parseSlashCommand(taskGoal) ? "Queue" : "Send"} disabled={!taskGoal.trim() || steeringMessageId !== null} title={conversationIsActive && !parseSlashCommand(taskGoal) ? "Queue message (Ctrl+Enter steers now)" : "Run command or send message"} onClick={() => conversationIsActive && !parseSlashCommand(taskGoal) ? queueDraftMessage() : void addTask()}><ArrowUp aria-hidden="true" /></button>
                  </div>
                  {taskError && <p className="error-message" role="alert">{taskError}</p>}
                </div>
              </div>
            </div>
          </div> : <>
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
                    <small>Affects: {approvals.find((approval) => approval.task_id === task.id)?.resource}</small>
                    {advanced && <TechnicalDetails title="Permission evidence" summary="Exact governed resource"><code>{approvals.find((approval) => approval.task_id === task.id)?.resource}</code></TechnicalDetails>}
                  </div>}
                  {taskEvidence(task)}
                </div>
                <div className="task-actions">
                  <ChatRunSelector
                    providers={providers}
                    codexModels={codexModels}
                    codexFeatures={codexFeatures}
                    advanced={advanced}
                    value={taskRunPreferences[task.id] ?? newChatRunPreferences}
                    onChange={(preferences) => setTaskRunPreferences((current) => ({ ...current, [task.id]: preferences }))}
                    discovering={modelDiscoveryBusy}
                    discoveryError={modelDiscoveryError}
                    onRefresh={() => void refreshCodexCatalog()}
                    label={`Model, effort, and speed for ${task.goal}`}
                    hasProject={Boolean(task.project_id)}
                  />
                  {task.state === "pending" && <TeamPresetSelector value={(taskRunPreferences[task.id] ?? newChatRunPreferences).team_preset} onChange={(team_preset) => setTaskRunPreferences((current) => ({ ...current, [task.id]: { ...(current[task.id] ?? newChatRunPreferences), team_preset } }))} />}
                  {advanced && task.state === "pending" && <label className="task-team-select"><span>Custom organization</span><select aria-label={`Custom organization for ${task.goal}`} value={taskOrganizationIds[task.id] ?? ""} onChange={(event) => setTaskOrganizationIds((current) => ({ ...current, [task.id]: event.target.value }))}>
                    <option value="">Automatic team</option>
                    {organizations.map((organization) => <option value={organization.organization_id} key={`${task.id}-${organization.organization_id}`}>{organization.name}</option>)}
                  </select></label>}
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
          </>}
        </section>}

        <section className="panel pull-request-panel" id="pull-requests" aria-label="Pull requests">
          <><div className="pull-request-workspace">
            <section className="pull-request-list-pane" aria-label="Pull request list">
              <div className="pull-request-tabs" role="tablist" aria-label="Pull request views">
                {([['all', 'All'], ['reviewing', 'Reviewing'], ['authored', 'Authored']] as const).map(([value, label]) => <button
                  className={codeReviewTab === value ? "active" : ""}
                  type="button"
                  role="tab"
                  aria-selected={codeReviewTab === value}
                  onClick={() => setCodeReviewTab(value)}
                  key={value}
                >{label}</button>)}
              </div>
              <div className="pull-request-primary-actions" aria-label="Code change actions">
                <button type="button" onClick={openLocalReviewSetup}><GitPullRequest aria-hidden="true" />New review</button>
                <button type="button" onClick={openCloudApplySetup}><CloudDownload aria-hidden="true" />Apply cloud task</button>
              </div>
              <div className="pull-request-search-row">
                <label className="pull-request-search"><span aria-hidden="true">⌕</span><span className="sr-only">Search pull requests</span><input type="search" aria-label="Search pull requests" value={codeReviewSearch} onChange={(event) => setCodeReviewSearch(event.target.value)} placeholder="Search pull requests" /></label>
                <div className="pull-request-filter-menu" data-transient-popover-root="review-filter">
                  <button className={`pull-request-filter${codeReviewFilterOpen || codeReviewStatusFilter !== "all" ? " active" : ""}`} type="button" aria-label="Filter pull requests" title="Filter" aria-expanded={codeReviewFilterOpen} onClick={() => setCodeReviewFilterOpen((current) => !current)}><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 5h16l-6.2 7.1v5.2l-3.6 1.8v-7z" /></svg></button>
                  {codeReviewFilterOpen && <div className="pull-request-filter-popover" role="menu" aria-label="Pull request filters"><strong>Review state</strong>{([['all', 'All'], ['open', 'Open'], ['finished', 'Finished']] as const).map(([value, label]) => <button type="button" role="menuitemradio" aria-checked={codeReviewStatusFilter === value} onClick={() => { setCodeReviewStatusFilter(value); setCodeReviewFilterOpen(false); }} key={value}><span aria-hidden="true">{codeReviewStatusFilter === value ? "●" : "○"}</span>{label}</button>)}<hr /><button type="button" role="menuitem" onClick={() => { setCodeReviewFilterOpen(false); openLocalReviewSetup(); }}><GitPullRequest aria-hidden="true" />New local review</button><button type="button" role="menuitem" onClick={() => { setCodeReviewFilterOpen(false); openCloudApplySetup(); }}><CloudDownload aria-hidden="true" />Apply cloud task</button></div>}
                </div>
              </div>
              <div className="pull-request-group-heading"><span>{codeReviewTab === "reviewing" ? "Reviewing" : codeReviewTab === "authored" ? "Authored locally" : "Local reviews"}</span><span>{filteredCodeReviewTasks.length}</span></div>
              <div className="pull-request-list">
                {filteredCodeReviewTasks.length === 0 ? <div className="pull-request-list-empty"><strong>{codeReviewSearch.trim() ? "No matching reviews" : codeReviewTab === "reviewing" ? "No reviews in progress" : "No local reviews yet"}</strong><span>{codeReviewSearch.trim() ? "Try another search." : "Start a read-only review of a Git project."}</span><button type="button" onClick={openLocalReviewSetup}>Start local review</button></div> : filteredCodeReviewTasks.map((task) => {
                  const project = projects.find((candidate) => candidate.project_id === task.project_id);
                  const projectName = project?.root.split(/[\\/]/).filter(Boolean).at(-1) ?? "Repository unavailable";
                  return <button className={`pull-request-row${selectedCodeReviewTaskId === task.id ? " selected" : ""}`} type="button" aria-pressed={selectedCodeReviewTaskId === task.id} onClick={() => { setSelectedCodeReviewTaskId(task.id); setCodeReviewSetupOpen(false); setCloudApplySetupOpen(false); }} key={task.id}>
                    <GitPullRequest className="pull-request-state-icon" data-state={task.state} aria-hidden="true" />
                    <span className="pull-request-row-copy"><strong>{task.goal.replace(CODE_REVIEW_GOAL_PREFIX, "").trim() || "Code review"}</strong><small>{projectName} · {task.state.replaceAll("_", " ")}</small></span>
                    <time dateTime={task.updated_at}>{new Date(task.updated_at).toLocaleDateString(undefined, { month: "short", day: "numeric" })}</time>
                  </button>;
                })}
              </div>
              <p className="pull-request-sync-note">GitHub titles, comments, checks, and changed files will appear after GitHub is connected. Local Git review works now.</p>
            </section>
            <section className="pull-request-detail-pane" aria-label="Pull request details">
              {cloudApplySetupOpen ? <div className="pull-request-review-setup cloud-apply-setup">
                <div className="pull-request-detail-heading"><span className="pull-request-detail-icon" aria-hidden="true"><CloudDownload /></span><div><h2>Apply a Codex cloud task</h2><p>Inspect the latest cloud diff in isolation, then choose whether to apply it.</p></div></div>
                <div className="code-review-start-card cloud-apply-start-card">
                  <span className="cloud-apply-note"><ShieldCheck aria-hidden="true" />Your project is unchanged while MetaCog prepares and validates the diff.</span>
                  <label>
                    <span>Repository</span>
                    <select value={selectedProject?.repository.is_git ? selectedProject.project_id : ""} disabled={cloudApplyBusy || cloudApplyCandidate?.state === "ready"} onChange={(event) => {
                      const project = projects.find((candidate) => candidate.project_id === event.target.value) ?? null;
                      setSelectedProject(project);
                      setCloudApplyCandidate(null);
                      setCloudApplyCandidates([]);
                      setCloudApplyError(null);
                      setCloudApplyTaskId("");
                      if (project?.repository.is_git) void loadCloudApplyCandidates(project.project_id);
                    }}>
                      <option value="">Choose a Git project</option>
                      {projects.filter((project) => project.repository.is_git).map((project) => <option key={project.project_id} value={project.project_id}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                    </select>
                  </label>
                  {cloudApplyCandidates.length > 0 && <label>
                    <span>Prepared changes</span>
                    <select value={cloudApplyCandidate?.id ?? ""} disabled={cloudApplyBusy} onChange={(event) => {
                      const candidate = cloudApplyCandidates.find((item) => item.id === event.target.value) ?? null;
                      setCloudApplyCandidate(candidate);
                      setCloudApplyTaskId(candidate?.task_id ?? "");
                      setCloudApplyError(null);
                    }}>
                      <option value="">Start another cloud task</option>
                      {cloudApplyCandidates.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.task_id} · {candidate.changed_paths.length} files</option>)}
                    </select>
                  </label>}
                  <label>
                    <span>Codex cloud task ID</span>
                    <input value={cloudApplyTaskId} disabled={cloudApplyBusy || cloudApplyCandidate?.state === "ready"} onChange={(event) => { setCloudApplyTaskId(event.target.value); setCloudApplyCandidate(null); setCloudApplyError(null); }} placeholder="Task ID" autoComplete="off" spellCheck={false} />
                  </label>
                  {!cloudApplyCandidate && <button className="primary" type="button" disabled={cloudApplyBusy || !selectedProject?.repository.is_git || !cloudApplyTaskId.trim()} onClick={() => void prepareCodexCloudApply()}>{cloudApplyOperation === "loading" ? "Loading…" : cloudApplyOperation === "preparing" ? "Checking…" : "Check changes"}</button>}
                  {cloudApplyCandidate && <div className="cloud-apply-candidate" aria-live="polite">
                    <div className="cloud-apply-candidate-heading"><span className={`task-state state-${cloudApplyCandidate.state}`}>{cloudApplyCandidate.state}</span><strong>{cloudApplyCandidate.changed_paths.length} changed {cloudApplyCandidate.changed_paths.length === 1 ? "file" : "files"}</strong></div>
                    <div className="cloud-apply-files" data-scroll-chain tabIndex={0} aria-label="Cloud task changed files">{cloudApplyCandidate.changed_paths.map((path) => <code key={path}><File aria-hidden="true" />{path}</code>)}</div>
                    {cloudApplyCandidate.output && <TechnicalDetails title="Codex output" summary="Cloud apply preparation result"><pre>{cloudApplyCandidate.output}</pre></TechnicalDetails>}
                    {cloudApplyCandidate.state === "ready" && <div className="cloud-apply-confirmation"><p>MetaCog will apply this exact validated diff to the current working tree. If the repository changed since preparation, it will stop without overwriting those changes.</p><div><button className="secondary danger-subtle" type="button" disabled={cloudApplyBusy} onClick={() => void discardCodexCloudApply()}><Trash2 aria-hidden="true" />{cloudApplyOperation === "discarding" ? "Discarding…" : "Discard prepared changes"}</button><button className="primary" type="button" disabled={cloudApplyBusy} onClick={() => void confirmCodexCloudApply()}>{cloudApplyOperation === "applying" ? "Applying…" : "Apply changes"}</button></div></div>}
                    {cloudApplyCandidate.state === "applied" && <p className="cloud-apply-success" role="status"><Check aria-hidden="true" />Changes applied to the working tree.</p>}
                  </div>}
                  {cloudApplyError && <p className="error-message" role="alert">{cloudApplyError}</p>}
                </div>
              </div> : codeReviewSetupOpen ? <div className="pull-request-review-setup">
                <div className="pull-request-detail-heading"><span className="pull-request-detail-icon" aria-hidden="true"><GitPullRequest /></span><div><h2>Start a local review</h2><p>Review code without changing the repository.</p></div></div>
                <div className="code-review-start-card">
                  <span className="read-only-note">Review is read-only. It will not modify files.</span>
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
                      <option value="branch">Branch changes</option><option value="uncommitted">Uncommitted changes</option><option value="commit">Current commit</option><option value="custom">Custom review</option>
                    </select>
                  </label>
                  {codeReviewScope === "branch" && <label><span>Base branch</span><input value={codeReviewBase} onChange={(event) => setCodeReviewBase(event.target.value)} placeholder="main" /></label>}
                  {codeReviewScope === "custom" && <label className="code-review-custom"><span>What should be reviewed?</span><textarea value={codeReviewInstructions} onChange={(event) => setCodeReviewInstructions(event.target.value)} placeholder="Describe the range and any review focus" /></label>}
                  <button className="primary" type="button" disabled={!selectedProject?.repository.is_git} onClick={startCodeReview}>Start review</button>
                  {codeReviewError && <p className="error-message" role="alert">{codeReviewError}</p>}
                </div>
              </div> : selectedCodeReviewTask ? <article className="pull-request-detail-card">
                <header><span className="pull-request-detail-icon" aria-hidden="true"><GitPullRequest /></span><div><span className={`task-state state-${selectedCodeReviewTask.state}`}>{selectedCodeReviewTask.state.replaceAll("_", " ")}</span><h2>{selectedCodeReviewTask.goal.replace(CODE_REVIEW_GOAL_PREFIX, "").trim() || "Code review"}</h2><p>{projects.find((project) => project.project_id === selectedCodeReviewTask.project_id)?.root ?? "Repository unavailable"}</p></div></header>
                <div className="pull-request-detail-summary"><div><span>Review type</span><strong>Local, read-only</strong></div><div><span>Updated</span><strong>{new Date(selectedCodeReviewTask.updated_at).toLocaleString()}</strong></div></div>
                <p>The review findings and discussion live in the associated MetaCog chat.</p>
                <button className="primary" type="button" onClick={() => openChat(selectedCodeReviewTask.id)}>Open review</button>
              </article> : <div className="pull-request-detail-empty"><GitPullRequest aria-hidden="true" /><strong>Select a review to view</strong><p>Choose a local review, start a new one, or apply changes from a Codex cloud task.</p><div><button type="button" onClick={openLocalReviewSetup}>New review</button><button type="button" onClick={openCloudApplySetup}>Apply cloud task</button></div></div>}
            </section>
          </div>{advanced && <TechnicalDetails title="Technical review controls" summary="Runtime approvals and legacy review administration">
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
            {approvalSourceError && <p className="error-message" role="alert">{approvalSourceError}</p>}
            {tasks.filter((task) => task.state === "waiting_for_approval").length === 0 ? <p className="empty-state">No runtime approvals need attention.</p> : <div className="task-list">{tasks.filter((task) => task.state === "waiting_for_approval").map((task) => {
              const approval = approvals.find((candidate) => candidate.task_id === task.id && candidate.state === "pending");
              return <article className="task-card" key={task.id}>
                <div className="task-card-main"><span className="task-state state-waiting_for_approval">permission</span><h3>{task.goal}</h3><small>{projects.find((project) => project.project_id === task.project_id)?.root ?? "General workspace"}</small></div>
                <div className="card-actions">
                  {approval?.origin?.conversation && <button className="secondary" type="button" disabled={approvalSourceBusy !== null} onClick={() => void openApprovalSource(approval.id)}>{approvalSourceBusy === approval.id ? "Opening…" : "Open exact source"}</button>}
                  <button className="secondary" type="button" onClick={() => openChat(task.id)}>Open chat</button>
                </div>
              </article>;
            })}</div>}
          </div>}
          <p className="integration-note">GitHub pull request titles, comments, checks, and changed-file sync will appear here after the GitHub integration is connected. Local Git review works now.</p>
          </TechnicalDetails>}
          </>
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
                    <div className="project-tags">
                      {project.build_systems.map((system) => <span key={`build-${system}`}>{system}</span>)}
                      {project.test_systems.map((system) => <span key={`test-${system}`}>{system}</span>)}
                      {project.repository.is_git && <span>{project.repository.dirty ? "Git · changes" : "Git · clean"}</span>}
                    </div>
                    {project.repository.is_git && <small>{project.repository.branch ? `Branch ${project.repository.branch}` : "Git repository"}{project.instructions.conflicts.length > 0 ? ` · ${project.instructions.conflicts.length} instruction conflict${project.instructions.conflicts.length === 1 ? "" : "s"}` : ""}</small>}
                    {advanced && <TechnicalDetails title="Project identity" summary="Filesystem path and detected instructions"><code>{project.root}</code><small>{project.languages.length > 0 ? project.languages.join(", ") : "Language not detected"}</small></TechnicalDetails>}
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
              {topologyPlanError ? <p className="error-message" role="alert">The topology plan could not be loaded: {topologyPlanError}</p> : !topologyPlan ? <p className="empty-state">Loading the current topology plan…</p> : topologyPlan.status === "empty" ? <p className="empty-state">No explicit deployment dependencies were discovered.</p> : topologyPlan.status === "blocked_by_cycle" ? <div className="error-message" role="alert">Dependency cycle detected. MetaCog will not present partial execution stages.{advanced && topologyPlan.unresolved_nodes.length > 0 && <small>{topologyPlan.unresolved_nodes.map((node) => node.label).join(", ")}</small>}</div> : <ol className="topology-plan" aria-label="Project topology plan">
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
              </div> : <p className="empty-state">A ready explicit topology and a manifest-discovered command are required before MetaCog can create an execution plan.</p>}
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
                  <h3>What MetaCog remembers</h3>
                  <p className="muted">Important project facts stay local, versioned, and tied to their source.</p>
                </div>
                <span className="count-badge">{memoryClaims.length} current</span>
              </div>
              <div className="memory-composer">
                {advanced && <label>Namespace<input aria-label="Memory namespace" value={memoryNamespace} onChange={(event) => setMemoryNamespace(event.target.value)} /></label>}
                <label>Fact name<input aria-label="Memory fact name" value={memoryKey} onChange={(event) => setMemoryKey(event.target.value)} placeholder="package manager" /></label>
                <label>What should MetaCog remember?<textarea aria-label="Memory value" value={memoryDraft} onChange={(event) => setMemoryDraft(event.target.value)} placeholder="This project uses pnpm." /></label>
                {advanced && <label>Confidence<input aria-label="Memory confidence" type="number" min="0" max="1" step="0.05" value={memoryConfidence} onChange={(event) => setMemoryConfidence(event.target.value)} /></label>}
                {advanced && <label>Fresh until (optional)<input aria-label="Memory fresh until" type="datetime-local" value={memoryFreshUntil} onChange={(event) => setMemoryFreshUntil(event.target.value)} /><small>After this time, MetaCog automatically marks the fact stale and retains its history.</small></label>}
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
                <p className="muted">MetaCog will copy this exact fact once. Later changes stay separate in each project.</p>
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
                  <div><dt>Protected Root</dt><dd>{protectedRoot ? `IPC v${protectedRoot.protocol_version} · schema ${protectedRoot.storage_schema_version}` : "Checking protected IPC…"}</dd></div>
                  <div><dt>Startup</dt><dd>{localService?.detail ?? "Checking packaged service…"}</dd></div>
                  <div><dt>Autonomy</dt><dd>{governorOverview?.autonomy ?? userSettings?.autonomy ?? "Loading"}</dd></div>
                  <div><dt>Organization</dt><dd>{governorOverview?.active_organization.name ?? "Loading"}</dd></div>
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

        {activeSurface === "agents" && <section className="panel agents-panel" id="agents" aria-label="Agents">
          <div className="agents-heading">
            <div><p className="eyebrow">GOVERNED AGENTS</p><h2>Agents</h2><p className="muted">See how MetaCog governs teams, assignments, and live work.</p></div>
            <button className="secondary" type="button" disabled={agentBusy} onClick={() => void refreshAgents()}>{agentBusy ? "Refreshing…" : "Refresh"}</button>
          </div>
          <nav className="segmented-tabs agents-tabs" aria-label="Agents pages">
            {(["overview", "teams", "activity"] as const).map((tab) => <button type="button" className={agentTab === tab ? "active" : ""} aria-current={agentTab === tab ? "page" : undefined} onClick={() => setAgentTab(tab)} key={tab}>{tab.charAt(0).toUpperCase() + tab.slice(1)}</button>)}
            {advanced && <button type="button" className={agentTab === "governor" ? "active" : ""} aria-current={agentTab === "governor" ? "page" : undefined} onClick={() => setAgentTab("governor")}>Governor</button>}
            {advanced && <button type="button" onClick={() => navigateSurface("improvement")}>Improvement</button>}
          </nav>
          {agentError && <p className="error-message" role="alert">{agentError}</p>}

          {agentTab === "overview" && <div className="agents-stack">
            <article className="agent-governor-card">
              <div className="governor-mark" aria-hidden="true"><img src={metaCogLogo} alt="" /></div>
              <div className="agent-card-copy"><span className="agent-kicker">DETERMINISTIC RUNTIME · NOT A MODEL AGENT</span><h3>MetaCog runtime</h3><p>Enforces owner policy above the Chat, Coordinator, and Coding Agents. It brokers tools, permissions, budgets, leases, memory, and evidence. The metacognitive-governor model role remains dormant.</p></div>
              <div className="agent-status-summary"><span className={`agent-state state-${governorOverview?.status.state ?? "starting"}`}>{governorOverview?.status.state ?? "starting"}</span><strong>{governorOverview?.active_runs ?? 0}</strong><small>active runs</small></div>
              <details className="governor-details">
                <summary>Subsystem status</summary>
                <div className="subsystem-grid">
                  {governorOverview?.subsystems.map((subsystem) => <div className="subsystem-row" key={subsystem.id}><span className={`health-dot health-${subsystem.state}`} aria-hidden="true" /><span><strong>{subsystem.label}</strong><small>{subsystem.detail}</small></span><em>{subsystem.state}</em></div>) ?? <p className="muted">Loading runtime status…</p>}
                  {codexHarnessStatus && <>
                    <div className="subsystem-row"><span className={`health-dot health-${codexHarnessStatus.process_state === "ready" && codexHarnessStatus.initialized ? "healthy" : "degraded"}`} aria-hidden="true" /><span><strong>Codex app-server</strong><small>Bundled {codexHarnessStatus.bundled_version} · protocol {codexHarnessStatus.protocol_version} · schema {codexHarnessStatus.schema_sha256.slice(0, 10)}</small></span><em>{codexHarnessStatus.process_state}</em></div>
                    <div className="subsystem-row"><span className={`health-dot health-${codexHarnessStatus.account_state === "authenticated" ? "healthy" : "degraded"}`} aria-hidden="true" /><span><strong>Shared Codex account</strong><small>{codexHarnessStatus.active_threads} active owned threads · {codexHarnessStatus.pending_approvals} approvals</small></span><em>{codexHarnessStatus.account_state}</em></div>
                    <div className="subsystem-row"><span className={`health-dot health-${codexHarnessStatus.unsupported_requests === 0 ? "healthy" : "unhealthy"}`} aria-hidden="true" /><span><strong>Protocol requests</strong><small>{codexPendingRequests.length} pending · {codexHarnessStatus.unsupported_requests} unsupported{codexHarnessStatus.last_unsupported_request ? ` · ${codexHarnessStatus.last_unsupported_request}` : ""}</small></span><em>{codexHarnessStatus.unsupported_requests === 0 ? "compatible" : "attention"}</em></div>
                    <div className="subsystem-row"><span className={`health-dot health-${codexConfigurationStatus ? "healthy" : "degraded"}`} aria-hidden="true" /><span><strong>Configuration and sandbox</strong><small>{codexHarnessStatus.configuration_state} · Windows sandbox {codexHarnessStatus.windows_sandbox_state}</small></span><em>{codexConfigurationStatus ? "loaded" : "checking"}</em></div>
                    <div className="subsystem-row"><span className={`health-dot health-${codexHarnessStatus.recovery_state === "healthy" ? "healthy" : "degraded"}`} aria-hidden="true" /><span><strong>MCP bridge and recovery</strong><small>MCP {codexHarnessStatus.mcp_bridge_state} · {codexHarnessStatus.last_notification ?? codexHarnessStatus.last_event ?? "No events yet"}</small></span><em>{codexHarnessStatus.recovery_state}</em></div>
                    <div className="subsystem-row"><span className={`health-dot health-${codexStableDiscovery ? "healthy" : "degraded"}`} aria-hidden="true" /><span><strong>Stable Codex capabilities</strong><small>{codexStableDiscovery ? `${codexStableDiscovery.skills.length} skills · ${discoveredHookCount} hooks · ${codexStableDiscovery.apps.length} apps` : "Discovery unavailable"}{codexStableDiscovery?.appsUnavailableReason ? ` · ${codexStableDiscovery.appsUnavailableReason}` : ""}</small></span><em>{codexStableDiscovery ? "loaded" : "checking"}</em></div>
                    <div className="subsystem-row"><span className="health-dot health-healthy" aria-hidden="true" /><span><strong>Harness lifecycle</strong><small>{codexHarnessStatus.last_unknown_notification ? `Unknown notification retained: ${codexHarnessStatus.last_unknown_notification}` : "Unknown notifications are retained for diagnostics"}</small></span><em>{codexHarnessStatus.restart_count} starts</em></div>
                  </>}
                </div>
              </details>
            </article>

            <article className="agent-section-card global-agent-session-section" aria-label="Compatibility agent sessions">
              <div className="section-heading compact-heading">
                <div><h3>Compatibility agent sessions</h3><p className="muted">Browse conversation agents while canonical workflow attachment migration proceeds.</p></div>
                <span className="count-badge">{visibleGlobalAgentSessions.length}</span>
              </div>
              <label className="compact-search"><span className="sr-only">Search global agent sessions</span><input type="search" aria-label="Search global agent sessions" value={globalAgentSearch} onChange={(event) => setGlobalAgentSearch(event.target.value)} placeholder="Search agents or conversations" /></label>
              {visibleGlobalAgentSessions.length === 0 ? <p className="empty-state">{globalAgentSessions.length === 0 ? "No governed agent sessions yet." : "No agent sessions match this search."}</p> : <div className="instance-list">
                {visibleGlobalAgentSessions.map((session) => <div className="instance-row global-agent-session-row" key={session.session_id} data-agent-state={session.state}>
                  <span>
                    <strong>{session.display_name}</strong>
                    <small>{session.conversation_title} · {session.kind.replaceAll("_", " ")}</small>
                    {advanced && <small>Session {session.session_id} · updated {new Date(session.updated_at).toLocaleString()}{session.model ? ` · ${session.model}` : ""}</small>}
                    {userSettings?.interface_complexity === "developer" && <small>Conversation {session.conversation_id} · role {session.role_id} · organization revision {session.organization_revision_id}{session.codex_thread_id ? ` · Codex thread ${session.codex_thread_id}` : ""}</small>}
                  </span>
                  <div className="card-actions"><em className={`agent-state state-${session.state}`}>{session.state}</em><button className="secondary" type="button" onClick={() => openChat(session.conversation_id)}>Open conversation</button></div>
                </div>)}
              </div>}
            </article>

            <article className="agent-section-card canonical-workflow-section" aria-label="Governed workflow activity">
              <div className="section-heading compact-heading">
                <div><h3>Governed workflow activity</h3><p className="muted">Drill from each Root-owned workflow into its exact current-attempt Work and Verification invocations.</p></div>
                <button className="secondary" type="button" disabled={workflowBusy !== null} onClick={() => void refreshCanonicalWorkflows()}>{workflowBusy === "list" ? "Refreshing…" : "Refresh"}</button>
              </div>
              {workflowError && <p className="error-message" role="alert">{workflowError}</p>}
              {canonicalWorkflows.length === 0 ? <p className="empty-state">No canonical workflows yet. Existing compatibility work remains available in its current views while migration proceeds.</p> : <div className="instance-list">
                  {canonicalWorkflows.map((attempt) => {
                    const inspection = workflowInspections[attempt.workflow_id];
                    const activity = workflowActivities[attempt.workflow_id];
                  return <div className="instance-row canonical-workflow-row" key={attempt.workflow_id} data-workflow-state={workflowStateKey(attempt.state)}>
                    <span>
                      <strong>{inspection?.task_contract.objective ?? `Workflow ${attempt.workflow_id.slice(0, 8)}`}</strong>
                      <small>{workflowStateLabel(attempt.state)}{advanced ? ` · attempt ${attempt.attempt_number} · revision ${attempt.revision} · generation ${attempt.lease_generation}` : ""}</small>
                      {advanced && typeof attempt.state === "object" && <small>Wait reason: {attempt.state.waiting.replaceAll("_", " ")}</small>}
                      {advanced && attempt.lease_owner && <small>Lease owner <code>{attempt.lease_owner}</code>{attempt.lease_expires_at ? ` until ${new Date(attempt.lease_expires_at).toLocaleString()}` : ""}</small>}
                    </span>
                    <div className="memory-actions">
                        {advanced && <button className="secondary" type="button" disabled={workflowBusy !== null} onClick={() => void inspectCanonicalWorkflowActivity(attempt.workflow_id)}>{workflowBusy === attempt.workflow_id ? "Loading…" : activity ? "Refresh activity" : "Inspect activity"}</button>}
                      {workflowCanRequestCancellation(attempt.state) && <button className="secondary" type="button" disabled={workflowBusy !== null} onClick={() => void requestCanonicalWorkflowCancellation(attempt)}>Request cancellation</button>}
                    </div>
                      {advanced && inspection && <div className="workflow-inspection">
                        <p><strong>Contract v{inspection.task_contract.version}</strong> · {inspection.task_contract.acceptance_criteria.length} acceptance criteria · {inspection.events.length} ordered events</p>
                        {activity && <div className="instance-list" aria-label={`Workflow ${attempt.workflow_id} invocation activity`}>
                          {activity.invocations.length === 0 ? <p className="empty-state">No Runtime invocations are attached to this current attempt.</p> : activity.invocations.map((invocation) => <div className="instance-row canonical-invocation-row" key={invocation.invocation_id} data-invocation-state={invocation.model_run_state ?? "unplanned"}>
                            <span><strong>{invocation.role === "work" ? "Work" : "Verification"}</strong><small>{invocationStateLabel(invocation.model_run_state)}{userSettings?.interface_complexity === "developer" ? ` · ${invocation.invocation_id}` : ""}</small></span>
                            <button className="secondary" type="button" disabled={invocationBusy !== null} onClick={() => void inspectCanonicalInvocation(invocation.invocation_id)}>Inspect invocation</button>
                          </div>)}
                          {activity.invocations_truncated && <p className="warning-box">Only the bounded current-attempt invocation window is shown.</p>}
                        </div>}
                      {inspection.task_contract.acceptance_criteria.length > 0 && <ul>{inspection.task_contract.acceptance_criteria.map((criterion) => <li key={criterion.clause_id}>{criterion.statement}</li>)}</ul>}
                        {userSettings?.interface_complexity === "developer" && <TechnicalDetails title="Canonical workflow activity evidence" summary={`${inspection.events.length} Root-owned events · ${activity?.invocations.length ?? 0} invocations`}>
                        <p className="form-note">Read-only canonical evidence. Correlation, causation, revision, generation, and idempotency identities are disclosed without exposing Root mutation authority.</p>
                          <pre aria-label={`Canonical workflow ${attempt.workflow_id} evidence`} aria-readonly="true" role="textbox" tabIndex={0}>{JSON.stringify(activity ?? inspection, null, 2)}</pre>
                      </TechnicalDetails>}
                    </div>}
                  </div>;
                })}
              </div>}
            </article>

            <article className="agent-section-card canonical-invocation-section" aria-label="Runtime invocations">
              <div className="section-heading compact-heading">
                <div><h3>Runtime invocations</h3><p className="muted">Work and independent verification use reconstructable evidence owned by Protected Root.</p></div>
                <button className="secondary" type="button" disabled={invocationBusy !== null} onClick={() => void refreshCanonicalInvocations()}>{invocationBusy === "list" ? "Refreshing…" : "Refresh"}</button>
              </div>
              {invocationError && <p className="error-message" role="alert">{invocationError}</p>}
              {focusedInvocationId && focusedInvocationInspection && <div className="approval-invocation-focus" role="status" aria-label="Focused approval invocation">
                <strong>Exact approval-bound invocation</strong>
                <span><code>{focusedInvocationId}</code> · {focusedInvocationInspection.manifest.role === "work" ? "Work" : "Verification"} · {focusedInvocationInspection.manifest.resolved_provider.locator} / {focusedInvocationInspection.manifest.resolved_model.locator}</span>
                <button className="link-button" type="button" onClick={() => setFocusedInvocationId(null)}>Clear focus</button>
              </div>}
              {canonicalInvocations.length === 0 ? <p className="empty-state">No reconstructable Runtime invocations yet.</p> : <div className="instance-list">
                {canonicalInvocations.map((invocation) => {
                  const inspection = invocationInspections[invocation.invocation_id];
                  const latestRun = inspection?.model_runs.at(-1)?.run;
                  return <div className={`instance-row canonical-invocation-row${focusedInvocationId === invocation.invocation_id ? " focused" : ""}`} id={`invocation-${invocation.invocation_id}`} key={invocation.invocation_id} data-invocation-id={invocation.invocation_id} data-invocation-state={invocation.model_run_state ?? "unplanned"} aria-current={focusedInvocationId === invocation.invocation_id ? "true" : undefined}>
                    <span>
                      <strong>{invocation.role === "work" ? "Work run" : "Verification run"}</strong>
                      <small>{invocationStateLabel(invocation.model_run_state)}{advanced ? ` · revision ${invocation.model_run_revision ?? 0} · updated ${new Date(invocation.updated_at).toLocaleString()}` : ""}</small>
                      {advanced && <small>Invocation <code>{invocation.invocation_id}</code> · workflow <code>{invocation.workflow_id}</code></small>}
                    </span>
                    {advanced && <div className="memory-actions">
                      <button className="secondary" type="button" disabled={invocationBusy !== null} onClick={() => void inspectCanonicalInvocation(invocation.invocation_id)}>{invocationBusy === invocation.invocation_id ? "Loading…" : inspection ? "Refresh evidence" : "Inspect"}</button>
                    </div>}
                    {advanced && inspection && <div className="invocation-inspection">
                      <p><strong>{inspection.manifest.resolved_provider.locator}</strong> · {inspection.manifest.resolved_model.locator}{inspection.manifest.observed_model_fingerprint ? ` · ${inspection.manifest.observed_model_fingerprint}` : ""}</p>
                      <p>{inspection.context_manifest.selected_sources.length} governed context source{inspection.context_manifest.selected_sources.length === 1 ? "" : "s"} · approximately {inspection.context_manifest.approximate_tokens.toLocaleString()} tokens · {inspection.model_runs.length} immutable run revision{inspection.model_runs.length === 1 ? "" : "s"}</p>
                      <p>TaskContract v{inspection.manifest.task_contract.version} · request <code>{inspection.manifest.request_hash.digest_hex.slice(0, 12)}…</code>{latestRun?.response_artifact ? <> · response <code>{latestRun.response_artifact.content_hash.digest_hex.slice(0, 12)}…</code></> : null}{latestRun?.observed_model_fingerprint ? <> · observed model <code>{latestRun.observed_model_fingerprint.slice(0, 28)}…</code></> : null}</p>
                      {latestRun?.outcome_evidence ? <p>Provider latency <code>{latestRun.outcome_evidence.latency_milliseconds.toLocaleString()} ms</code> · cost <code>{latestRun.outcome_evidence.actual_cost_microusd === null ? "unknown" : `$${(latestRun.outcome_evidence.actual_cost_microusd / 1_000_000).toFixed(6)}`}</code>{latestRun.outcome_evidence.error_class ? <> · error <code>{latestRun.outcome_evidence.error_class.replaceAll("_", " ")}</code></> : null}</p> : null}
                      {inspection.model_runs_truncated && <p className="warning-box">Only the bounded reconstruction window is shown.</p>}
                      <button className="link-button" type="button" disabled={workflowBusy !== null} onClick={() => void inspectCanonicalWorkflow(invocation.workflow_id)}>Load source workflow evidence →</button>
                      {userSettings?.interface_complexity === "developer" && <TechnicalDetails title="Canonical invocation evidence" summary={`${inspection.model_runs.length} immutable model-run revisions`}>
                        <p className="form-note">Read-only reconstruction evidence. Developer mode exposes exact manifest, context references, hashes, Runtime identity, and lifecycle history without prompt material, response content, secrets, private reasoning, database access, or Root mutation authority.</p>
                        <pre aria-label={`Canonical invocation ${invocation.invocation_id} evidence`} aria-readonly="true" role="textbox" tabIndex={0}>{JSON.stringify(inspection, null, 2)}</pre>
                      </TechnicalDetails>}
                    </div>}
                  </div>;
                })}
              </div>}
            </article>

            <article className="agent-section-card canonical-action-section" aria-label="Governed actions">
              <div className="section-heading compact-heading">
                <div><h3>Governed actions</h3><p className="muted">Protected Root records what was authorized, what was observed, and whether an external change is certain.</p></div>
                <button className="secondary" type="button" disabled={actionBusy !== null} onClick={() => void refreshCanonicalActions()}>{actionBusy === "list" ? "Refreshing…" : "Refresh"}</button>
              </div>
              {approvalSourceError && <p className="error-message" role="alert">{approvalSourceError}</p>}
              {actionError && <p className="error-message" role="alert">{actionError}</p>}
              {actionNotice && <p className="success-message" role="status">{actionNotice}</p>}
              {canonicalActions.length === 0 ? <p className="empty-state">No governed actions yet.</p> : <div className="instance-list">
                {canonicalActions.map((action) => {
                  const inspection = actionInspections[action.action_id];
                  return <div className="instance-row canonical-action-row" key={action.execution_id} data-action-id={action.action_id} data-action-state={action.state}>
                    <span>
                      <strong>Action {action.action_id.slice(0, 8)}</strong>
                      <small>{actionStateLabel(action.state)}{advanced ? ` · ${action.effect_class.replaceAll("_", " ")} · revision ${action.revision} · generation ${action.lease_generation}` : ""}</small>
                      {advanced && <small>Actor <code>{action.actor}</code> · updated {new Date(action.updated_at).toLocaleString()}</small>}
                    </span>
                    {advanced && <div className="memory-actions">
                      <button className="secondary" type="button" disabled={actionBusy !== null} onClick={() => void inspectCanonicalAction(action.action_id)}>{actionBusy === action.action_id ? "Loading…" : inspection ? "Refresh evidence" : "Inspect"}</button>
                    </div>}
                    {advanced && inspection && <div className="action-inspection">
                      <p><strong>Operation</strong> <code>{inspection.envelope.operation.type_id}</code></p>
                      <p>{inspection.executions.length} execution record{inspection.executions.length === 1 ? "" : "s"} · {inspection.receipts.length} receipt{inspection.receipts.length === 1 ? "" : "s"} · {inspection.events.length} ordered event{inspection.events.length === 1 ? "" : "s"}</p>
                      <p>Source workflow <code>{inspection.envelope.workflow_id}</code></p>
                      {inspection.envelope.origin && <div className="action-origin" aria-label="Bound action origin" role="region">
                        <p><strong>Bound origin</strong> · immutable source locators and content hashes</p>
                        <ul>
                          {([
                            ["Conversation", inspection.envelope.origin.conversation],
                            ["Invocation", inspection.envelope.origin.invocation],
                            ["Command", inspection.envelope.origin.command],
                            ["Approval", inspection.envelope.origin.approval],
                            ["Agent", inspection.envelope.origin.agent]
                          ] as const).filter((entry) => entry[1] !== null).map(([label, source]) => <li key={`${label}-${source!.source_type}-${source!.locator}`}><strong>{label}</strong> <code>{source!.locator}</code> <small>{source!.content_hash.digest_hex.slice(0, 12)}…</small></li>)}
                          {inspection.envelope.origin.evidence.map((source) => <li key={`Evidence-${source.source_type}-${source.locator}`}><strong>Evidence</strong> <code>{source.locator}</code> <small>{source.content_hash.digest_hex.slice(0, 12)}…</small></li>)}
                        </ul>
                        {inspection.envelope.origin.conversation && <button className="link-button" type="button" disabled={!conversations.some((conversation) => conversation.id === inspection.envelope.origin?.conversation?.locator)} title={conversations.some((conversation) => conversation.id === inspection.envelope.origin?.conversation?.locator) ? "Open the exact originating conversation" : "The originating conversation has not been projected into this compatibility session yet"} onClick={() => openChat(inspection.envelope.origin!.conversation!.locator)}>Open originating conversation →</button>}
                        {inspection.envelope.origin.invocation && <button className="link-button" type="button" disabled={invocationBusy !== null} onClick={() => void openCanonicalInvocationSource(inspection.envelope.origin!.invocation!)}>{invocationBusy === inspection.envelope.origin.invocation.locator ? "Opening exact invocation…" : "Open exact Runtime invocation →"}</button>}
                        {inspection.envelope.origin.approval && <button className="link-button" type="button" disabled={approvalSourceBusy !== null} onClick={() => void openApprovalSource(inspection.envelope.origin!.approval!.locator)}>{approvalSourceBusy === inspection.envelope.origin.approval.locator ? "Opening approval source…" : "Open approval, agent, command, and evidence →"}</button>}
                      </div>}
                      {(typeof inspection.envelope.compensates_action === "string" || (inspection.compensation_actions?.length ?? 0) > 0) && <div className="action-compensation-history" aria-label="Compensation history" role="region">
                        <p><strong>Compensation history</strong> · exact causal action links</p>
                        {typeof inspection.envelope.compensates_action === "string" && <button className="link-button" type="button" disabled={actionBusy !== null} onClick={() => void inspectCanonicalAction(inspection.envelope.compensates_action as string)}>Inspect compensated action <code>{inspection.envelope.compensates_action}</code> →</button>}
                        {(inspection.compensation_actions ?? []).map((compensationActionId) => <button className="link-button" type="button" disabled={actionBusy !== null} onClick={() => void inspectCanonicalAction(compensationActionId)} key={compensationActionId}>Inspect compensation <code>{compensationActionId}</code> →</button>)}
                      </div>}
                      {inspection.executions.at(-1)?.state === "committed" && inspection.envelope.effect_class === "reversible" && inspection.envelope.compensation != null && (inspection.compensation_actions?.length ?? 0) === 0 && <div className="action-compensation-control" aria-label={`Compensate action ${action.action_id}`} role="region">
                        <p><strong>Owner-authorized compensation</strong></p>
                        <p className="form-note">Protected Root will execute only the immutable compensation plan already attached to this action. The reason and confirmation become authorization evidence.</p>
                        <label>Reason
                          <textarea aria-label={`Compensation reason for action ${action.action_id}`} maxLength={4096} value={compensationReasons[action.action_id] ?? ""} onChange={(event) => setCompensationReasons((current) => ({ ...current, [action.action_id]: event.target.value }))} />
                        </label>
                        <button className="danger" type="button" disabled={actionBusy !== null || !(compensationReasons[action.action_id]?.trim())} onClick={() => void compensateCanonicalAction(action.action_id)}>{actionBusy === `compensate:${action.action_id}` ? "Initiating exact compensation…" : "Initiate exact compensation"}</button>
                      </div>}
                      {inspection.reconciliation_jobs.length > 0 && <p>Reconciliation: {inspection.reconciliation_jobs.map((job) => job.state.replaceAll("_", " ")).join(", ")}</p>}
                      <button className="link-button" type="button" disabled={workflowBusy !== null} onClick={() => void inspectCanonicalWorkflow(inspection.envelope.workflow_id)}>Load source workflow evidence →</button>
                      {userSettings?.interface_complexity === "developer" && <TechnicalDetails title="Canonical action evidence" summary={`${inspection.events.length} Root-owned events`}>
                        <p className="form-note">Read-only canonical evidence. Developer mode exposes exact intent, execution, receipt, reconciliation, correlation, and causation records without granting Root mutation authority.</p>
                        <pre aria-label={`Canonical action ${action.action_id} evidence`} aria-readonly="true" role="textbox" tabIndex={0}>{JSON.stringify(inspection, null, 2)}</pre>
                      </TechnicalDetails>}
                    </div>}
                  </div>;
                })}
              </div>}
            </article>

            <article className="agent-section-card governed-capability-section" aria-label="Governed capabilities">
              <div className="section-heading compact-heading">
                <div><h3>Governed capabilities</h3><p className="muted">{advanced ? "Inspect the exact bounded access Protected Root issued to physical Tool execution." : "MetaCog keeps tool access limited to what each action needs."}</p></div>
                <button className="secondary" type="button" disabled={capabilityBusy !== null} onClick={() => void refreshToolAuthorizations()}>{capabilityBusy === "list" ? "Refreshing…" : "Refresh"}</button>
              </div>
              {capabilityError && <p className="error-message" role="alert">{capabilityError}</p>}
              {toolAuthorizations.length === 0 ? <p className="empty-state">No governed tool activity yet.</p> : <div className="instance-list">
                {toolAuthorizations.map((authorization) => {
                  const inspection = toolAuthorizationInspections[authorization.action_id];
                  return <div className="instance-row governed-capability-row" key={authorization.permit_id} data-capability-state={authorization.state}>
                    <span>
                      <strong>{authorization.tool_name}</strong>
                      <small>{advanced ? `${authorization.state.replaceAll("_", " ")} · ${authorization.effect_class.replaceAll("_", " ")} access` : `Bounded tool access · ${authorization.state.replaceAll("_", " ")}`}</small>
                      {advanced && <small>{authorization.filesystem_scope_count} filesystem · {authorization.network_scope_count} network · {authorization.secret_slot_count} secret slot{authorization.secret_slot_count === 1 ? "" : "s"} · process spawn {authorization.process_spawn ? "allowed" : "denied"}</small>}
                    </span>
                    {advanced && <div className="memory-actions">
                      <button className="secondary" type="button" disabled={capabilityBusy !== null} onClick={() => void inspectToolAuthorization(authorization.action_id)}>{capabilityBusy === authorization.action_id ? "Loading…" : inspection ? "Refresh evidence" : "Inspect access"}</button>
                    </div>}
                    {advanced && inspection && <div className="action-inspection capability-inspection">
                      <p><strong>Manifest</strong> {authorization.tool_name} v{authorization.tool_manifest.version} · <code>{authorization.tool_manifest.content_hash.digest_hex.slice(0, 12)}…</code></p>
                      <p>Workflow fence revision {inspection.attempt_revision} · generation {inspection.lease_generation} · {inspection.platform}</p>
                      {inspection.scopes.filesystem.map((scope) => <p key={`${scope.access}-${scope.root}`}><strong>Filesystem {scope.access}</strong> <code>{scope.root}</code></p>)}
                      {inspection.scopes.network.map((scope) => <p key={scope.destination}><strong>Network</strong> <code>{scope.destination}</code></p>)}
                      {inspection.scopes.secret_slots.map((slot) => <p key={slot}><strong>Secret slot type</strong> <code>{slot}</code></p>)}
                      {(inspection.scopes.truncated.filesystem || inspection.scopes.truncated.network || inspection.scopes.truncated.secret_slots) && <p className="warning-box">The scope preview is bounded; the exact complete scope hash remains available in Developer evidence.</p>}
                      <p>Sandbox: filesystem {inspection.sandbox.filesystem_scope.replaceAll("_", " ")} · network {inspection.sandbox.network.replaceAll("_", " ")} · process tree {inspection.sandbox.process_tree.replaceAll("_", " ")} · secrets {inspection.sandbox.secrets.replaceAll("_", " ")}</p>
                      <button className="link-button" type="button" disabled={actionBusy !== null} onClick={() => void inspectCanonicalAction(authorization.action_id)}>Load governed action evidence →</button>
                      {userSettings?.interface_complexity === "developer" && <TechnicalDetails title="Exact capability evidence" summary={`Permit ${authorization.permit_id.slice(0, 8)} · read-only`}>
                        <p className="form-note">Exact content-minimized manifest, runner, workflow fence, policy, scope, proposal, argument, and permit hashes. Raw arguments, credential values, opaque secret handles, database access, and Root mutation authority are excluded.</p>
                        <pre aria-label={`Tool authorization ${authorization.action_id} evidence`} aria-readonly="true" role="textbox" tabIndex={0}>{JSON.stringify(inspection, null, 2)}</pre>
                      </TechnicalDetails>}
                    </div>}
                  </div>;
                })}
              </div>}
            </article>

            <article className="agent-section-card">
              <div className="section-heading compact-heading"><div><h3>Live conversation agents</h3><p className="muted">The user-facing Chat Agent, persistent Coordinator, and currently working named Coding Agents are shown.</p></div></div>
              <div className="instance-list">
                {(visibleConversation?.team?.sessions ?? []).filter((session) => session.kind === "chat" || session.kind === "coordinator" || (session.kind === "coder" && ["starting", "working", "waiting"].includes(session.state))).map((session) => <div className="instance-row" key={session.id}><span><strong>{session.display_name || (session.kind === "chat" ? "Chat Agent" : session.kind === "coordinator" ? "Coordinator" : "Coding Agent")}</strong><small>{session.kind.replaceAll("_", " ")} · {session.model ?? "Automatic model"} · prompt revision {session.prompt_revision}</small></span><em>{session.state}</em></div>)}
                {!visibleConversation?.team?.sessions.some((session) => session.kind === "chat" || session.kind === "coordinator" || (session.kind === "coder" && ["starting", "working", "waiting"].includes(session.state))) && <p className="muted">Open a conversation to inspect its Chat Agent, Coordinator, and active Coding Agents.</p>}
              </div>
            </article>

            <article className="agent-section-card topology-card">
              <div className="section-heading compact-heading"><div><h3>Active team</h3><p className="muted">{activeOrganization ? `${activeOrganization.name} · revision ${activeOrganization.revision}` : "Loading assignment…"}</p></div>{activeOrganization && <button className="secondary" type="button" onClick={() => editOrganization(activeOrganization)}>Edit team</button>}</div>
              {activeOrganization && <><div className="topology-diagram" aria-hidden="true"><div className="topology-governor">MetaCog runtime</div><span className="topology-line" /><div className="topology-team">{activeOrganization.name}</div><div className="topology-roles">{activeOrganization.roles.filter((role) => role.kind === "conversation" || role.kind === "coordinator" || role.kind === "coder").map((role) => <div className={`topology-role role-${role.kind}`} key={role.id}><strong>{role.name}</strong><small>{role.kind}</small></div>)}</div></div><ul className="topology-accessible-list" aria-label="Active conversation roles">{activeOrganization.roles.filter((role) => role.kind === "conversation" || role.kind === "coordinator" || role.kind === "coder").map((role) => <li key={`accessible-${role.id}`}><strong>{role.name}</strong><span>{role.kind.replaceAll("_", " ")} · {role.purpose}</span></li>)}</ul></>}
            </article>

            <div className="agent-overview-grid"><article className="agent-section-card"><h3>Current work</h3><strong className="agent-metric">{governorOverview?.active_runs ?? 0}</strong><p className="muted">running or waiting agent teams</p><button className="link-button" type="button" onClick={() => setAgentTab("activity")}>Inspect activity →</button></article><article className="agent-section-card"><h3>Recent outcomes</h3><strong className="agent-metric">{agentRuns.filter((run) => run.run.state === "completed").length}</strong><p className="muted">completed in the latest {governorOverview?.recent_runs ?? agentRuns.length} runs</p></article><article className="agent-section-card"><h3>Default assignment</h3><strong>{activeOrganization?.name ?? "Loading"}</strong><p className="muted">Projects and tasks can override this team.</p><button className="link-button" type="button" onClick={() => setAgentTab("teams")}>Manage assignments →</button></article></div>
          </div>}

          {agentTab === "teams" && <div className="agents-stack">
            <article className="agent-section-card assignments-card"><div className="section-heading compact-heading"><div><h3>Assignments</h3><p className="muted">Task override → project assignment → global default.</p></div></div><div className="assignment-row"><span><strong>Global default</strong><small>Used when no project or task override is selected.</small></span><select aria-label="Global default team" disabled={agentBusy || !organizationAssignments} value={organizationAssignments?.global_organization_id ?? ""} onChange={(event) => void assignGlobalOrganization(event.target.value)}>{organizations.map((organization) => <option value={organization.organization_id} key={`global-${organization.organization_id}`}>{organization.name} · r{organization.revision}</option>)}</select></div>{projects.map((project) => <div className="assignment-row" key={`assignment-${project.project_id}`}><span><strong>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</strong><small>Project default</small></span><select aria-label={`Team for ${project.root}`} disabled={agentBusy || !organizationAssignments} value={organizationAssignments?.project_overrides[project.project_id] ?? ""} onChange={(event) => void assignProjectOrganization(project.project_id, event.target.value || undefined)}><option value="">Use global default</option>{organizations.map((organization) => <option value={organization.organization_id} key={`${project.project_id}-${organization.organization_id}`}>{organization.name} · r{organization.revision}</option>)}</select></div>)}</article>

            {!organizationDraft ? <article className="agent-section-card"><div className="section-heading compact-heading"><div><h3>Reusable teams</h3><p className="muted">Each run pins the exact active revision it starts with.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("improvement")}>Create or import</button></div><div className="team-list">{organizations.map((organization) => <div className="team-row" key={`team-${organization.organization_id}`}><span><strong>{organization.name}</strong><small>{organization.roles.length} roles · revision {organization.revision}</small></span><div className="card-actions">{organizationAssignments?.global_organization_id === organization.organization_id && <span className="count-badge">Default</span>}<button className="secondary" type="button" onClick={() => editOrganization(organization)}>Edit</button></div></div>)}</div></article> : <article className="agent-section-card organization-editor">
              <div className="section-heading compact-heading"><div><h3>Edit team revision</h3><p className="muted">Changes create a new immutable revision. Running work stays pinned.</p></div><button className="secondary" type="button" onClick={() => { setOrganizationDraft(null); setOrganizationPreview(null); }}>Close</button></div>
              <label>Team name<input value={organizationDraft.name} onChange={(event) => { setOrganizationDraft({ ...organizationDraft, name: event.target.value }); setOrganizationPreview(null); }} /></label>
              <div className="editor-grid"><label>Maximum parallel agents<input type="number" min="1" max={organizationDraft.policy.max_total_agents} value={organizationDraft.policy.max_parallel_agents} onChange={(event) => { setOrganizationDraft({ ...organizationDraft, policy: { ...organizationDraft.policy, max_parallel_agents: Number(event.target.value) } }); setOrganizationPreview(null); }} /></label><label>Maximum total agents<input type="number" min="1" value={organizationDraft.policy.max_total_agents} onChange={(event) => { setOrganizationDraft({ ...organizationDraft, policy: { ...organizationDraft.policy, max_total_agents: Number(event.target.value) } }); setOrganizationPreview(null); }} /></label></div>
              <div className="role-editor-list">
                <div className="section-heading compact-heading"><h4>Roles</h4><button className="secondary" type="button" onClick={() => { setOrganizationDraft({ ...organizationDraft, roles: [...organizationDraft.roles, { id: "", kind: "temporary_specialist", name: "New specialist", purpose: "Handle a bounded specialty within this team.", prompt_policy: "Follow the assigned goal, owner policy, and evidence requirements.", model_policy: { capability_class: "general", preferred_models: [], allow_fallback: true, max_escalation_tier: 1 }, tool_capabilities: [], memory_namespace: "team" }] }); setOrganizationPreview(null); }}>Add role</button></div>
                {organizationDraft.roles.map((role, index) => <fieldset className="role-editor" key={role.id || `new-role-${index}`}><legend>{role.name || `Role ${index + 1}`}</legend><div className="editor-grid"><label>Name<input value={role.name} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, name: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label><label>Kind<select value={role.kind} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, kind: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }}>{["executive", "conversation", "coordinator", "coder", "evaluator", "tool_worker", "researcher", "tester", "temporary_specialist"].map((kind) => <option value={kind} key={kind}>{kind.replaceAll("_", " ")}</option>)}</select></label></div><label>Purpose<textarea value={role.purpose} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, purpose: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label>{advanced && <div className="advanced-fields"><label>Prompt policy<textarea value={role.prompt_policy} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, prompt_policy: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label><label>Preferred model<select value={role.model_policy.preferred_models[0] ?? ""} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, model_policy: { ...role.model_policy, preferred_models: event.target.value ? [event.target.value] : [] } }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }}><option value="">Automatic routing</option>{discoveredModels.map((model) => <option value={model.id} key={`${role.id}-${model.id}`}>{model.display_name}</option>)}</select></label><label>Memory namespace<input value={role.memory_namespace} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, memory_namespace: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label><label>Tool capabilities<input value={role.tool_capabilities.join(", ")} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, tool_capabilities: event.target.value.split(",").map((value) => value.trim()).filter(Boolean) }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label></div>}</fieldset>)}
              </div>
              {advanced && <div className="relationship-editor"><div className="section-heading compact-heading"><h4>Relationships</h4><button className="secondary" type="button" disabled={organizationDraft.roles.filter((role) => role.id).length < 2} onClick={() => { const roles = organizationDraft.roles.filter((role) => role.id); setOrganizationDraft({ ...organizationDraft, relationships: [...organizationDraft.relationships, { from: roles[0].id, to: roles[1].id, kind: "delegates_to" }] }); setOrganizationPreview(null); }}>Add relationship</button></div>{organizationDraft.relationships.map((relationship, index) => <div className="relationship-row" key={`${relationship.from}-${relationship.to}-${index}`}><select aria-label="From role" value={relationship.from} onChange={(event) => { const relationships = [...organizationDraft.relationships]; relationships[index] = { ...relationship, from: event.target.value }; setOrganizationDraft({ ...organizationDraft, relationships }); setOrganizationPreview(null); }}>{organizationDraft.roles.filter((role) => role.id).map((role) => <option value={role.id} key={`from-${index}-${role.id}`}>{role.name}</option>)}</select><span>→</span><select aria-label="Relationship kind" value={relationship.kind} onChange={(event) => { const relationships = [...organizationDraft.relationships]; relationships[index] = { ...relationship, kind: event.target.value }; setOrganizationDraft({ ...organizationDraft, relationships }); setOrganizationPreview(null); }}>{["delegates_to", "reviews", "escalates_to", "reports_to", "shares_information_with", "observed_by"].map((kind) => <option value={kind} key={kind}>{kind.replaceAll("_", " ")}</option>)}</select><span>→</span><select aria-label="To role" value={relationship.to} onChange={(event) => { const relationships = [...organizationDraft.relationships]; relationships[index] = { ...relationship, to: event.target.value }; setOrganizationDraft({ ...organizationDraft, relationships }); setOrganizationPreview(null); }}>{organizationDraft.roles.filter((role) => role.id).map((role) => <option value={role.id} key={`to-${index}-${role.id}`}>{role.name}</option>)}</select><button className="link-button" type="button" onClick={() => { setOrganizationDraft({ ...organizationDraft, relationships: organizationDraft.relationships.filter((_, candidate) => candidate !== index) }); setOrganizationPreview(null); }}>Remove</button></div>)}</div>}
              <div className="composer-actions"><button className="primary" type="button" disabled={agentBusy} onClick={() => void previewOrganizationChanges()}>{agentBusy ? "Validating…" : "Review changes"}</button></div>
              {organizationPreview && <div className="revision-preview"><h4>Review revision {organizationPreview.organization.revision}</h4>{organizationPreview.changes.length === 0 ? <p className="muted">No material changes detected.</p> : <ul>{organizationPreview.changes.map((change) => <li key={change}>{change}</li>)}</ul>}{organizationPreview.warnings.length > 0 && <div className="warning-box"><strong>Warnings</strong><ul>{organizationPreview.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul></div>}<label>Change summary<input value={organizationChangeSummary} onChange={(event) => setOrganizationChangeSummary(event.target.value)} placeholder="Why this revision should become active" /></label><button className="primary" type="button" disabled={agentBusy || !organizationChangeSummary.trim() || organizationPreview.changes.length === 0} onClick={() => void publishOrganizationChanges()}>Confirm and activate revision</button></div>}
            </article>}
          </div>}

          {agentTab === "activity" && <div className="activity-layout"><div className="agent-run-list" aria-label="Agent runs">{agentRuns.length === 0 ? <p className="empty-state">No agent runs yet.</p> : agentRuns.map((view) => <button className={`agent-run-row${selectedAgentRunId === view.run.id ? " active" : ""}`} type="button" onClick={() => setSelectedAgentRunId(view.run.id)} key={view.run.id}><span><strong>{tasks.find((task) => task.id === view.run.task_id)?.goal ?? "Agent run"}</strong><small>{new Date(view.run.updated_at).toLocaleString()} · {view.plan.selected_roles.length} roles</small></span><em className={`agent-state state-${view.run.state}`}>{view.run.state}</em></button>)}</div><article className="agent-section-card run-inspector">{!selectedAgentRun ? <div className="empty-state">Select a run to inspect its agents and evidence.</div> : <><div className="section-heading compact-heading"><div><h3>{selectedAgentTask?.goal ?? "Agent run"}</h3><p className="muted">Pinned revision <code>{selectedAgentRun.run.organization_revision_id}</code></p></div><div className="card-actions">{selectedAgentTask?.state === "blocked" && <button className="primary" type="button" disabled={taskBusy} onClick={() => void retryBlockedTask(selectedAgentTask.id)}>Retry task</button>}{selectedAgentTask?.state === "waiting_for_approval" && selectedAgentApproval && <button className="primary" type="button" disabled={taskBusy} onClick={() => void approveAndResume(selectedAgentTask.id, selectedAgentApproval.id)}>Approve and resume</button>}{["planned", "running", "waiting", "recovering"].includes(selectedAgentRun.run.state) && <button className="secondary" type="button" disabled={agentBusy} onClick={() => void cancelRun(selectedAgentRun.run.id)}>Cancel run</button>}</div></div><p>{selectedAgentRun.plan.selection_reason}</p><div className="run-metrics"><span><strong>{selectedAgentRun.instances.length}</strong><small>agents</small></span><span><strong>{selectedAgentRun.plan.items.length}</strong><small>work items</small></span><span><strong>{selectedAgentRun.usage.reduce((sum, usage) => sum + usage.input_tokens + usage.output_tokens, 0).toLocaleString()}</strong><small>tokens</small></span><span><strong>{selectedAgentRun.evaluations.length}</strong><small>evaluations</small></span></div><div className="instance-list">{selectedAgentRun.instances.map((instance) => { const role = organizations.flatMap((organization) => organization.roles).find((candidate) => candidate.id === instance.role_id); return <div className="instance-row" key={instance.id}><span><strong>{role?.name ?? instance.role_id}</strong><small>{instance.model ? `${instance.model.model_id} · ${instance.model.route_reason}` : "Model not routed yet"}</small></span><em>{instance.state}</em></div>; })}</div>{advanced && <><details><summary>Work plan and leases</summary><ul>{selectedAgentRun.plan.items.map((item) => <li key={item.id}><strong>{item.goal}</strong> · {item.state}</li>)}</ul><ul>{selectedAgentRun.leases.map((lease) => <li key={lease.id}><strong>{lease.goal}</strong> · {lease.state} · {lease.budget.consumed_tokens.toLocaleString()} tokens</li>)}</ul></details><details><summary>Evaluations and replay evidence</summary>{selectedAgentRun.evaluations.length === 0 ? <p className="muted">No evaluation recorded yet.</p> : selectedAgentRun.evaluations.map((evaluation) => <div className="evaluation-row" key={evaluation.id}><strong>{evaluation.outcome}</strong><span>{evaluation.confidence.basis_points / 100}% confidence</span>{evaluation.evidence.map((evidence, index) => <small key={`${evaluation.id}-${index}`}>{evidence.passed ? "Pass" : "Fail"} · {evidence.method}: {evidence.detail}</small>)}</div>)}</details></>}</>}</article></div>}

          {agentTab === "governor" && advanced && <div className="agents-stack">
            <article className="agent-section-card">
              <div className="section-heading compact-heading"><div><h3>Governor guardrails</h3><p className="muted">The model Governor remains disabled. These owner-controlled boundaries are available to the operator and CLI.</p></div><span className={`agent-state state-${governorPolicy?.governor_model_enabled ? "active" : "idle"}`}>{governorPolicy?.governor_model_enabled ? "enabled" : "disabled"}</span></div>
              <div className="run-metrics"><span><strong>{governorPolicy?.allowed_actions.length ?? 0}</strong><small>allowed actions</small></span><span><strong>{governorPolicy?.denied_actions.length ?? 0}</strong><small>denied actions</small></span><span><strong>{governorPolicy?.protected_surfaces.length ?? 0}</strong><small>protected surfaces</small></span><span><strong>{governorPolicy?.requires_owner_approval ? "Required" : "Not required"}</strong><small>owner approval</small></span></div>
              <p>{governorPolicy?.implementation_path ?? "Loading governed implementation path…"}</p>
              <details className="governor-details"><summary>Inspect authority boundaries</summary><div className="editor-grid"><div><strong>Allowed</strong><ul>{governorPolicy?.allowed_actions.map((action) => <li key={action}>{action.replaceAll("_", " ")}</li>)}</ul></div><div><strong>Denied</strong><ul>{governorPolicy?.denied_actions.map((action) => <li key={action}>{action.replaceAll("_", " ")}</li>)}</ul></div><div><strong>Protected</strong><ul>{governorPolicy?.protected_surfaces.map((surface) => <li key={surface}>{surface.replaceAll("_", " ")}</li>)}</ul></div></div></details>
            </article>
            <article className="agent-section-card">
              <div className="section-heading compact-heading"><div><h3>Governed proposals</h3><p className="muted">Approve bounded changes here, then hand approved implementation to a normal project chat. Chat → Coordinator → Coding Agents remains the only mutation path.</p></div><span className="count-badge">{governorProposals.length}</span></div>
              {governorProposals.length === 0 ? <div className="empty-state">No Governor proposals are awaiting review.</div> : <div className="team-list">{governorProposals.map((proposal) => <div className="team-row" key={proposal.id}><span><strong>{proposal.title}</strong><small>{proposal.target.replaceAll("_", " ")} · {proposal.risk} risk · {proposal.state.replaceAll("_", " ")}</small><small>{proposal.diagnosis}</small></span><div className="card-actions">{proposal.state === "awaiting_owner_approval" && <><button className="primary" type="button" disabled={agentBusy} onClick={() => void decideGovernor(proposal, true)}>Approve</button><button className="secondary danger" type="button" disabled={agentBusy} onClick={() => void decideGovernor(proposal, false)}>Reject</button></>}{proposal.state === "approved" && <button className="primary" type="button" disabled={agentBusy} onClick={() => void handGovernorProposalToChat(proposal)}>Request implementation</button>}<span className={`agent-state state-${proposal.state}`}>{proposal.state.replaceAll("_", " ")}</span></div><details className="governor-details"><summary>Proposal evidence and validation</summary><p>{proposal.proposed_change}</p><strong>Success criteria</strong><ul>{proposal.success_criteria.map((criterion) => <li key={criterion}>{criterion}</li>)}</ul>{proposal.validation_findings.length > 0 && <><strong>Validation findings</strong><ul>{proposal.validation_findings.map((finding) => <li key={finding}>{finding}</li>)}</ul></>}</details></div>)}</div>}
              {governorProposals.some((proposal) => proposal.state === "approved") && <label>Implementation conversation<select aria-label="Governor implementation conversation" value={governorConversationId} onChange={(event) => setGovernorConversationId(event.target.value)}><option value="">Select a project chat</option>{conversations.filter((conversation) => conversation.state === "active" && conversation.project_id).map((conversation) => <option value={conversation.id} key={conversation.id}>{conversation.title}</option>)}</select></label>}
            </article>
          </div>}
        </section>}

        <section className="panel scheduler-panel" id="improvement" aria-label="Measured improvement">
          <div className="section-heading">
            <div>
              <p className="eyebrow">IMPROVEMENT</p>
              <h2>Measured changes, safe rollback</h2>
              <p className="muted">Candidates stay inactive until benchmark evidence supports promotion. Shadow work can evaluate, but cannot change a real task.</p>
            </div>
            <div className="card-actions"><button className="secondary" type="button" onClick={() => { setAgentTab("overview"); navigateSurface("agents"); }}>Agents overview</button><span className="count-badge">{experiments.filter((item) => item.state === "running").length} running</span></div>
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
                <label>Portable organization JSON<textarea aria-label="Portable organization JSON" value={portableOrganizationJson} onChange={(event) => setPortableOrganizationJson(event.target.value)} placeholder="Export a revision or paste a portable MetaCog organization" /></label>
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
                <textarea aria-label="Signed organization package" value={organizationShareJson} onChange={(event) => setOrganizationShareJson(event.target.value)} placeholder="Paste a signed MetaCog organization package" />
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
          {CHATGPT_SITES_LIBRARY_ENABLED ? <>
            <div className="section-heading sites-heading">
              <div>
                <p className="eyebrow">SITES</p>
                <h2>Sites</h2>
                <p className="muted">Create, open, and keep refining your ChatGPT Sites in chat.</p>
              </div>
              <span className="count-badge">{siteLibraryEntries.length}</span>
            </div>
            <div className="sites-simple-layout">
              {codexSitesError && <div className="inline-notice warning" role="alert">{codexSitesError}</div>}
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
                {siteLibraryEntries.length === 0
                  ? <div className="site-empty-state"><strong>No sites yet</strong><span>Build one from a prompt or an existing project.</span></div>
                  : siteLibraryEntries.map((entry) => {
                    const site = entry.localSite;
                    const project = site ? projects.find((candidate) => candidate.project_id === site.project_id) : undefined;
                    const deployment = site ? siteDeployments[site.id]?.deployments[0] : undefined;
                    return <article className="site-library-card" key={entry.key} onContextMenu={(event) => openSiteContextMenu(event, entry.key)}>
                      <div className="site-preview">
                        <span aria-hidden="true">▧</span>
                        {entry.screenshotUrl && <img src={entry.screenshotUrl} alt={`Preview of ${entry.displayName}`} loading="lazy" referrerPolicy="no-referrer" onError={(event) => event.currentTarget.remove()} />}
                      </div>
                      <div className="site-library-copy">
                        <h3>{entry.displayName}</h3>
                        {entry.description && <p>{entry.description}</p>}
                        <span className="site-url">{entry.publicUrl}</span>
                        <small>{project ? project.root.split(/[\\/]/).filter(Boolean).at(-1) : "ChatGPT Sites"}{deployment ? ` · ${deployment.state}` : " · Ready to refine"}</small>
                      </div>
                      <div className="site-library-actions">
                        <button className="secondary" type="button" title="Uses MetaCog's separate, persistent browser profile" onClick={() => void openSiteInApp(entry)}>Open in MetaCog browser</button>
                        <button className="secondary" type="button" onClick={() => continueSiteLibraryChat(entry)}>Continue editing</button>
                        <button className="site-more-button" type="button" aria-label={`More options for ${entry.displayName}`} onClick={(event) => openSiteContextMenu(event, entry.key)}>•••</button>
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
              <p className="muted">Connect outbound with your SSH agent and an explicitly pinned host key. MetaCog never opens a remote control listener.</p>
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
              <p className="muted">Reserve durable local sessions now. MetaCog reports them as requested until a real executor is attached.</p>
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
                {platformComputerScreenOnly ? "MetaCog can see the primary display but cannot interact." : "MetaCog can see and use the primary display. Every input batch must match the latest captured pixels."}
              </p>}
              <p className="muted">Reservation is local and restart-safe. It will not be labeled active until MetaCog has attached a verified executor.</p>
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
                    <small>Returning control blanks the current page before MetaCog can read it.</small>
                    <button className="primary" type="button" disabled={platformBusy} onClick={() => reclaimPlatformBrowser(session.id)}>Return control to MetaCog</button>
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
                    <small>MetaCog cannot see or control this display while it is yours. Returning control does not capture your activity.</small>
                    <button className="primary" type="button" disabled={platformBusy} onClick={() => reclaimPlatformComputer(session.id)}>Return computer control to MetaCog</button>
                  </>}
                  <button className="secondary" type="button" disabled={platformBusy} onClick={() => togglePlatformHistory(session.id)}>{platformHistory[session.id] ? "Hide history" : "History"}</button>
                  {!(["closed", "failed"] as string[]).includes(session.state) && <button className="link-button" type="button" disabled={platformBusy} onClick={() => closePlatform(session.id)}>Close</button>}
                </div>
              </article>)}
            </div>
          </div>
        </section>

        <section className="panel scheduler-panel" id="scheduler" aria-label="Scheduler">
          <>
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
                  <h3>What should MetaCog do on a schedule?</h3>
                  <p>Describe the work. MetaCog will help refine the prompt, cadence, destination, and permissions before it runs unattended.</p>
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
              <p className="scheduled-local-note">Local project schedules need this computer on, MetaCog running, and the project available. They currently run in the selected project; dedicated Git worktree isolation is planned.</p>
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
                      {schedule.state !== "completed" && <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => requestScheduleWake(schedule.id)}>Run now</button>}
                      <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => duplicateSchedule(schedule)}>Duplicate</button>
                      <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => toggleScheduleHistory(schedule.id)}>{scheduleHistory[schedule.id] ? "Hide runs" : "Recent runs"}</button>
                      {schedule.state !== "completed" && <button className="link-button" type="button" disabled={scheduleBusy} onClick={() => changeScheduleState(schedule.id, "completed")}>Delete</button>}
                    </div>
                  </article>)}
              </div>
            </div>
          </>{advanced && <TechnicalDetails title="Exact scheduler controls" summary="Triggers, budgets, durable identities, and manual wake controls">
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
          </TechnicalDetails>}
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
                  : "Finish signing in in your browser. MetaCog will connect automatically."}</p>
                <button className="link-button" type="button" onClick={() => openVerification(login.verification_url)}>
                  {login.user_code ? "Open device authorization page" : "Open sign-in page"}
                </button>
                {login.user_code && <p className="device-code">Code: <strong>{login.user_code}</strong></p>}
                <small>{login.user_code
                  ? "MetaCog checks the device authorization automatically after you enter the code."
                  : "The callback listener closes after 3 minutes. If it expires, the sign-in option will return here."}</small>
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
            {advanced && <TechnicalDetails title="Connection details" summary="Identifiers and optional OpenAI routing">
              <div className="advanced-fields provider-advanced">
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
              </div>
            </TechnicalDetails>}
            <p className="form-note">MetaCog discovers models from the endpoint before saving. Credentials go directly to the operating-system credential store.</p>
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
                Add a tool server. MetaCog discovers its capabilities before saving it and
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
                <TechnicalDetails title="Server transport" summary="Protocol, executable, identity, and authentication">
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
                </TechnicalDetails>
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
          <>
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
            <p className="plugin-directory-note">Enabled plugins can be pinned to a chat with an explicit @ mention. Mentioning a plugin does not invoke it or expand its grants; the Chat Agent can use only that exact revision through the normal approval and audit boundary. The universal public marketplace and connector sign-in are not connected in this build yet.</p>
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
                  <p className="plugin-directory-description">Adds a reusable sandboxed capability to MetaCog chats. Its requested access stays governed by the host policy.</p>
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
          </>{advanced && <TechnicalDetails title="Plugin administration" summary="Local packages, exact grants, structured invocation, and immutable evidence">
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
                <p className="form-note">Local plugins are imported disabled and unsigned. MetaCog does not grant access automatically.</p>
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
          </TechnicalDetails>}
        </section>

        <section className="panel settings-page" id="settings" aria-label="Settings">
          <button className="settings-back" type="button" onClick={() => navigateSurface("tasks")}><span aria-hidden="true">←</span> Back to app</button>
          <label className="settings-search"><span aria-hidden="true">⌕</span><span className="sr-only">Search settings</span><input aria-label="Search settings" value={settingsSearch} onChange={(event) => setSettingsSearch(event.target.value)} placeholder="Search settings…" /></label>
          <div className="settings-layout">
            <div className="settings-nav-shell">
            <nav className="settings-nav" aria-label="Settings sections">
              {settingsGroups.map((group) => {
                const items = group.items.filter((item) => (advanced || item.id !== "hooks") && (!normalizedSettingsSearch || item.label.toLocaleLowerCase().includes(normalizedSettingsSearch)));
                return items.length > 0 && <section key={group.label}><h3>{group.label}</h3>{items.map((item) => <button className={settingsTab === item.id ? "active" : ""} type="button" aria-current={settingsTab === item.id ? "page" : undefined} onClick={() => { setSettingsTab(item.id); if (item.id === "hooks") void refreshCodexHooks(); window.scrollTo({ top: 0, behavior: "auto" }); }} key={item.id}><span aria-hidden="true">{item.glyph}</span>{item.label}</button>)}</section>;
              })}
              {settingsGroups.every((group) => group.items.every((item) => normalizedSettingsSearch && !item.label.toLocaleLowerCase().includes(normalizedSettingsSearch))) && <p className="settings-no-results">No settings found.</p>}
            </nav>
            <div
              className="settings-nav-resizer"
              role="separator"
              aria-orientation="vertical"
              aria-label="Resize settings navigation"
              aria-valuemin={190}
              aria-valuemax={420}
              aria-valuenow={settingsNavWidth}
              tabIndex={0}
              onPointerDown={beginSettingsNavResize}
              onDoubleClick={() => setSettingsNavWidth(244)}
              onKeyDown={(event) => {
                if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
                event.preventDefault();
                setSettingsNavWidth((current) => Math.min(420, Math.max(190, current + (event.key === "ArrowRight" ? 16 : -16))));
              }}
            />
            </div>
            <div className="settings-content" data-settings-tab={settingsTab}>
              <div className="settings-pane" data-settings-pane="general">
                <div className="section-heading">
                  <div><h2 id="settings-heading">General</h2><p className="muted">Choose how MetaCog works by default.</p></div>
                  {userSettings && <span className="count-badge">Revision {userSettings.revision}</span>}
                </div>
                {userSettings ? <><h3>Permissions</h3><div className="provider-setup settings-card settings-card-stack" aria-label="Owner settings">
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
            <fieldset className="settings-card settings-card-stack" aria-label="Interface complexity">
              <legend><strong>Interface complexity</strong></legend>
              <p className="form-note">All three choices use the same governed runtime. Higher levels reveal more controls and evidence; they never grant more authority.</p>
              {(["simple", "advanced", "developer"] as const).map((complexity) => <label className="check-row" key={complexity}>
                <input type="radio" name="interface-complexity" value={complexity} checked={userSettings.interface_complexity === complexity} disabled={settingsBusy} onChange={() => void setInterfaceComplexity(complexity)} />
                <span><strong>{complexity[0].toUpperCase() + complexity.slice(1)}</strong><small>{complexity === "simple" ? "Everyday chat, projects, approvals, progress, and verified outcomes." : complexity === "advanced" ? "Practical engineering controls, commands, diffs, hooks, policies, and detailed verification." : "Under-the-hood contracts, manifests, lineage, experiments, IPC, sandbox, and diagnostic state."}</small></span>
              </label>)}
              {userSettings.interface_complexity === "developer" && <p aria-label={DEVELOPER_GOVERNANCE_WARNING} className="form-note" role="alert" tabIndex={0}>{DEVELOPER_GOVERNANCE_WARNING}</p>}
            </fieldset>
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
                  <span>{revision.settings.autonomy} · {revision.settings.interface_complexity} · shadow {revision.settings.shadow_experiments ? "on" : "off"}</span>
                  <small>{revision.reason}</small>
                </li>)}
              </ul>
            </div>}
            {userSettings.interface_complexity === "developer" && <TechnicalDetails title="Runtime contract inspector" summary="Current compatibility projection">
              <p className="form-note">This read-only projection exposes the shared settings contract and disclosed Protected Root health. Root mutation authority, credentials, and evaluator internals remain outside this client.</p>
              <pre aria-label="Current runtime settings contract" aria-readonly="true" role="textbox" tabIndex={0}>{JSON.stringify({
                stable_type_id: "metacog.user_settings.v2",
                schema_version: userSettings.schema_version,
                revision: userSettings.revision,
                interface_complexity: userSettings.interface_complexity,
                authority: "presentation_only",
                protected_mutation: "root_required",
                root: protectedRoot ? {
                  service: protectedRoot.service,
                  process_id: protectedRoot.process_id,
                  runtime_identity: protectedRoot.runtime_identity.instance_id,
                  protocol_version: protectedRoot.protocol_version,
                  session_id: protectedRoot.session_id,
                  storage_schema_version: protectedRoot.storage_schema_version,
                  writer_queue_capacity: protectedRoot.writer_queue_capacity,
                  migration_receipt: protectedRoot.migration_receipt
                } : "unavailable"
              }, null, 2)}</pre>
            </TechnicalDetails>}
                </div><h3>General</h3><div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Default file destination</strong><small>Files and folders remain in their attached project or explicit global folder.</small></span><span className="count-badge">Project</span></div>
                  <div className="settings-control"><span><strong>Agent environment</strong><small>MetaCog runs locally on this computer through the governed daemon.</small></span><span className="count-badge">Local</span></div>
                  <div className="settings-control"><span><strong>Language</strong><small>Language for the app interface</small></span><span className="count-badge">Auto detect</span></div>
                  <div className="settings-control"><span><strong>Suggested prompts</strong><small>Show focused starting points on consumer workflows.</small></span><span className="count-badge">On</span></div>
                </div><h3>Composer</h3><div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Send shortcut</strong><small>Enter sends a prompt; Shift+Enter inserts a line.</small></span><span className="count-badge">Enter</span></div>
                  <div className="settings-control"><span><strong>Follow-up behavior</strong><small>New messages are queued while MetaCog is working.</small></span><span className="count-badge">Queue</span></div>
                </div><h3>Notifications</h3><div className="settings-card settings-card-stack"><div className="settings-control"><span><strong>Permission and question notifications</strong><small>MetaCog surfaces requests that require owner input without pretending a background action completed.</small></span><span className="count-badge">On</span></div></div></> : <p className="empty-state">Loading durable settings…</p>}
              </div>
              <div className="settings-pane" data-settings-pane="import">
                <div className="section-heading"><div><h2>Import</h2><p className="muted">Bring local folders and portable MetaCog data into this installation.</p></div></div>
          <h3>Import from another AI app</h3>
          <div className="settings-card settings-card-stack settings-unavailable" aria-label="Other AI app imports">
            <div className="settings-control"><span><strong>Keep imports in sync</strong><small>Continuous cross-app synchronization is not available.</small></span><span className="count-badge">Off</span></div>
            <div className="settings-control"><span><strong>Claude Code</strong><small>No compatible setup was detected; direct import is not implemented.</small></span><button className="secondary" type="button" disabled>Import</button></div>
            <div className="settings-control"><span><strong>Cursor</strong><small>No compatible setup was detected; direct import is not implemented.</small></span><button className="secondary" type="button" disabled>Import</button></div>
          </div>
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
                <div className="section-heading"><div><h2>Configuration</h2><p className="muted">Configure agent defaults, updates, and diagnostics.</p></div></div>
          <h3>Agent defaults</h3>
          <div className="settings-card settings-card-stack">
            <div className="settings-control"><span><strong>Approval policy</strong><small>Derived from the durable autonomy setting.</small></span><button className="secondary" type="button" onClick={() => setSettingsTab("general")}>{userSettings?.autonomy ?? "Loading"}</button></div>
            <div className="settings-control"><span><strong>Filesystem boundary</strong><small>Attached projects and explicit global folders define normal workspace authority.</small></span><span className="count-badge">Governed</span></div>
            <div className="settings-control"><span><strong>Web access</strong><small>Provider and tool access stays within reviewed capabilities.</small></span><span className="count-badge">Capability based</span></div>
            <div className="settings-control"><span><strong>Reasoning and output detail</strong><small>Available model features are discovered from each provider endpoint.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("providers")}>Models</button></div>
          </div>
          {advanced && <TechnicalDetails
            title="Codex app-server transport"
            summary={codexHarnessStatus?.transport?.kind === "remote_web_socket"
              ? `Remote · ${codexHarnessStatus.transport.endpoint}`
              : codexHarnessStatus?.transport?.kind === "remote_unix_socket"
                ? `Unix socket · ${codexHarnessStatus.transport.socket_path}`
                : "Local stdio (default)"}
          >
            <p className="form-note">Connect to an explicitly managed Codex app-server over WebSocket. Use <code>wss://</code> except for localhost or an SSH-forwarded loopback connection. MetaCog stores only the environment-variable name, never its bearer token.</p>
            <div className="settings-card settings-card-stack" aria-label="Codex app-server transport configuration">
              <label>Remote endpoint<input aria-label="Codex remote endpoint" value={codexRemoteEndpoint} disabled={codexTransportBusy} onChange={(event) => setCodexRemoteEndpoint(event.target.value)} placeholder="wss://host.example/codex" /></label>
              <label>Bearer token environment variable (optional)<input aria-label="Codex remote bearer token environment variable" value={codexRemoteAuthTokenEnv} disabled={codexTransportBusy || !codexRemoteEndpoint.trim()} onChange={(event) => setCodexRemoteAuthTokenEnv(event.target.value)} placeholder="CODEX_REMOTE_TOKEN" /></label>
              <div className="composer-actions"><button className="primary" type="button" disabled={codexTransportBusy || !userSettings} onClick={() => void saveCodexTransport()}>{codexTransportBusy ? "Connecting…" : codexRemoteEndpoint.trim() ? "Connect remote" : "Use local"}</button></div>
              <small>Transport changes are blocked while work is active and must initialize successfully before they are saved.</small>
              {codexTransportError && <p className="error-message" role="alert">{codexTransportError}</p>}
            </div>
          </TechnicalDetails>}
          {advanced && <TechnicalDetails
            title="Windows sandbox"
            summary={`Codex restricted-token sandbox · ${codexConfigurationStatus?.windows_sandbox?.status ?? "checking"}`}
          >
            <p className="form-note">These controls call the pinned Codex 0.150.1 sandbox setup directly. Setup may request Windows administrator approval. MetaCog blocks changes while any conversation is actively working.</p>
            <div className="settings-card settings-card-stack" aria-label="Windows sandbox configuration">
              <div className="settings-control">
                <span><strong>Default restricted-token sandbox</strong><small>Configure the elevated Codex helper and make it the default for new Windows sessions.</small></span>
                {codexConfigurationStatus?.windows_sandbox?.status === "ready"
                  ? <span className="count-badge">Ready</span>
                  : <button className="primary" type="button" disabled={!navigator.userAgent.includes("Windows") || windowsSandboxBusy !== null} onClick={() => void configureWindowsSandbox()}>{windowsSandboxBusy === "setup" ? "Setting up…" : "Set up"}</button>}
              </div>
              <div className="provider-form windows-sandbox-read-form" aria-label="Additional sandbox read directory">
                <div className="section-heading compact-heading"><div><h3>Additional read directory</h3><p className="muted">Refresh one project sandbox with bounded read-only access. This does not add a writable root or change organization authority.</p></div></div>
                <div className="editor-grid">
                  <label>Project<select aria-label="Windows sandbox project" value={windowsSandboxProjectId} disabled={windowsSandboxBusy !== null} onChange={(event) => setWindowsSandboxProjectId(event.target.value)}><option value="">Choose project</option>{projects.map((project) => <option value={project.project_id} key={`sandbox-project-${project.project_id}`}>{projectDisplayName(project)}</option>)}</select></label>
                  <label>Permission profile<select aria-label="Windows sandbox permission profile" value={windowsSandboxPermissionProfile} disabled={windowsSandboxBusy !== null || windowsSandboxPermissionProfiles.length === 0} onChange={(event) => setWindowsSandboxPermissionProfile(event.target.value)}>{windowsSandboxPermissionProfiles.length === 0 && <option value=":workspace">Workspace</option>}{windowsSandboxPermissionProfiles.map((profile) => <option value={profile.id} key={`sandbox-profile-${profile.id}`}>{profile.id}{profile.description ? ` · ${profile.description}` : ""}</option>)}</select></label>
                </div>
                <label>Folder<div className="settings-inline-field"><input aria-label="Windows sandbox read directory" value={windowsSandboxReadPath} disabled={windowsSandboxBusy !== null} onChange={(event) => setWindowsSandboxReadPath(event.target.value)} placeholder="C:\\path\\to\\reference" /><button className="secondary" type="button" disabled={!navigator.userAgent.includes("Windows") || windowsSandboxBusy !== null} onClick={() => void chooseWindowsSandboxReadFolder()}>Browse</button></div></label>
                <div className="composer-actions"><button className="primary" type="button" disabled={!navigator.userAgent.includes("Windows") || windowsSandboxBusy !== null || !windowsSandboxProjectId || !windowsSandboxReadPath.trim()} onClick={() => void addWindowsSandboxReadDirectory()}>{windowsSandboxBusy === "read" ? "Granting…" : "Grant read access"}</button></div>
                <p className="form-note">The folder must already exist and remain outside symlink escapes. Codex applies the effective project permission profile and does not persist this folder in MetaCog settings.</p>
              </div>
              {!navigator.userAgent.includes("Windows") && <p className="empty-state">Windows sandbox controls are available only in the Windows desktop app.</p>}
              {windowsSandboxNotice && <p role="status">{windowsSandboxNotice}</p>}
              {windowsSandboxError && <p className="error-message" role="alert">{windowsSandboxError}</p>}
            </div>
          </TechnicalDetails>}
          {advanced && <TechnicalDetails title="Codex feature flags" summary={`${codexFeatures.length} flags from pinned Codex 0.150.1`}>
            <p className="form-note">These are the same process-wide flags exposed by <code>mcg features</code>. Changes affect new Codex sessions and cannot expand MetaCog organization, approval, or agent authority.</p>
            <label className="codex-feature-search"><span className="sr-only">Search Codex feature flags</span><input value={codexFeatureSearch} onChange={(event) => setCodexFeatureSearch(event.target.value)} placeholder="Search feature flags" /></label>
            <div className="codex-feature-list" data-scroll-chain aria-label="Codex feature flags">
              {codexFeatures
                .filter((feature) => {
                  const query = codexFeatureSearch.trim().toLocaleLowerCase();
                  return !query || feature.name.toLocaleLowerCase().includes(query) || feature.displayName?.toLocaleLowerCase().includes(query) || feature.description?.toLocaleLowerCase().includes(query);
                })
                .sort((left, right) => left.name.localeCompare(right.name))
                .map((feature) => <label className="codex-feature-row" key={feature.name}>
                  <span><strong>{feature.displayName ?? feature.name}</strong><small><code>{feature.name}</code> · {codexFeatureStageLabel(feature.stage)}{feature.description ? ` · ${feature.description}` : ""}</small></span>
                  <span className="settings-switch"><input aria-label={`${feature.enabled ? "Disable" : "Enable"} ${feature.name}`} type="checkbox" checked={feature.enabled} disabled={codexFeatureBusy !== null} onChange={(event) => void changeCodexFeature(feature, event.target.checked)} /><i aria-hidden="true" /></span>
                </label>)}
              {codexFeatures.length > 0 && codexFeatures.every((feature) => {
                const query = codexFeatureSearch.trim().toLocaleLowerCase();
                return query && !feature.name.toLocaleLowerCase().includes(query) && !feature.displayName?.toLocaleLowerCase().includes(query) && !feature.description?.toLocaleLowerCase().includes(query);
              }) && <p className="empty-state">No feature flags match that search.</p>}
              {codexFeatures.length === 0 && !codexFeatureError && <p className="empty-state">Loading feature flags…</p>}
            </div>
            {codexFeatureBusy && <p role="status">Updating <code>{codexFeatureBusy}</code>…</p>}
            {codexFeatureError && <p className="error-message" role="alert">{codexFeatureError}</p>}
          </TechnicalDetails>}
          <h3>Application</h3>
          <div className="provider-connections" aria-label="Application updates">
            <div className="section-heading compact-heading">
              <div>
                <h3>Application updates</h3>
                <p className="muted">MetaCog checks only the trusted release service embedded by the owner-controlled build and accepts only packages signed by its embedded update key.</p>
              </div>
              {updateConfiguration && <span className="count-badge">Version {updateConfiguration.current_version}</span>}
            </div>
            {!updateConfiguration && !updateError && <p className="empty-state">Loading update policy…</p>}
            {updateConfiguration?.recovery_notice && <p className="error-message" role="status">
              {updateConfiguration.recovery_notice}
            </p>}
            {updateConfiguration && !updateConfiguration.configured && <p className="empty-state">
              Updates are unavailable in this build. Install a newer owner-signed package manually; MetaCog will never accept an unsigned fallback.
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
                  <h3>MetaCog {updateCheck.available_version}</h3>
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
                <h3>Install signed MetaCog {updateCheck.available_version}?</h3>
                <p className="form-note">MetaCog will download and verify the signed package, create and verify a credential-free portable backup, stop the local service cleanly, install the update, and restart. It will stop without installing if any verification fails.</p>
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
              <p><strong>Overall: {diagnostics.overall.toUpperCase()}</strong> · MetaCog {diagnostics.version} · {diagnostics.os}</p>
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
            <p className="form-note">Restore is intentionally offline: stop the daemon, then run <code>mcg backup restore PATH --confirm</code>. Add <code>--restore-secrets</code> only for an encrypted credential backup. MetaCog retains the previous database for rollback.</p>
          </div>
              </div>

              <div className="settings-pane settings-summary-pane" data-settings-pane="profile">
                <h2>Profile</h2>
                <div className="settings-profile-hero">
                  <AccountAvatar className="settings-profile-avatar" profile={signedInWithOpenAi ? openAiProfile : undefined} avatarUrl={signedInWithOpenAi ? openAiAvatarUrl : undefined} fallback={signedInWithOpenAi ? "O" : "M"} />
                  <strong>{signedInWithOpenAi ? openAiAccountName : "Local owner"}</strong>
                  <span>{signedInWithOpenAi ? openAiAccountDetail : "Private local profile"}</span>
                  {signedInWithOpenAi && <div className="settings-profile-avatar-actions">
                    <input
                      ref={accountAvatarInputRef}
                      className="visually-hidden"
                      type="file"
                      accept="image/png,image/jpeg,image/webp,image/gif"
                      aria-label="Choose profile photo"
                      onChange={(event) => {
                        chooseAccountAvatar(event.currentTarget.files?.[0], openAiAccountKey);
                        event.currentTarget.value = "";
                      }}
                    />
                    <button className="secondary" type="button" onClick={() => accountAvatarInputRef.current?.click()}>{localOpenAiAvatarUrl ? "Change photo" : "Choose photo"}</button>
                    {localOpenAiAvatarUrl && <button className="secondary" type="button" onClick={removeAccountAvatar}>Use OpenAI photo</button>}
                  </div>}
                  {signedInWithOpenAi && !openAiProfile?.avatar_url && !localOpenAiAvatarUrl && <small className="settings-profile-avatar-note">OpenAI did not provide a profile photo for this sign-in. Choose one to keep it privately on this computer.</small>}
                  {accountAvatarError && <p className="form-error" role="alert">{accountAvatarError}</p>}
                </div>
                <h3>Activity</h3>
                <div className="settings-stats" aria-label="Profile activity">
                  <div><strong>{proxyUsage?.usage.total_tokens.toLocaleString() ?? "—"}</strong><span>Tokens</span></div>
                  <div><strong>{proxyUsage?.call_count.toLocaleString() ?? "—"}</strong><span>Provider calls</span></div>
                  <div><strong>{proxyUsage?.conversation_count.toLocaleString() ?? "—"}</strong><span>Conversations</span></div>
                  <div><strong>{projects.length.toLocaleString()}</strong><span>Projects</span></div>
                </div>
                <div className="settings-card"><div><strong>Private by design</strong><p>MetaCog shows only activity recorded by this local installation. It does not invent streaks, public handles, or subscription details that providers do not expose.</p></div><button className="secondary" type="button" onClick={() => setSettingsTab("account")}>Manage account</button></div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="appearance">
                <h2>Appearance</h2>
                <h3>Theme</h3>
                <div className="theme-choices" role="group" aria-label="Theme">
                  {(["system", "light", "dark"] as ThemePreference[]).map((theme) => <button key={theme} type="button" aria-pressed={themePreference === theme} onClick={() => setThemePreference(theme)}><span className={`theme-preview theme-preview-${theme}`} aria-hidden="true"><i /><i /><i /></span><strong>{theme.charAt(0).toUpperCase() + theme.slice(1)}</strong></button>)}
                </div>
                <h3>Preferences</h3>
                <div className="settings-card settings-card-stack">
                  <label className="settings-control"><span><strong>Reduce motion</strong><small>Reduce interface animation and transitions</small></span><input type="checkbox" checked={reduceMotion} onChange={(event) => setReduceMotion(event.target.checked)} /></label>
                  <div className="settings-control"><span><strong>UI font</strong><small>Use the operating system interface font</small></span><span className="count-badge">System default</span></div>
                  <div className="settings-control"><span><strong>Code font</strong><small>Use the operating system monospace font</small></span><span className="count-badge">System default</span></div>
                  <div className="settings-control"><span><strong>UI font size</strong><small>Base size used throughout MetaCog</small></span><span className="count-badge">14 px</span></div>
                  <div className="settings-control"><span><strong>Diff markers</strong><small>Show changes using color and +/− markers</small></span><span className="count-badge">Color</span></div>
                </div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="voice">
                <h2>Voice</h2>
                <h3>General</h3>
                <div className="settings-card settings-card-stack settings-unavailable">
                  <div className="settings-control"><span><strong>Microphone</strong><small>Voice input is not available in this build</small></span><button className="secondary" type="button" disabled>System default</button></div>
                </div>
                <h3>Voice chat</h3>
                <div className="settings-card settings-card-stack settings-unavailable">
                  <div className="settings-control"><span><strong>Voice</strong><small>A governed voice runtime has not been implemented</small></span><span className="count-badge">Unavailable</span></div>
                  <div className="settings-control"><span><strong>Voice chat hotkey</strong><small>Global voice activation remains off</small></span><span className="count-badge">Off</span></div>
                </div>
                <h3>Dictation</h3>
                <div className="settings-card settings-card-stack settings-unavailable"><div className="settings-control"><span><strong>Dictation controls</strong><small>MetaCog will not request microphone authority or save recordings until this capability is implemented and tested.</small></span><span className="count-badge">Unavailable</span></div></div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="personalization">
                <h2>Personalization</h2>
                <h3>Custom instructions</h3>
                <p className="muted">Give MetaCog durable guidance for future chats.</p>
                <textarea className="settings-instructions" aria-label="Custom instructions" disabled placeholder="Custom instructions are not connected to the runtime yet." />
                <h3>Working style</h3>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Autonomy</strong><small>Choose how independently MetaCog acts within its owner constitution.</small></span><button className="secondary" type="button" onClick={() => setSettingsTab("general")}>{userSettings?.autonomy ?? "Loading"}</button></div>
                  <div className="settings-control"><span><strong>Shadow verification</strong><small>Optional second-model checks are controlled in General settings.</small></span><span className="count-badge">{userSettings?.shadow_experiments ? "On" : "Off"}</span></div>
                  <div className="settings-control"><span><strong>Local memories</strong><small>Automatic cross-chat personal memory is not enabled. Project memory remains explicit and reviewable.</small></span><span className="count-badge">Off</span></div>
                </div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="shortcuts">
                <h2>Keyboard shortcuts</h2>
                <label className="shortcut-search"><span aria-hidden="true">⌕</span><input aria-label="Search shortcuts" value={shortcutSearch} onChange={(event) => setShortcutSearch(event.target.value)} placeholder="Search shortcuts" /></label>
                <dl className="shortcut-list">
                  {filteredShortcuts.map(([name, description, shortcut]) => <div key={`${name}-${shortcut}`}><span><dt>{name}</dt><small>{description}</small></span><dd>{shortcut.split("+").map((key) => <kbd key={`${name}-${key}`}>{key}</kbd>)}</dd></div>)}
                </dl>
                {filteredShortcuts.length === 0 && <p className="empty-state">No shortcuts found.</p>}
                <p className="form-note">Only shortcuts that work in this MetaCog build are listed. Shortcut reassignment will appear after conflict-safe binding is implemented.</p>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="usage">
                <h2>Usage &amp; billing</h2>
                <p className="muted">Review usage recorded by this local installation. Billing remains with the provider you chose.</p>
                <h3>Local usage</h3>
                <div className="settings-stats" aria-label="Local usage totals">
                  <div><strong>{proxyUsage?.usage.total_tokens.toLocaleString() ?? "—"}</strong><span>Tokens</span></div>
                  <div><strong>{proxyUsage?.call_count.toLocaleString() ?? "—"}</strong><span>Calls</span></div>
                  <div><strong>{proxyUsage?.conversation_count.toLocaleString() ?? "—"}</strong><span>Conversations</span></div>
                  <div><strong>{proxyUsage ? `$${proxyUsage.known_actual_cost_usd.toFixed(4)}` : "—"}</strong><span>Known API cost</span></div>
                </div>
                <div className="settings-section-heading"><span><strong>Codex usage</strong><small>Rolling limits reported for this signed-in ChatGPT account.</small></span><button className="secondary" type="button" disabled={!signedInWithOpenAi || codexUsageBusy} onClick={() => void refreshCodexUsage()}>{codexUsageBusy ? "Refreshing…" : "Refresh"}</button></div>
                {signedInWithOpenAi && <div className="settings-card settings-card-stack codex-usage-card">
                  <div className="settings-control"><span><strong>{openAiPlanLabel}</strong><small>{openAiProfile?.account_structure ? `${openAiProfile.account_structure} account${openAiProfile.account_role ? ` · ${openAiProfile.account_role}` : ""}` : "Account plan reported by OpenAI"}</small></span><span className="count-badge">{openAiProfile?.usage?.limit_reached ? "Limit reached" : openAiProfile?.usage?.allowed === false ? "Unavailable" : "Active"}</span></div>
                  {codexUsageWindows.map((window, index) => <div className="settings-control codex-usage-row" key={`${window.window_seconds ?? "window"}-${index}`}><span><strong>{usageWindowLabel(window, index === 0 ? "Primary limit" : "Secondary limit")}</strong><small>{usageResetLabel(window.reset_at)}</small><i className="codex-usage-meter" aria-hidden="true"><b style={{ width: `${window.remaining_percent}%` }} /></i></span><span className="usage-remaining"><strong>{window.remaining_percent}%</strong><small>left</small></span></div>)}
                  {openAiProfile?.usage?.additional_rate_limits.flatMap((limit) => [limit.primary_window, limit.secondary_window].filter((window): window is CodexUsageWindow => Boolean(window)).map((window, index) => <div className="settings-control codex-usage-row" key={`${limit.name}-${index}`}><span><strong>{limit.name}</strong><small>{usageResetLabel(window.reset_at)}</small><i className="codex-usage-meter" aria-hidden="true"><b style={{ width: `${window.remaining_percent}%` }} /></i></span><span className="usage-remaining"><strong>{window.remaining_percent}%</strong><small>left</small></span></div>))}
                  {codexUsageWindows.length === 0 && (openAiProfile?.usage?.additional_rate_limits.length ?? 0) === 0 && <div className="settings-control"><span><strong>Usage unavailable</strong><small>OpenAI did not return rolling Codex limits for this account. Refresh later or check Codex directly.</small></span></div>}
                </div>}
                {!signedInWithOpenAi && <div className="settings-empty-card"><strong>Connect ChatGPT to see Codex usage</strong><p>Sign in from Account, then MetaCog can show the account plan and rolling limits OpenAI reports.</p><button className="secondary" type="button" onClick={() => setSettingsTab("account")}>Open Account</button></div>}
                <h3>Provider billing</h3>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>ChatGPT subscription</strong><small>{signedInWithOpenAi ? `${openAiPlanLabel}. Connected through explicit OpenAI OAuth.` : "Not connected. You can sign in from Account without enabling API-key billing."}</small></span><button className="secondary" type="button" onClick={() => setSettingsTab("account")}>{signedInWithOpenAi ? "Manage" : "Connect"}</button></div>
                  <div className="settings-control"><span><strong>API and compatible providers</strong><small>API keys, OpenRouter, and OpenAI-compatible endpoints are billed by their provider. Unknown custom-model costs remain unknown.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("providers")}>Provider details</button></div>
                </div>
                <p className="form-note">MetaCog does not sell credits, collect subscription payments, or silently move subscription work to API billing.</p>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="account"><h2>Account</h2><div className="settings-card account-card"><AccountAvatar className="owner-avatar" profile={signedInWithOpenAi ? openAiProfile : undefined} avatarUrl={signedInWithOpenAi ? openAiAvatarUrl : undefined} fallback={signedInWithOpenAi ? "O" : "M"} /><div><strong>{signedInWithOpenAi ? openAiAccountName : "Connect your OpenAI subscription"}</strong><p>{signedInWithOpenAi ? `Signed in with ChatGPT${codexAccount?.email ? ` as ${codexAccount.email}` : openAiProfile?.email ? ` as ${openAiProfile.email}` : ""}.` : "A browser window will open for OpenAI sign-in and return authorization to MetaCog automatically."}</p><small>{signedInWithOpenAi ? `${openAiPlanLabel} · Using shared Codex account` : "Subscription OAuth never silently falls back to API-key billing."}</small></div>{signedInWithOpenAi ? <button className="secondary" type="button" disabled={busy} onClick={() => void logOutOfOpenAi()}>{busy ? "Disconnecting…" : "Disconnect MetaCog"}</button> : <button className="primary" type="button" disabled={busy || Boolean(login || codexLogin)} onClick={() => void startLogin("browser")}>{busy ? "Opening sign-in…" : "Sign in with ChatGPT"}</button>}</div>
                {!signedInWithOpenAi && <div className="settings-card settings-card-stack"><div className="settings-control"><span><strong>Sign in on another device</strong><small>Use Codex device-code authorization when this computer cannot open the sign-in browser.</small></span><button className="secondary" type="button" disabled={busy || Boolean(login || codexLogin)} onClick={() => void startSharedCodexLogin("device_code")}>Use device code</button></div></div>}
                {advanced && !signedInWithOpenAi && <TechnicalDetails title="Credential login" summary="API key or ChatGPT access token"><p className="form-note">API-key use is billed through the API account. Access-token login follows Codex's supported stdin flow. MetaCog sends the value once, does not save it, and clears this field after the attempt.</p><div className="settings-card settings-card-stack"><label>Credential type<select aria-label="Codex credential type" value={codexLoginMethod} disabled={busy} onChange={(event) => setCodexLoginMethod(event.target.value as "api_key" | "access_token")}><option value="api_key">OpenAI API key</option><option value="access_token">ChatGPT access token</option></select></label><label>Credential<input aria-label="Codex login credential" type="password" autoComplete="off" value={codexCredential} disabled={busy} onChange={(event) => setCodexCredential(event.target.value)} /></label><button className="primary" type="button" disabled={busy || !codexCredential} onClick={() => void startSharedCodexLogin(codexLoginMethod, codexCredential)}>Sign in</button></div></TechnicalDetails>}
                {codexAccount?.authenticated && <div className="settings-card settings-card-stack"><div className="settings-control"><span><strong>Shared Codex authentication</strong><small>MetaCog uses the global Codex account without changing its plugins or settings.</small></span><button className="secondary" type="button" disabled={busy} onClick={() => void signOutOfCodexDevice()}>Sign out of Codex on this device</button></div></div>}
                {(login || codexLogin) && <div className="oauth-card" role="status"><p>{codexLogin?.userCode ? <>Enter code <strong>{codexLogin.userCode}</strong> on the Codex device authorization page.</> : "Finish signing in in your browser. MetaCog will connect automatically."}</p><button className="link-button" type="button" onClick={() => openVerification(codexLogin?.authUrl ?? codexLogin?.verificationUrl ?? login?.verification_url ?? "")}>Open sign-in page</button><small>The authorization listener closes after 3 minutes. If it expires, the sign-in option will return here.</small></div>}
                <p className="provider-state" role="status">{providerStatus}</p>{providerError && <p className="error-message" role="alert">{providerError}</p>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="plugins">
                <div className="settings-page-heading settings-page-heading-actions"><div><h2>Plugins</h2><p>Manage installed plugins and the access they receive.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("plugins")}>Browse directory</button></div>
                <div className="settings-segmented" role="tablist" aria-label="Plugin categories"><button type="button" role="tab" aria-selected="true">Installed <span>{plugins.length}</span></button><button type="button" role="tab" aria-selected="false" onClick={() => navigateSurface("plugins")}>Directory</button><button type="button" role="tab" aria-selected="false" onClick={() => navigateSurface("mcp")}>MCPs</button></div>
                <div className="settings-inline-toolbar"><strong>Installed</strong><span>{plugins.filter((plugin) => plugin.installation.enabled).length} active</span></div>
                {plugins.length === 0 ? <div className="settings-empty-card"><strong>No plugins installed</strong><p>Browse the directory or install a reviewed local plugin package.</p><button className="secondary" type="button" onClick={() => navigateSurface("plugins")}>Open plugin directory</button></div> : <div className="settings-integration-list" aria-label="Installed plugins">
                  {plugins.map((plugin) => {
                    const manifest = plugin.active_revision.manifest;
                    const source = plugin.active_revision.source.kind === "local_import" ? "Personal plugin" : "Workspace registry";
                    return <div className="settings-integration-row" key={manifest.id}><span className="settings-app-icon" aria-hidden="true">{manifest.display_name.slice(0, 1).toLocaleUpperCase()}</span><span className="settings-integration-copy"><strong>{manifest.display_name}</strong><small>{source} · {manifest.version}</small></span><label className="settings-switch"><span className="sr-only">Enable {manifest.display_name}</span><input type="checkbox" checked={plugin.installation.enabled} disabled={pluginBusy} onChange={() => void togglePlugin(plugin)} /><i aria-hidden="true" /></label></div>;
                  })}
                </div>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="browser">
                <div className="settings-page-heading"><h2>Browser</h2><p>Manage MetaCog's governed browser sessions and site boundaries.</p></div>
                <div className="settings-feature-card"><span className="settings-feature-icon" aria-hidden="true">▣</span><span><strong>Browser automation</strong><small>{browserCapability?.detail ?? "Checking browser capability on this computer…"}</small></span><label className="settings-switch"><span className="sr-only">Browser automation availability</span><input type="checkbox" checked={browserCapability?.availability === "available"} readOnly disabled /><i aria-hidden="true" /></label></div>
                <div className="settings-section-heading"><strong>Sessions</strong><button className="secondary" type="button" onClick={() => navigateSurface("platform")}>Manage</button></div>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Active browser sessions</strong><small>Sessions are limited to explicitly allowed origins and expire automatically.</small></span><span className="count-badge">{activeBrowserSessions.length}</span></div>
                  <div className="settings-control"><span><strong>Default visibility</strong><small>New governed sessions begin privately unless visible control is explicitly requested.</small></span><span className="count-badge">Private</span></div>
                  <div className="settings-control"><span><strong>Approval</strong><small>Browser authority follows the owner autonomy policy and reviewed task capabilities.</small></span><button className="secondary" type="button" onClick={() => setSettingsTab("general")}>Review policy</button></div>
                </div>
                <div className="settings-section-heading"><span><strong>Site permissions</strong><small>Allowed sites are attached to individual governed sessions.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("platform")}>Add session</button></div>
                <div className="settings-empty-card"><p>{activeBrowserSessions.length === 0 ? "No active site permissions." : `${activeBrowserSessions.length} governed browser session${activeBrowserSessions.length === 1 ? "" : "s"} currently hold explicit site boundaries.`}</p></div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="computer">
                <div className="settings-page-heading"><h2>Computer use</h2><p>Manage how MetaCog uses other applications on this computer.</p></div>
                <div className="settings-section-heading"><strong>Control</strong><span>{activeComputerSessions.length} active</span></div>
                <div className="settings-integration-list" aria-label="Computer use controls">
                  <div className="settings-integration-row"><span className="settings-app-icon settings-app-icon-gradient" aria-hidden="true">⌁</span><span className="settings-integration-copy"><strong>Governed app control</strong><small>{computerCapability?.detail ?? "Checking computer-use capability…"}</small></span><label className="settings-switch"><span className="sr-only">Computer use availability</span><input type="checkbox" checked={computerCapability?.availability === "available"} readOnly disabled /><i aria-hidden="true" /></label></div>
                  <div className="settings-integration-row"><span className="settings-app-icon" aria-hidden="true">◎</span><span className="settings-integration-copy"><strong>Screen reading</strong><small>Captured frames remain scoped to an explicit session and target region.</small></span><span className="count-badge">Governed</span></div>
                  <div className="settings-integration-row"><span className="settings-app-icon" aria-hidden="true">⌨</span><span className="settings-integration-copy"><strong>Input control</strong><small>Mouse, keyboard, and clipboard capabilities must be granted per session.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("platform")}>Manage</button></div>
                </div>
                <div className="settings-section-heading"><strong>Always-allowed apps</strong></div>
                <div className="settings-empty-card"><p>None yet</p></div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="hooks">
                <div className="settings-page-heading settings-page-heading-actions">
                  <div><h2>Hooks</h2><p>Review lifecycle automations before they run. MetaCog trusts only the exact definition shown here; any later change requires another review.</p></div>
                  <button className="secondary" type="button" disabled={codexHookBusy !== null} onClick={() => void refreshCodexHooks()}><RotateCw aria-hidden="true" />{codexHookBusy === "refresh" ? "Refreshing…" : "Refresh"}</button>
                </div>
                {codexHookError && <p className="error-message" role="alert">{codexHookError}</p>}
                {(codexStableDiscovery?.hooks ?? []).flatMap((entry) => entry.warnings.map((warning) => <div className="warning-box" role="status" key={`${entry.cwd}-warning-${warning}`}><strong>Hook warning</strong><span>{warning}</span></div>))}
                {(codexStableDiscovery?.hooks ?? []).flatMap((entry) => entry.errors.map((error) => <div className="warning-box" role="alert" key={`${entry.cwd}-error-${error.path}-${error.message}`}><strong>Could not load hooks from {error.path}</strong><span>{error.message}</span></div>))}
                {discoveredHookCount === 0
                  ? <div className="settings-empty-card"><Workflow aria-hidden="true" /><strong>No hooks configured</strong><p>{workspaceProject ? `No user, project, plugin, or managed hooks were found for ${workspaceProjectName}.` : "Choose a project to include its project hooks, or add a user hook in Codex configuration."}</p></div>
                  : <div className="codex-hook-list" aria-label="Configured Codex hooks">{(codexStableDiscovery?.hooks ?? []).flatMap((entry) => entry.hooks.map((hook) => {
                    const trusted = hook.trustStatus === "trusted" || hook.trustStatus === "managed";
                    const handler = hook.handlerType === "command" ? hook.command ?? "Command hook" : hook.handlerType === "mcpTool" ? `${hook.server ?? "MCP"} / ${hook.tool ?? "tool"}` : hook.handlerType;
                    const action: "trust" | "enable" | "disable" = !trusted ? "trust" : hook.enabled ? "disable" : "enable";
                    return <article className={`codex-hook-row trust-${hook.trustStatus}`} key={`${entry.cwd}-${hook.key}`}>
                      <span className="settings-feature-icon" aria-hidden="true"><Workflow /></span>
                      <div className="codex-hook-copy">
                        <span className="codex-hook-title"><strong>{hook.eventName}</strong><span className={`count-badge hook-trust-${hook.trustStatus}`}>{hook.trustStatus}</span>{!hook.enabled && <span className="count-badge">disabled</span>}{hook.isManaged && <span className="count-badge">managed</span>}</span>
                        <small>{handler}</small>
                        <small>{hook.source} · {hook.matcher ?? "all matching events"}</small>
                        <TechnicalDetails title="Technical details" summary={hook.sourcePath}>
                          <dl className="hook-property-grid">
                            <div><dt>Source</dt><dd>{hook.sourcePath}</dd></div>
                            <div><dt>Handler</dt><dd>{hook.handlerType}</dd></div>
                            <div><dt>Timeout</dt><dd>{hook.timeoutSec}s</dd></div>
                            <div><dt>Async</dt><dd>{hook.async ? "Yes" : "No"}</dd></div>
                            <div><dt>Definition hash</dt><dd><code>{hook.currentHash}</code></dd></div>
                            <div><dt>Hook key</dt><dd><code>{hook.key}</code></dd></div>
                          </dl>
                        </TechnicalDetails>
                      </div>
                      {hook.isManaged
                        ? <span className="count-badge">Policy controlled</span>
                        : <button className={action === "trust" ? "primary" : "secondary"} type="button" disabled={codexHookBusy !== null} onClick={() => void changeCodexHook(entry, hook, action)}>{codexHookBusy === hook.key ? "Updating…" : action === "trust" ? "Review and trust" : action === "enable" ? "Enable" : "Disable"}</button>}
                    </article>;
                  }))}</div>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="connections"><h2>Connections</h2><div className="settings-card"><div><strong>Providers and tools</strong><p>Manage model providers, ChatGPT subscription access, MCP servers, and remote hosts.</p></div><div className="settings-card-actions"><button className="secondary" type="button" onClick={() => navigateSurface("providers")}>Providers</button><button className="secondary" type="button" onClick={() => navigateSurface("mcp")}>Tools</button><button className="secondary" type="button" onClick={() => navigateSurface("ssh")}>SSH</button></div></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="git">
                <div className="settings-page-heading"><h2>Git</h2><p>Choose defaults for governed repository workflows.</p></div>
                <div className="settings-card account-card">
                  <span className="settings-app-icon" aria-hidden="true">⑂</span>
                  <div><strong>{githubConnection?.connected ? githubConnection.account?.name ?? githubConnection.account?.login ?? "GitHub connected" : "Connect GitHub"}</strong><p>{githubConnection?.connected ? `Signed in as @${githubConnection.account?.login}. MetaCog can list repositories and perform only explicit repository actions.` : githubConnection?.configured ? "Sign in in your browser to grant MetaCog repository access through its GitHub App." : "This build needs an MetaCog GitHub App client ID before browser sign-in can start."}</p><small>Tokens are stored in the operating-system credential vault, never the project or SQLite database.</small></div>
                  {githubConnection?.connected
                    ? <button className="secondary" type="button" disabled={githubBusy} onClick={() => void signOutOfGitHub()}>Disconnect</button>
                    : <button className="primary" type="button" disabled={githubBusy || githubConnection?.configured === false} onClick={() => void signInToGitHub()}>{githubBusy ? "Connecting…" : "Sign in to GitHub"}</button>}
                </div>
                {githubError && <p className="error-message" role="alert">{githubError}</p>}
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Branch prefix</strong><small>Prefix used when MetaCog creates a task branch.</small></span><span className="settings-value-field">codex/</span></div>
                  <div className="settings-control"><span><strong>Local Git by default</strong><small>Newly attached plain folders receive an empty local baseline commit. Existing files are not staged automatically.</small></span><span className="count-badge">On</span></div>
                  {workspaceProject && !workspaceProject.repository.is_git && <div className="settings-control"><span><strong>Selected project is still a folder</strong><small>Initialize its local version history now so managed agents can use isolated worktrees.</small></span><button className="secondary" type="button" disabled={githubBusy} onClick={() => void initializeSelectedProjectGit()}>Initialize local Git</button></div>}
                  <div className="settings-control"><span><strong>Pull request review</strong><small>Reviews are read-only and findings remain separate from fixes.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("pull-requests")}>Open reviews</button></div>
                  <div className="settings-control"><span><strong>Force push</strong><small>MetaCog does not enable destructive history rewriting by default.</small></span><span className="count-badge">Off</span></div>
                  <div className="settings-control"><span><strong>Review delivery</strong><small>Keep review findings in the current task when possible.</small></span><span className="count-badge">Inline</span></div>
                </div>
                {githubConnection?.connected && <>
                  <div className="settings-section-heading"><span><strong>Publish selected project</strong><small>Create an empty repository and link it as origin. Pushing remains a separate explicit action.</small></span></div>
                  <div className="settings-card settings-card-stack">
                    <label><span>Repository name</span><input aria-label="GitHub repository name" value={githubRepositoryName} onChange={(event) => setGitHubRepositoryName(event.target.value)} placeholder={workspaceProjectName.replace(/\s+/g, "-").toLowerCase()} /></label>
                    <label className="settings-control"><span><strong>Private repository</strong><small>Recommended until the owner reviews the first push.</small></span><input type="checkbox" checked={githubRepositoryPrivate} onChange={(event) => setGitHubRepositoryPrivate(event.target.checked)} /></label>
                    <button className="primary" type="button" disabled={githubBusy || !workspaceProject || !workspaceProject.repository.is_git || !githubRepositoryName.trim()} onClick={() => void createAndLinkGitHubRepository()}>Create and link repository</button>
                    {!workspaceProject && <small>Select a project before creating or linking a repository.</small>}
                  </div>
                  <div className="settings-section-heading"><span><strong>Accessible repositories</strong><small>{githubRepositories.length} visible to this GitHub authorization.</small></span><button className="secondary" type="button" disabled={githubBusy} onClick={() => void refreshGitHub()}>Refresh</button></div>
                  <div className="settings-integration-list" aria-label="GitHub repositories">
                    {githubRepositories.slice(0, 25).map((repository) => <div className="settings-integration-row" key={repository.id}><span className="settings-app-icon" aria-hidden="true">⑂</span><span className="settings-integration-copy"><strong>{repository.full_name}</strong><small>{repository.private ? "Private" : "Public"} · {repository.default_branch}</small></span><button className="secondary" type="button" disabled={githubBusy || !workspaceProject?.repository.is_git} onClick={() => void linkExistingGitHubRepository(repository)}>Link</button></div>)}
                    {githubRepositories.length === 0 && <div className="settings-empty-card"><p>No accessible repositories were returned.</p></div>}
                  </div>
                </>}
                <div className="settings-section-heading"><span><strong>Commit instructions</strong><small>Project and repository instructions remain the source of commit guidance.</small></span></div>
                <textarea className="settings-instructions" aria-label="Commit instructions" disabled placeholder="Custom commit guidance is not connected yet." />
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="environments">
                <div className="settings-page-heading settings-page-heading-actions"><div><h2>Environments</h2><p>Local environments tell MetaCog how each attached project is set up.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("projects")}>Add project</button></div>
                <div className="settings-section-heading"><strong>Select a project</strong><span>{projects.length} attached</span></div>
                {projects.length === 0 ? <div className="settings-empty-card"><strong>No projects attached</strong><p>Add a project to discover its repository, languages, tools, and runnable commands.</p><button className="secondary" type="button" onClick={() => navigateSurface("projects")}>Add project</button></div> : <div className="settings-project-list" aria-label="Project environments">{projects.map((project) => <button type="button" onClick={() => navigateSurface("projects")} key={project.id}><span className="settings-app-icon" aria-hidden="true">▤</span><span><strong>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.project_id}</strong><small>{project.repository.is_git ? `${project.repository.branch ?? "Git repository"}${project.repository.dirty ? " · changes present" : ""}` : "Local project"}</small></span><i aria-hidden="true">＋</i></button>)}</div>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="worktrees">
                <div className="settings-page-heading settings-page-heading-actions"><div><h2>Worktrees</h2><p>Review the complete durable inventory of isolated coding work, including retained work from conversations that are not currently open.</p></div><button className="secondary" type="button" onClick={() => void refreshManagedWorktrees()}>Refresh</button></div>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Worktree root</strong><small>Managed worktrees will use an owner-controlled local directory.</small></span><span className="settings-value-field">Runtime managed</span></div>
                  <div className="settings-control"><span><strong>Cleanup policy</strong><small>Integrated worktrees are removed; failed, cancelled, conflicting, and unapplied work is retained.</small></span><span className="count-badge">Safe retention</span></div>
                  <div className="settings-control"><span><strong>Isolation policy</strong><small>Scheduled and coding work must remain within reviewed project boundaries.</small></span><span className="count-badge">Governed</span></div>
                </div>
                <div className="settings-section-heading"><strong>Managed worktrees</strong><span>{managedConversationWorktrees.length}</span></div>
                {managedConversationWorktrees.length === 0 ? <div className="settings-empty-card"><p>No managed coding worktrees are retained.</p></div> : <div className="settings-card settings-card-stack" aria-label="Managed worktrees">{managedConversationWorktrees.map(({ worktree, conversationId, delegation }) => {
                  const blockingExclusions = worktree.baseline_exclusions?.filter((entry) => entry.review_blocker).length ?? 0;
                  const journal = worktree.integration_journal;
                  return <div className="settings-control" key={worktree.id}><span><strong>{worktree.branch}</strong><small>{worktree.state}{worktree.retained_reason ? ` · ${worktree.retained_reason}` : ""} · {(worktree.storage_bytes / (1024 * 1024)).toFixed(1)} MiB</small><small>{(worktree.workspace_mode ?? "git_worktree").replaceAll("_", " ")} · baseline {(worktree.baseline_confidence ?? "verified").replaceAll("_", " ")}{blockingExclusions > 0 ? ` · ${blockingExclusions} review blocker${blockingExclusions === 1 ? "" : "s"}` : ""}{journal ? ` · integration ${journal.state.replaceAll("_", " ")}` : ""}</small></span><span className="settings-inline-actions"><button className="secondary" type="button" onClick={() => openChat(conversationId)}>Review</button>{worktree.state !== "removed" && delegation && !["requested", "preparing", "running", "waiting", "integrating"].includes(delegation.state) && <button className="secondary danger" type="button" onClick={() => void discardConversationDelegation(conversationId, delegation.id, worktree.revision).then((view) => { setConversationViews((current) => ({ ...current, [conversationId]: view })); return refreshManagedWorktrees(); }).catch((error) => setTaskError(errorMessage(error)))}>Discard</button>}</span></div>;
                })}</div>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="archived"><h2>Archived chats</h2>{archivedConversations.length === 0 ? <div className="settings-card"><div><strong>No archived chats</strong><p>Chats you archive will appear here and can be restored at any time.</p></div></div> : <div className="settings-card archived-chat-list">{archivedConversations.map((conversation) => <div className="settings-row" key={conversation.id}><div><strong>{conversation.title}</strong><p>{conversation.project_id ? projects.find((project) => project.project_id === conversation.project_id)?.root ?? "Project chat" : "General chat"}</p></div><button className="secondary" type="button" disabled={chatMutationBusy} onClick={() => void setChatArchived(conversation.id, false)}>Restore</button></div>)}</div>}</div>
            </div>
          </div>
        </section>
      </main>
      {commandDialog && <div className="slash-command-dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setCommandDialog(null); }}>
        <section className="slash-command-dialog" role="dialog" aria-modal="true" aria-label={commandDialog === "notice" ? commandNotice.title : `Slash command ${commandDialog}`}>
          <header><div><Command aria-hidden="true" /><strong>{commandDialog === "notice" ? commandNotice.title : `/${commandDialog}`}</strong></div><button type="button" aria-label="Close command dialog" onClick={() => setCommandDialog(null)}><X aria-hidden="true" /></button></header>
          {commandDialog === "notice" && <p>{commandNotice.detail}</p>}
          {commandDialog === "status" && <div className="slash-command-properties">
            <div><span>Chat</span><strong>{selectedChatId ?? "Not started"}</strong></div>
            <div><span>Project</span><strong>{currentWorkspaceProject() ? projectDisplayName(currentWorkspaceProject()!) : "None"}</strong></div>
            <div><span>Model</span><strong>{newChatRunPreferences.model_label ?? newChatRunPreferences.model_id ?? "Automatic"}</strong></div>
            <div><span>Context</span><strong>{visibleConversation ? `${visibleConversation.turns.length} turns` : "No turns"}</strong></div>
            <div><span>Plan</span><strong>{codexRateLimits?.plan_type ?? codexAccount?.plan_type ?? "Unavailable"}</strong></div>
            <div><span>Primary usage</span><strong>{codexRateLimits?.primary ? `${codexRateLimits.primary.remaining_percent}% remaining` : "Unavailable"}</strong></div>
            <div><span>Secondary usage</span><strong>{codexRateLimits?.secondary ? `${codexRateLimits.secondary.remaining_percent}% remaining` : "Unavailable"}</strong></div>
          </div>}
          {commandDialog === "model" && <div className="slash-command-choices" role="listbox" aria-label="Models">
            {!selectedCommandLocalProvider && <button type="button" role="option" aria-selected={!newChatRunPreferences.model_id} onClick={() => { setNewChatRunPreferences((current) => ({ ...current, provider_id: undefined, model_provider: undefined, model_id: undefined, model_label: undefined })); setCommandDialog(null); }}><span><strong>Automatic</strong><small>Use the account default</small></span>{!newChatRunPreferences.model_id && <Check aria-hidden="true" />}</button>}
            {selectedCommandLocalProvider
              ? commandLocalModels.map((model) => <button type="button" role="option" aria-selected={newChatRunPreferences.model_id === model} key={model} onClick={() => { setNewChatRunPreferences((current) => ({ ...current, provider_id: selectedCommandLocalProvider.id, model_provider: selectedCommandLocalProvider.id as "ollama" | "lmstudio", model_id: model, model_label: model, service_tier: undefined, speed: "standard" })); setCommandDialog(null); }}><span><strong>{model}</strong><small>{selectedCommandLocalProvider.display_name}</small></span>{newChatRunPreferences.model_id === model && <Check aria-hidden="true" />}</button>)
              : codexModels.filter((model) => !model.hidden).map((model) => <button type="button" role="option" aria-selected={newChatRunPreferences.model_id === model.id} key={model.id} onClick={() => { setNewChatRunPreferences((current) => ({ ...current, provider_id: "codex", model_provider: undefined, model_id: model.id, model_label: model.displayName, reasoning_effort: model.defaultReasoningEffort as ReasoningEffort, service_tier: model.defaultServiceTier })); setCommandDialog(null); }}><span><strong>{model.displayName}</strong><small>{model.description}</small></span>{newChatRunPreferences.model_id === model.id && <Check aria-hidden="true" />}</button>)}
          </div>}
          {commandDialog === "reasoning" && <div className="slash-command-choices" role="listbox" aria-label="Reasoning effort">
            {(selectedCommandLocalProvider ? ["low", "medium", "high"] : codexModels.find((model) => model.id === newChatRunPreferences.model_id)?.supportedReasoningEfforts.map((item) => item.reasoningEffort) ?? ["low", "medium", "high", "xhigh"]).map((effort) => <button type="button" role="option" aria-selected={newChatRunPreferences.reasoning_effort === effort} key={effort} onClick={() => { setNewChatRunPreferences((current) => ({ ...current, reasoning_effort: effort as ReasoningEffort })); setCommandDialog(null); }}><span><strong>{effort === "xhigh" ? "Extra high" : effort.charAt(0).toUpperCase() + effort.slice(1)}</strong><small>Model-supported reasoning level</small></span>{newChatRunPreferences.reasoning_effort === effort && <Check aria-hidden="true" />}</button>)}
          </div>}
          {commandDialog === "personality" && <div className="slash-command-choices" role="listbox" aria-label="Response personality">
            {([
              ["default", "Default", "Use the normal MetaCog response style"],
              ["concise", "Concise", "Lead with the outcome and keep explanations compact"],
              ["friendly", "Friendly", "Warm and conversational while remaining precise"],
              ["professional", "Professional", "Polished, direct, and businesslike"]
            ] as const).map(([id, label, description]) => { const selected = (newChatRunPreferences.personality ?? "default") === id; return <button type="button" role="option" aria-selected={selected} key={id} onClick={() => { setNewChatRunPreferences((current) => ({ ...current, personality: id === "default" ? undefined : id })); setCommandDialog(null); }}><span><strong>{label}</strong><small>{description}</small></span>{selected && <Check aria-hidden="true" />}</button>; })}
          </div>}
          {commandDialog === "feedback" && <form onSubmit={(event) => { event.preventDefault(); const details = feedbackIncludeDiagnostics ? `\n\nMetaCog diagnostics:\n- Version: ${codexHarnessStatus?.bundled_version ?? "unknown"}\n- Process: ${codexHarnessStatus?.process_state ?? "unknown"}\n- Protocol: ${codexHarnessStatus?.protocol_version ?? "unknown"}` : ""; const url = `https://github.com/fourtytwo42/Metacognitive-Governor/issues/new?title=${encodeURIComponent("MetaCog feedback")}&body=${encodeURIComponent(`${feedbackDraft.trim()}${details}`)}`; void openExternalUrl(url); setCommandDialog(null); }}>
            <label><span>Feedback</span><textarea aria-label="Feedback" required value={feedbackDraft} onChange={(event) => setFeedbackDraft(event.target.value)} placeholder="What happened, and what should MetaCog do instead?" /></label>
            <label className="slash-command-checkbox"><input type="checkbox" checked={feedbackIncludeDiagnostics} onChange={(event) => setFeedbackIncludeDiagnostics(event.target.checked)} /><span>Include non-secret version and runtime status</span></label>
            <div className="slash-command-dialog-actions"><button className="secondary" type="button" onClick={() => setCommandDialog(null)}>Cancel</button><button className="primary" type="submit" disabled={!feedbackDraft.trim()}>Continue to GitHub</button></div>
          </form>}
        </section>
      </div>}
      {workspaceSidePanelOpen && workspacePanelsVisible && <aside className="workspace-side-panel" aria-label="Workspace side panel">
        <div className="workspace-side-resizer" role="separator" aria-orientation="vertical" aria-label="Resize side panel" aria-valuemin={280} aria-valuemax={window.innerWidth <= 820 ? Math.max(320, window.innerWidth - 48) : Math.max(320, window.innerWidth - (sidebarOpen ? sidebarWidth : 0) - 320)} aria-valuenow={workspaceSideWidth} tabIndex={0} onPointerDown={beginSidePanelResize} onDoubleClick={() => setWorkspaceSideWidth(520)} onKeyDown={(event) => {
          if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
          event.preventDefault();
          const maximum = window.innerWidth <= 820 ? Math.max(320, window.innerWidth - 48) : Math.max(320, window.innerWidth - (sidebarOpen ? sidebarWidth : 0) - 320);
          setWorkspaceSideWidth((current) => Math.min(maximum, Math.max(280, current + (event.key === "ArrowLeft" ? 24 : -24))));
        }} />
        <header>
          <div className="workspace-panel-tabs" role="tablist" aria-label="Side panel tools">
            {([
              { tab: "browser", label: "Browser", Icon: Globe2 },
              { tab: "files", label: "Files", Icon: Files },
              { tab: "activity", label: "Activity", Icon: Activity }
            ] as Array<{ tab: WorkspacePanelTab; label: string; Icon: typeof Globe2 }>).map(({ tab, label, Icon }) => <button type="button" role="tab" aria-selected={workspaceSidePanelTab === tab} className={workspaceSidePanelTab === tab ? "active" : ""} onClick={() => setWorkspaceSidePanelTab(tab)} key={tab}><Icon aria-hidden="true" />{label}</button>)}
          </div>
          <button type="button" aria-label="Close side panel" title="Close side panel" onClick={() => setWorkspaceSidePanelOpen(false)}><X aria-hidden="true" /></button>
        </header>
        <div className={`workspace-side-content${workspaceSidePanelTab === "browser" && siteBrowserTabs.length > 0 ? " browser-workspace" : ""}`}>
          {workspaceSidePanelTab === "browser" && <section className={`side-browser${siteBrowserTabs.length > 0 ? " tabbed-browser" : ""}`} aria-label="Browser">
            {siteBrowserTabs.length > 0 ? <>
              <div className="browser-tab-strip" role="tablist" aria-label="Browser tabs">
                {siteBrowserTabs.map((tab) => <div className={`browser-tab${tab.id === activeSiteBrowserTabId ? " active" : ""}`} role="tab" aria-selected={tab.id === activeSiteBrowserTabId} tabIndex={tab.id === activeSiteBrowserTabId ? 0 : -1} onClick={() => selectSiteBrowserTab(tab)} key={tab.id}>
                  <Globe2 aria-hidden="true" /><span title={tab.title}>{tab.title}</span><button type="button" aria-label={`Close ${tab.title}`} onClick={(event) => { event.stopPropagation(); closeSiteBrowserTab(tab.id); }}><X aria-hidden="true" /></button>
                </div>)}
                <button className="browser-new-tab" type="button" aria-label="New browser tab" title="New tab" onClick={createSiteBrowserTab}><Plus aria-hidden="true" /></button>
              </div>
              <form className="browser-toolbar" onSubmit={(event) => { event.preventDefault(); navigateSiteBrowserPanel(); }}>
                <button type="button" aria-label="Back" title="Back" disabled={!activeSiteBrowserTab?.url} onClick={() => activeSiteBrowserTab && void controlSiteInAppBrowser(activeSiteBrowserTab.id, "back").catch((error) => setPlatformError(errorMessage(error)))}><ArrowLeft aria-hidden="true" /></button>
                <button type="button" aria-label="Forward" title="Forward" disabled={!activeSiteBrowserTab?.url} onClick={() => activeSiteBrowserTab && void controlSiteInAppBrowser(activeSiteBrowserTab.id, "forward").catch((error) => setPlatformError(errorMessage(error)))}><ArrowRight aria-hidden="true" /></button>
                <button type="button" aria-label="Reload" title="Reload" disabled={!activeSiteBrowserTab?.url} onClick={() => activeSiteBrowserTab && void controlSiteInAppBrowser(activeSiteBrowserTab.id, "reload").catch((error) => setPlatformError(errorMessage(error)))}><RotateCw aria-hidden="true" /></button>
                <input aria-label="Address" type="url" value={sideBrowserUrl} onChange={(event) => setSideBrowserUrl(event.target.value)} placeholder="Enter a URL" />
                <button type="submit" aria-label="Go" title="Go"><ExternalLink aria-hidden="true" /></button>
              </form>
            </> : <>
              <div className="side-panel-heading"><div><strong>Governed browser</strong><small>Each session is restricted to its allowed site.</small></div><button type="button" onClick={() => navigateSurface("platform")}>Manage</button></div>
              <form onSubmit={(event) => { event.preventDefault(); void startSideBrowser(); }}>
                <input aria-label="Browser URL" type="url" value={sideBrowserUrl} onChange={(event) => setSideBrowserUrl(event.target.value)} placeholder="https://example.com" />
                <button className="primary" type="submit" disabled={platformBusy}>{platformBusy ? "Opening…" : "Open"}</button>
              </form>
            </>}
            {platformError && <p className="workspace-panel-error" role="alert">{platformError}</p>}
            {activeSiteBrowserTab ? activeSiteBrowserTab.url
              ? <EmbeddedSiteBrowser tabId={activeSiteBrowserTab.id} url={activeSiteBrowserTab.url} onError={setPlatformError} />
              : <div className="browser-start-page"><Globe2 aria-hidden="true" /><strong>Start browsing</strong><p>Enter a URL to open a page</p></div>
              : <><div className="side-session-list">
              {activeBrowserSessions.length === 0 && <div className="workspace-panel-empty"><Globe2 aria-hidden="true" /><strong>No browser sessions</strong><p>Enter a URL to start a private, governed session.</p></div>}
              {activeBrowserSessions.map((session) => {
                const browser = platformBrowsers[session.id];
                const selected = selectedBrowserSession?.id === session.id;
                return <article className={`side-session${selected ? " selected" : ""}`} key={session.id}>
                  <div><span className="session-favicon" aria-hidden="true"><Globe2 /></span><span><strong>{browser?.title || session.display_name}</strong><small>{browser?.safe_url || session.allowed_origins.join(", ")}</small></span><i className={`session-state state-${session.state}`} title={session.state} /></div>
                  {session.state === "requested" && <button type="button" disabled={platformBusy} onClick={() => void attachPlatformBrowser(session.id, false)}>Start</button>}
                  {session.state === "active" && <form onSubmit={(event) => { event.preventDefault(); void navigatePlatformBrowser(session.id); }}><input aria-label={`URL for ${session.display_name}`} value={platformBrowserUrls[session.id] ?? browser?.safe_url ?? ""} onChange={(event) => setPlatformBrowserUrls((current) => ({ ...current, [session.id]: event.target.value }))} /><button type="submit" disabled={platformBusy}>Go</button></form>}
                  {session.state === "active" && !selected && <button type="button" onClick={() => setSelectedBrowserSessionId(session.id)}>View</button>}
                  <button className="link-button" type="button" disabled={platformBusy} onClick={() => void closePlatform(session.id)}>Close session</button>
                </article>;
              })}
            </div>
            {selectedBrowserSession && selectedBrowserPreviewUrl && <GovernedBrowserPreview
              sessionId={selectedBrowserSession.id}
              url={selectedBrowserPreviewUrl}
              title={selectedBrowserObservation?.title || selectedBrowserSession.display_name}
            />}</>}
          </section>}
          {workspaceSidePanelTab === "files" && <section className="side-files" aria-label="Project files">
            <div className="side-panel-heading"><div><strong>{workspaceProjectName}</strong><small>{workspaceProject?.repository.is_git ? workspaceProject.repository.branch ?? "Git repository" : workspaceProject ? "Local project" : "Choose a project"}</small></div></div>
            {workspaceProject ? <>
              <button className="side-project-root" type="button" onClick={() => void openCurrentWorkspace("file_explorer")}><FolderOpen aria-hidden="true" /><span><strong>Project root</strong><small title={workspaceProject.root}>{workspaceProject.root}</small></span><ExternalLink aria-hidden="true" /></button>
              <div className="side-detail-list"><div><span>Repository</span><strong>{workspaceProject.repository.is_git ? "Git" : "Folder"}</strong></div><div><span>Branch</span><strong>{workspaceProject.repository.branch ?? "—"}</strong></div><div><span>Changes</span><strong>{workspaceProject.repository.dirty ? "Present" : "Clean"}</strong></div></div>
              <button className="secondary" type="button" onClick={() => { setWorkspaceBottomPanelOpen(true); if (workspaceProject.root) setTerminalWorkingDirectory(workspaceProject.root); }}>Open project terminal</button>
            </> : <div className="workspace-panel-empty"><FolderOpen aria-hidden="true" /><strong>No project selected</strong><p>Choose a project or open one from the left sidebar.</p></div>}
          </section>}
          {workspaceSidePanelTab === "activity" && <section className="side-activity" aria-label="Task activity">
            <div className="side-panel-heading"><div><strong>Task activity</strong><small>Current state and durable evidence.</small></div></div>
            {workspaceTask ? <>
              <article className="side-task-summary"><span className={`task-state state-${workspaceTask.state}`}>{workspaceTask.state.replaceAll("_", " ")}</span><strong>{workspaceTask.goal}</strong><small>{workspaceProjectName}</small></article>
              <button className="secondary" type="button" disabled={taskBusy} onClick={() => void toggleTaskHistory(workspaceTask.id)}>{taskHistory[workspaceTask.id] ? "Refresh evidence" : "Load evidence"}</button>
              {taskHistory[workspaceTask.id] && <div className="side-event-list">{taskHistory[workspaceTask.id]!.events.slice(-12).reverse().map((event) => <div key={event.sequence}><span>{event.kind.replaceAll("_", " ")}</span><time>{new Date(event.occurred_at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</time></div>)}</div>}
            </> : <div className="workspace-panel-empty"><Activity aria-hidden="true" /><strong>No task selected</strong><p>Open a chat to inspect its live activity.</p></div>}
          </section>}
        </div>
      </aside>}
      {workspaceBottomPanelOpen && workspacePanelsVisible && <section className="workspace-bottom-panel" aria-label="Terminal">
        <div className="workspace-bottom-resizer" role="separator" aria-orientation="horizontal" aria-label="Resize terminal" onMouseDown={beginBottomPanelResize} />
        <header><div className="terminal-tabs"><button className="active" type="button"><Terminal aria-hidden="true" />Terminal</button><button type="button" aria-label="Clear terminal" title="Clear terminal" onClick={() => setTerminalEntries([])}><Plus aria-hidden="true" /></button></div><div><button type="button" disabled={!workspaceProject} aria-label="Use project root" title="Use project root" onClick={() => workspaceProject && setTerminalWorkingDirectory(workspaceProject.root)}><Home aria-hidden="true" /></button><button type="button" aria-label="Close terminal" title="Close terminal" onClick={() => setWorkspaceBottomPanelOpen(false)}><X aria-hidden="true" /></button></div></header>
        <div className="terminal-output" ref={terminalOutputRef} role="log" aria-live="polite">
          {terminalEntries.length === 0 && <div className="terminal-welcome"><strong>{navigator.userAgent.includes("Windows") ? "Windows PowerShell" : "Project shell"}</strong><span>Commands run in the selected project. Type <code>clear</code> to clear this panel.</span></div>}
          {terminalEntries.map((entry) => <div className="terminal-entry" key={entry.id}><div><span className="terminal-prompt">{entry.workingDirectory}&gt;</span> <span>{entry.command}</span></div>{entry.result?.output && <pre>{entry.result.output}</pre>}{entry.result && entry.result.exit_code !== 0 && <small>Process exited with code {entry.result.exit_code}</small>}{entry.error && <p role="alert">{entry.error}</p>}</div>)}
          {terminalBusy && <div className="terminal-running" role="status">Running…</div>}
        </div>
        <form className="terminal-command-line" onSubmit={(event) => { event.preventDefault(); void submitTerminalCommand(); }}><span aria-hidden="true">›</span><input ref={terminalInputRef} aria-label="Terminal command" autoComplete="off" spellCheck={false} value={terminalCommand} disabled={terminalBusy} onChange={(event) => setTerminalCommand(event.target.value)} onKeyDown={(event) => { if (event.key === "ArrowUp" && !terminalCommand) { event.preventDefault(); setTerminalCommand(terminalEntries.at(-1)?.command ?? ""); } }} placeholder={terminalWorkingDirectory || "Choose a project to begin"} /><button className="sr-only" type="submit">Run command</button></form>
      </section>}
    </div>
    </div>
  );
}
