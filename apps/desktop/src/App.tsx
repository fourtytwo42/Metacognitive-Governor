import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type MouseEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const CHAT_FOLLOW_THRESHOLD_PX = 96;

export function isChatViewportNearBottom(
  viewport: Pick<HTMLElement, "scrollHeight" | "scrollTop" | "clientHeight">,
  threshold = CHAT_FOLLOW_THRESHOLD_PX
) {
  return viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <= threshold;
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

import {
  approveRequest,
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
  createTask,
  cancelAgentRun,
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
  listOpenInApplications,
  listOrganizationPublishers,
  listOrganizationShareReviews,
  listOrganizations,
  listAgentRuns,
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
  getGovernorOverview,
  getConversation,
  updateConversation,
  getCodexModels,
  getCodexAccount,
  getCodexRateLimits,
  getCodexHarnessStatus,
  getCodexConfigurationStatus,
  getCodexPendingRequests,
  getCodexStableDiscovery,
  reconcileOwnedCodexThread,
  forkOwnedCodexThread,
  startOwnedCodexReview,
  watchConversationProject,
  unwatchConversationProject,
  startCodexLogin,
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
  type Conversation,
  type ConversationActivity,
  type AgentDelegation,
  type ConversationTeamState,
  type ConversationMessage,
  type ConversationTurn,
  type ConversationView,
  type CodexModel,
  type CodexAccount,
  type CodexRateLimits,
  type CodexHarnessStatus,
  type CodexConfigurationStatus,
  type CodexPendingRequest,
  type CodexStableDiscovery,
  type CodexLoginStart,
  type ReasoningEffort,
  type ExecutionSpeed,
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
    <div ref={hostRef} className="embedded-site-browser-host" aria-label="MCG browser content" />
  </div>;
}

function PinIcon({ solid = false }: { solid?: boolean }) {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><path fill={solid ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" d="M14.7 4.1 19.9 9.3l-2.6 1.3-2.8 4.5.2 2.4-1.1 1.1-3.2-3.2-4.7 4.7-.9-.9 4.7-4.7-3.2-3.2 1.1-1.1 2.4.2 4.5-2.8 1.3-2.6Z" /></svg>;
}

function ArchiveIcon() {
  return <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" d="M4 7h16M5 7l1 13h12l1-13M4 4h16v3H4zM9.5 11.5h5" /></svg>;
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
  approval_mode: "ask"
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
      model_id: parsed.model_id,
      model_label: parsed.model_label,
      reasoning_effort: parsed.reasoning_effort ?? "medium",
      speed: parsed.speed ?? "standard",
      service_tier: parsed.service_tier,
      approval_mode: parsed.approval_mode ?? "ask"
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
  created_at: string;
}

function storedChatQueue(): QueuedChatMessage[] {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(CHAT_QUEUE_STORAGE_KEY) || "[]") as Partial<QueuedChatMessage>[];
    return parsed.filter((item): item is QueuedChatMessage => Boolean(
      item.id && item.conversation_id && item.content?.trim() && item.preferences && item.created_at
    ));
  } catch {
    return [];
  }
}

const APPROVAL_MODE_DETAILS: Record<ApprovalMode, { label: string; description: string; icon: string }> = {
  ask: {
    label: "Ask for approval",
    description: "Ask before editing outside the workspace or using restricted capabilities.",
    icon: "✋"
  },
  auto_review: {
    label: "Approve for me",
    description: "Codex reviews requests and only asks you when an action may be unsafe.",
    icon: "⬡"
  },
  full_access: {
    label: "Full access",
    description: "Allow internet access and any file on this computer without asking.",
    icon: "⚠"
  }
};

function PermissionSelector({
  value,
  onChange,
  hasProject
}: {
  value: ApprovalMode;
  onChange: (mode: ApprovalMode) => void;
  hasProject: boolean;
}) {
  const selected = APPROVAL_MODE_DETAILS[value];
  return <details className={`permission-selector permission-${value}`}>
    <summary aria-label="Change permissions" title="Change permissions">
      <span aria-hidden="true">{selected.icon}</span>
      <span>{selected.label}</span>
      <span aria-hidden="true">⌄</span>
    </summary>
    <div className="permission-popover" role="menu" aria-label="How should MCG actions be approved?">
      <div className="permission-popover-heading">
        <strong>How should MCG actions be approved?</strong>
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
            onChange(mode);
            event.currentTarget.closest("details")?.removeAttribute("open");
          }}
        >
          <span className="permission-option-icon" aria-hidden="true">{detail.icon}</span>
          <span><strong>{detail.label}</strong><small>{detail.description}</small></span>
          {value === mode && <span className="permission-option-check" aria-hidden="true">✓</span>}
        </button>;
      })}
      {!hasProject && <p>Your selection is saved. Project-free turns stay read-only until you attach a project.</p>}
    </div>
  </details>;
}

function ChatRunSelector({
  providers,
  codexModels = [],
  value,
  onChange,
  discovering,
  discoveryError,
  onRefresh,
  label = "Run settings"
}: {
  providers: ProviderListItem[];
  codexModels?: CodexModel[];
  value: ChatRunPreferences;
  onChange: (preferences: ChatRunPreferences) => void;
  discovering: boolean;
  discoveryError: string | null;
  onRefresh: () => void;
  label?: string;
}) {
  const providerModels = providers.flatMap((provider) => (provider.catalog?.models ?? []).map((model) => ({
    providerId: provider.connection.id,
    providerName: provider.connection.display_name,
    model
  })));
  const useCodexCatalog = codexModels.length > 0;
  const models = useCodexCatalog
    ? codexModels.filter((model) => !model.hidden).map((model) => ({ providerId: "codex", providerName: "OpenAI Subscription", model: { id: model.id, display_name: model.displayName } as ProviderModel }))
    : providerModels;
  const modelKey = value.provider_id && value.model_id ? `${value.provider_id}\u0000${value.model_id}` : "";
  const summaryModel = value.model_label ?? value.model_id ?? "Automatic";
  const updateModel = (key: string) => {
    const selected = models.find((entry) => `${entry.providerId}\u0000${entry.model.id}` === key);
    onChange(selected ? {
      ...value,
      provider_id: selected.providerId,
      model_id: selected.model.id,
      model_label: selected.model.display_name,
      reasoning_effort: useCodexCatalog
        ? codexModels.find((model) => model.id === selected.model.id)?.defaultReasoningEffort ?? value.reasoning_effort
        : value.reasoning_effort,
      service_tier: useCodexCatalog
        ? codexModels.find((model) => model.id === selected.model.id)?.defaultServiceTier
        : value.service_tier
    } : {
      reasoning_effort: value.reasoning_effort,
      speed: value.speed,
      service_tier: value.service_tier,
      approval_mode: value.approval_mode
    });
  };
  const selectedCodexModel = value.model_id
    ? codexModels.find((model) => model.id === value.model_id)
    : codexModels.find((model) => model.isDefault);
  const effortChoices = selectedCodexModel?.supportedReasoningEfforts.map((item) => item.reasoningEffort) ?? ["none", "low", "medium", "high", "xhigh"];
  const tierChoices = selectedCodexModel?.serviceTiers ?? [];
  return <details className="chat-run-selector">
    <summary aria-label={label} title={label}>
      <span>{summaryModel}</span>
      <small>{value.reasoning_effort === "xhigh" ? "Extra high" : value.reasoning_effort} · {value.speed}</small>
      <span aria-hidden="true">⌄</span>
    </summary>
    <div className="chat-run-popover">
      <strong>{label}</strong>
      <label>Model<select value={modelKey} disabled={discovering} onChange={(event) => updateModel(event.target.value)}>
        <option value="">Automatic</option>
        {models.map(({ providerId, providerName, model }) => <option value={`${providerId}\u0000${model.id}`} key={`${providerId}-${model.id}`}>{model.display_name} · {providerName}</option>)}
      </select></label>
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
        <button className="secondary chat-run-refresh" type="button" disabled={discovering || (!useCodexCatalog && providers.length === 0)} onClick={onRefresh}>{discovering ? "Loading…" : "Refresh models"}</button>
      </div>
      <label>Effort<select value={value.reasoning_effort} onChange={(event) => onChange({ ...value, reasoning_effort: event.target.value as ReasoningEffort })}>
        {effortChoices.map((effort) => <option value={effort} key={effort}>{effort === "xhigh" ? "Extra high" : effort.charAt(0).toUpperCase() + effort.slice(1)}</option>)}
      </select></label>
      <label>Speed<select value={value.service_tier ?? ""} onChange={(event) => {
        const serviceTier = event.target.value;
        onChange({ ...value, service_tier: serviceTier || undefined, speed: /fast|priority/i.test(serviceTier) ? "fast" : "standard" as ExecutionSpeed });
      }}>
        {tierChoices.length > 0
          ? <><option value="">Standard</option>{tierChoices.map((tier) => <option value={tier.id} key={tier.id}>{tier.name}</option>)}</>
          : <><option value="">Standard</option><option value="fast">Fast</option></>}
      </select></label>
      <small>{useCodexCatalog ? "Choices are reported by Codex for the shared signed-in account." : "Effort is applied when the selected model supports reasoning control. Fast requests the provider priority tier."}</small>
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
                  {activity.output && <pre>{activity.output}</pre>}
                </div>
              </details>
            : <div className={`chat-command state-${activity.state}`} key={activity.codex_item_id}>{summary}</div>;
        })}
      </div>
    </details>;
}

type ConversationTimelineEntry =
  | { type: "message"; timestamp: number; stableOrder: number; message: ConversationMessage }
  | { type: "activity"; timestamp: number; stableOrder: number; activity: ConversationActivity };

function ConversationActivityTimeline({ messages, activities }: { messages: ConversationMessage[]; activities: ConversationActivity[] }) {
  const visibleActivities = activities.filter((activity) => activity.kind !== "usage");
  const entries: ConversationTimelineEntry[] = [
    ...messages.map((message, stableOrder) => ({
      type: "message" as const,
      timestamp: message.codex_item_id ? Date.parse(message.created_at) : Number.POSITIVE_INFINITY,
      stableOrder,
      message
    })),
    ...visibleActivities.map((activity, stableOrder) => ({
      type: "activity" as const,
      timestamp: Date.parse(activity.started_at),
      stableOrder: messages.length + stableOrder,
      activity
    }))
  ].sort((left, right) => left.timestamp - right.timestamp || left.stableOrder - right.stableOrder);

  const content: ReactNode[] = [];
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
    if (entry.type === "activity") {
      const activity = entry.activity;
      const presentation: Record<ConversationActivity["kind"], [string, string]> = {
        working: ["…", "Working"], waiting: ["◷", "Waiting"], reasoning_summary: ["✦", "Reasoning"],
        plan: ["☷", "Updated plan"], command: ["›_", "Ran commands"], file_change: ["✎", "Updated files"],
        mcp_call: ["⌘", "Used a tool"], collaboration_call: ["⌘", "Delegated work"], web_search: ["⌕", "Searched the web"],
        image_view: ["▧", "Viewed an image"], approval: ["!", "Approval"], elicitation: ["?", "Input requested"],
        context_compaction: ["≡", activity.state === "running" ? "Context automatically compacting" : "Context automatically compacted"],
        usage: ["◔", "Usage updated"], recovery: ["↻", "Recovering"], interruption: ["■", "Interrupted"],
        failure: ["!", "Failed"], completion: ["✓", "Completed"]
      };
      const [icon, fallbackLabel] = presentation[activity.kind];
      const label = activity.command || fallbackLabel;
      const canExpand = Boolean(activity.output) && activity.kind !== "reasoning_summary";
      const row = <>
        <span className="chat-activity-icon" aria-hidden="true">{icon}</span>
        <span>{label}</span>
        {activity.state === "running" && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}
      </>;
      content.push(canExpand
        ? <details className={`chat-compaction state-${activity.state}`} key={activity.codex_item_id}>
            <summary>{row}</summary><pre className="chat-activity-detail">{activity.output}</pre>
          </details>
        : <div className={`chat-compaction state-${activity.state}`} role="status" key={activity.codex_item_id}>{row}</div>);
    } else {
      const message = entry.message;
      content.push(<article className="chat-message assistant-message" key={message.id}>
        <span>Chat Agent</span><p>{message.content}</p>
      </article>);
    }
    index += 1;
  }

  if (content.length === 0) return null;
  return <div className="chat-activity-timeline" aria-label="Turn activity">{content}</div>;
}

function CodingAgentCards({ team, conversationId, onView }: { team?: ConversationTeamState; conversationId?: string; onView: (view: ConversationView) => void }) {
  const [tabs, setTabs] = useState<Record<string, "activity" | "exchange" | "blackboard" | "changes" | "evidence">>({});
  const [patches, setPatches] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  if (!team || team.delegations.length === 0) return null;
  const stateLabel = (state: AgentDelegation["state"]) => state.replaceAll("_", " ").replace(/^./, (value) => value.toLocaleUpperCase());
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
    {team.delegations.map((delegation, index) => {
      const session = team.sessions.find((candidate) => candidate.id === delegation.coder_session_id);
      const worktree = team.worktrees.find((candidate) => candidate.id === delegation.worktree_id);
      const messages = team.messages.filter((message) => message.delegation_id === delegation.id);
      const blackboard = team.blackboard.filter((entry) => entry.delegation_id === delegation.id);
      const active = ["requested", "preparing", "running", "waiting", "blocked", "reported", "integrating"].includes(delegation.state);
      const tab = tabs[delegation.id] ?? "activity";
      const cancellable = ["requested", "preparing", "running", "waiting"].includes(delegation.state);
      const reviewable = Boolean(worktree) && ["reported", "blocked", "failed", "cancelled"].includes(delegation.state);
      const applicable = Boolean(worktree) && ["reported", "blocked"].includes(delegation.state);
      const discardable = Boolean(worktree) && worktree?.state !== "removed" && !["preparing", "running", "integrating"].includes(delegation.state);
      const retryable = ["failed", "cancelled", "blocked"].includes(delegation.state);
      return <details className={`coding-agent-card state-${delegation.state}`} key={delegation.id}>
        <summary>
          <span className="coding-agent-glyph" aria-hidden="true">⌘</span>
          <span className="coding-agent-title"><strong>Coding Agent {index + 1}</strong><small>{delegation.brief.objective}</small></span>
          <span className="coding-agent-state">{stateLabel(delegation.state)}{active && <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>}</span>
          <span className="chat-activity-chevron" aria-hidden="true">⌄</span>
        </summary>
        <div className="coding-agent-detail">
          <div className="coding-agent-meta">
            {session?.model && <span>Model <code>{session.model}</code></span>}
            <span>Prompt revision <code>{session?.prompt_revision ?? "—"}</code></span>
            {worktree && <span>Branch <code>{worktree.branch}</code></span>}
            {delegation.run_id && <span>Run <code>{delegation.run_id}</code></span>}
            {delegation.agent_lease_id && <span>Lease <code>{delegation.agent_lease_id}</code></span>}
          </div>
          <div className="coding-agent-tabs" role="tablist" aria-label={`Coding Agent ${index + 1} details`}>{(["activity", "exchange", "blackboard", "changes", "evidence"] as const).map((value) => <button type="button" role="tab" aria-selected={tab === value} className={tab === value ? "active" : ""} onClick={() => setTabs((current) => ({ ...current, [delegation.id]: value }))} key={value}>{value === "exchange" ? "Agent Exchange" : value.replace(/^./, (letter) => letter.toUpperCase())}</button>)}</div>
          {tab === "activity" && <section><h4>Activity</h4><p>Status: {stateLabel(delegation.state)}</p><p>Started {new Date(delegation.created_at).toLocaleString()}</p>{delegation.retry_of_id && <p>Retry of <code>{delegation.retry_of_id}</code></p>}{worktree && <p>Isolation: {(worktree.workspace_mode ?? "git_worktree").replaceAll("_", " ")} · baseline {(worktree.baseline_confidence ?? "verified").replaceAll("_", " ")}</p>}{worktree?.retained_reason && <p>Retained: {worktree.retained_reason}</p>}</section>}
          {tab === "exchange" && <section><h4>Agent exchange</h4>{messages.length === 0 ? <p>No messages recorded yet.</p> : messages.map((message) => <article key={message.id}><small>{message.kind.replaceAll("_", " ")}</small><p>{message.content}</p></article>)}</section>}
          {tab === "blackboard" && <section><h4>Blackboard</h4>{blackboard.length === 0 ? <p>No shared entries yet.</p> : blackboard.map((entry) => <article key={entry.id}><small>{entry.kind}</small><p>{entry.content}</p></article>)}</section>}
          {tab === "changes" && <section><h4>Changes</h4>{delegation.changed_files.length === 0 ? <p>No changed files reported yet.</p> : delegation.changed_files.map((file) => <code key={file}>{file}</code>)}{patches[delegation.id] && <pre className="coding-agent-patch">{patches[delegation.id]}</pre>}</section>}
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
  active
}: {
  turn: ConversationTurn;
  messages: ConversationMessage[];
  activities: ConversationActivity[];
  active: boolean;
}) {
  const visibleMessages = messages.filter((message) => message.role !== "assistant" || message.content.length > 0);
  const assistantMessages = visibleMessages.filter((message) => message.role !== "user");
  const hasResponseText = messages.some((message) => message.role === "assistant" && message.content.length > 0);
  return <section className="chat-turn" aria-label="Conversation turn">
    {visibleMessages.filter((message) => message.role === "user").map((message) => <article className="chat-message user-message" key={message.id}>
      <span>You</span><p>{message.content}</p>
    </article>)}
    {(activities.length > 0 || assistantMessages.length > 0) && <ConversationActivityTimeline messages={assistantMessages} activities={activities} />}
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

const MCG_SHORTCUTS = [
  ["New chat", "Start a new chat", "Ctrl+N"],
  ["Open global folder", "Make a folder available to every project and general chat", "Ctrl+O"],
  ["Settings", "Open MCG settings", "Ctrl+,"],
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
  const [codexAccountDisconnected, setCodexAccountDisconnected] = useState(() => window.localStorage.getItem("mcg.codex-account-disconnected") === "true");
  const [providerStatus, setProviderStatus] = useState("Not connected");
  const [busy, setBusy] = useState(false);
  const [providerError, setProviderError] = useState<string | null>(null);
  const [providers, setProviders] = useState<ProviderListItem[]>([]);
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
  const [codexAccount, setCodexAccount] = useState<CodexAccount | null>(null);
  const [codexRateLimits, setCodexRateLimits] = useState<CodexRateLimits | null>(null);
  const [codexHarnessStatus, setCodexHarnessStatus] = useState<CodexHarnessStatus | null>(null);
  const [codexConfigurationStatus, setCodexConfigurationStatus] = useState<CodexConfigurationStatus | null>(null);
  const [codexPendingRequests, setCodexPendingRequests] = useState<CodexPendingRequest[]>([]);
  const [codexStableDiscovery, setCodexStableDiscovery] = useState<CodexStableDiscovery | null>(null);
  const [codexRecoveryBusy, setCodexRecoveryBusy] = useState(false);
  const [codexReviewBusy, setCodexReviewBusy] = useState(false);
  const [draftThreadId, setDraftThreadId] = useState(() => crypto.randomUUID());
  const [taskGoal, setTaskGoal] = useState("");
  const [queuedChatMessages, setQueuedChatMessages] = useState<QueuedChatMessage[]>(storedChatQueue);
  const [steeringMessageId, setSteeringMessageId] = useState<string | null>(null);
  const [queueDispatchErrorId, setQueueDispatchErrorId] = useState<string | null>(null);
  const [taskConstraints, setTaskConstraints] = useState("");
  const [taskSuccess, setTaskSuccess] = useState("");
  const [taskRisk, setTaskRisk] = useState<RiskLevel>("medium");
  const [taskMaxCost, setTaskMaxCost] = useState("");
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
      return stored ? JSON.parse(stored) as Record<string, ChatRunPreferences> : {};
    } catch {
      return {};
    }
  });
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
  const [agentTab, setAgentTab] = useState<"overview" | "teams" | "activity">("overview");
  const [governorOverview, setGovernorOverview] = useState<GovernorOverview | null>(null);
  const [organizationAssignments, setOrganizationAssignments] = useState<OrganizationAssignments | null>(null);
  const [agentRuns, setAgentRuns] = useState<AgentRunView[]>([]);
  const [selectedAgentRunId, setSelectedAgentRunId] = useState<string | null>(null);
  const [organizationDraft, setOrganizationDraft] = useState<OrganizationGenome | null>(null);
  const [organizationPreview, setOrganizationPreview] = useState<OrganizationRevisionPreview | null>(null);
  const [organizationChangeSummary, setOrganizationChangeSummary] = useState("");
  const [agentBusy, setAgentBusy] = useState(false);
  const [agentError, setAgentError] = useState<string | null>(null);
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
    if (!chatContextMenu && !siteContextMenu) return;
    const close = () => {
      setChatContextMenu(null);
      setSiteContextMenu(null);
    };
    window.addEventListener("pointerdown", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("blur", close);
    };
  }, [chatContextMenu, siteContextMenu]);

  useEffect(() => {
    window.localStorage.setItem("mcg.global-folders", JSON.stringify(globalFolders));
  }, [globalFolders]);

  useEffect(() => {
    window.localStorage.setItem("mcg.chat-run-preferences.default", JSON.stringify(newChatRunPreferences));
  }, [newChatRunPreferences]);

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
        if (active) setCodexSitesError(`The OpenAI Sites library could not refresh: ${errorMessage(error)}`);
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
      if (event.key === "Escape") {
        setDesktopMenu(null);
        setWorkspacePopover(null);
        return;
      }
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
      title: "MCG",
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

  function continueSiteLibraryChat(entry: SiteLibraryEntry) {
    if (entry.localSite) {
      continueSiteChat(entry.localSite);
      return;
    }
    setSelectedProject(null);
    setSelectedChatId(null);
    setTaskGoal(`Refine the existing OpenAI Site ${entry.displayName} at ${entry.publicUrl}. Inspect its current Sites project and source, show me a reviewable version, and do not publish changes until they are approved.`);
    setTaskConstraints("");
    setTaskSuccess("");
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
        provider_id: conversation.execution_preferences.model_id ? "codex" : undefined,
        model_id: conversation.execution_preferences.model_id,
        model_label: model?.displayName,
        reasoning_effort: conversation.execution_preferences.reasoning_effort ?? current.reasoning_effort,
        service_tier: tier,
        speed: tier && /fast|priority/i.test(tier) ? "fast" : "standard",
        approval_mode: conversation.execution_preferences.approval_mode ?? "ask"
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

  async function copyChatText(value: string, status: string) {
    try {
      await navigator.clipboard.writeText(value);
      setWorkspaceActionStatus(status);
    } catch (error) {
      setTaskError(`Copy failed: ${errorMessage(error)}`);
    }
    setChatContextMenu(null);
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
    Promise.allSettled([getCodexModels(), getCodexAccount(), getCodexRateLimits(), getCodexHarnessStatus(), getCodexConfigurationStatus(), getCodexPendingRequests()])
      .then(([models, account, rateLimits, harness, configuration, pendingRequests]) => {
        if (!active) return;
        if (models.status === "fulfilled") setCodexModels(models.value);
        if (account.status === "fulfilled") setCodexAccount(account.value);
        if (rateLimits.status === "fulfilled") setCodexRateLimits(rateLimits.value);
        if (harness.status === "fulfilled") setCodexHarnessStatus(harness.value);
        if (configuration.status === "fulfilled") setCodexConfigurationStatus(configuration.value);
        if (pendingRequests.status === "fulfilled") setCodexPendingRequests(pendingRequests.value);
        if (models.status === "rejected") setModelDiscoveryError(`Codex model discovery unavailable: ${errorMessage(models.reason)}`);
      });
    getCodexStableDiscovery()
      .then((discovery) => { if (active) setCodexStableDiscovery(discovery); })
      .catch(() => { if (active) setCodexStableDiscovery(null); });
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
    getGovernorOverview()
      .then((response) => {
        if (active) {
          setGovernorOverview(response);
          setOrganizationAssignments(response.assignments);
        }
      })
      .catch((error) => {
        if (active) setAgentError(errorMessage(error));
      });
    listAgentRuns()
      .then((response) => {
        if (active) setAgentRuns(response.runs);
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
    if (method === "browser") {
      try {
        if (codexAccount?.authenticated) {
          window.localStorage.removeItem("mcg.codex-account-disconnected");
          setCodexAccountDisconnected(false);
          setProviderStatus("Using shared Codex account");
          return;
        }
        const started = await startCodexLogin();
        setCodexLogin(started);
        setProviderStatus("Waiting for browser authorization (up to 3 minutes)");
        await openExternalUrl(started.authUrl);
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
        await cancelCodexLogin(started.loginId).catch(() => undefined);
        setCodexLogin(null);
        setProviderStatus("Sign-in timed out");
        setProviderError("Sign-in timed out after 3 minutes. Start sign-in again when you’re ready.");
      } catch (error) {
        setCodexLogin(null);
        setProviderError(errorMessage(error));
        setProviderStatus("Connection failed");
      } finally {
        setBusy(false);
      }
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
      const [models, account, limits, harness, configuration, pendingRequests] = await Promise.all([
        getCodexModels(), getCodexAccount(), getCodexRateLimits(), getCodexHarnessStatus(), getCodexConfigurationStatus(), getCodexPendingRequests()
      ]);
      setCodexModels(models);
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
    reader.onerror = () => setAccountAvatarError("MCG could not read that image.");
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
        setAccountAvatarError("MCG could not save that image locally.");
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
      setProviderStatus("MCG disconnected from the shared Codex account");
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
    preferences: ChatRunPreferences = newChatRunPreferences
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
        model_id: preferences.model_id,
        reasoning_effort: preferences.reasoning_effort,
        service_tier: preferences.service_tier,
        approval_mode: preferences.approval_mode
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
      const view = await sendConversationMessage(conversation.id, { content, execution_preferences: executionPreferences });
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

  async function addTask() {
    const content = taskGoal.trim();
    if (!content) {
      setTaskError("Write a message to continue.");
      return;
    }
    setTaskGoal("");
    setTaskConstraints("");
    setTaskSuccess("");
    setTaskMaxCost("");
    const sent = await sendMessageContent(content);
    if (!sent) setTaskGoal((current) => current || content);
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
          model_id: uncertainTurn.execution_preferences.model_id,
          reasoning_effort: uncertainTurn.execution_preferences.reasoning_effort ?? newChatRunPreferences.reasoning_effort,
          service_tier: uncertainTurn.execution_preferences.service_tier,
          approval_mode: uncertainTurn.execution_preferences.approval_mode ?? newChatRunPreferences.approval_mode
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
    if (!selectedChatId || !workspaceProject) return;
    setCodexReviewBusy(true);
    setTaskError(null);
    try {
      const result = await startOwnedCodexReview(selectedChatId, { type: "uncommittedChanges" }, "inline");
      setWorkspaceActionStatus(`Review started on owned Codex thread ${result.reviewThreadId.slice(0, 8)}…`);
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setCodexReviewBusy(false);
    }
  }

  function queueDraftMessage() {
    const content = taskGoal.trim();
    if (!content || !selectedChatId) return;
    setQueuedChatMessages((current) => [...current, {
      id: crypto.randomUUID(),
      conversation_id: selectedChatId,
      content,
      preferences: { ...newChatRunPreferences },
      created_at: new Date().toISOString()
    }]);
    setTaskGoal("");
    setTaskError(null);
  }

  async function steerMessage(content: string, queuedMessageId?: string) {
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
      const view = await steerConversationTurn(selectedChatId, turn.id, content.trim());
      setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      if (queuedMessageId) {
        setQueuedChatMessages((current) => current.filter((item) => item.id !== queuedMessageId));
      } else {
        setTaskGoal("");
      }
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setSteeringMessageId(null);
    }
  }

  function handleComposerKeyDown(event: ReactKeyboardEvent<HTMLTextAreaElement>) {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
    event.preventDefault();
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
    const [overview, runs] = await Promise.all([getGovernorOverview(), listAgentRuns()]);
    setGovernorOverview(overview);
    setOrganizationAssignments(overview.assignments);
    setAgentRuns(runs.runs);
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
  const conversationIds = new Set(conversationChatSummaries.map((chat) => chat.id));
  const legacyTaskChatSummaries = tasks
    .filter((task) => !conversationIds.has(chatIdForTask(task)))
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
  const unassignedChats = chatSummaries.filter((chat) => !chat.conversation.project_id || !projects.some((project) => project.project_id === chat.conversation.project_id));
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
  const managedConversationWorktrees = Object.values(conversationViews).flatMap((view) =>
    (view?.team?.worktrees ?? []).map((worktree) => ({
      worktree,
      conversationId: view!.conversation.id,
      delegation: view!.team!.delegations.find((candidate) => candidate.id === worktree.delegation_id)
    }))
  );
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
  const codingTeamIsRunning = Boolean(visibleConversation?.team?.delegations.some((delegation) =>
    ["requested", "preparing", "running", "waiting", "integrating"].includes(delegation.state)
  ));

  useEffect(() => {
    if (activeSurface !== "threads" || !selectedChatId || !codingTeamIsRunning) return;
    const refresh = () => {
      void getConversation(selectedChatId).then((view) => {
        setConversationViews((current) => ({ ...current, [selectedChatId]: view }));
      }).catch(() => undefined);
    };
    const timer = window.setInterval(refresh, 1000);
    return () => window.clearInterval(timer);
  }, [activeSurface, selectedChatId, codingTeamIsRunning]);

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
    void sendMessageContent(next.content, conversation, next.preferences).then((sent) => {
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

  const workspaceOwnsChatScroll = workspaceSidePanelOpen || workspaceBottomPanelOpen;

  useEffect(() => {
    if (activeSurface !== "threads" || !visibleConversation) return;
    chatShouldFollowRef.current = true;
    const scrollTarget = workspaceOwnsChatScroll ? workspaceRef.current : window;
    const viewport = workspaceOwnsChatScroll ? workspaceRef.current : document.scrollingElement;
    if (!scrollTarget || !viewport) return;
    const trackChatPosition = () => {
      chatShouldFollowRef.current = isChatViewportNearBottom(viewport);
    };
    scrollTarget.addEventListener("scroll", trackChatPosition, { passive: true });
    return () => scrollTarget.removeEventListener("scroll", trackChatPosition);
  }, [activeSurface, selectedChatId, visibleConversation?.conversation.id, workspaceOwnsChatScroll]);

  useEffect(() => {
    if (activeSurface !== "threads" || !visibleConversation || !chatShouldFollowRef.current) return;
    const frame = window.requestAnimationFrame(() => {
      const viewport = workspaceOwnsChatScroll ? workspaceRef.current : document.scrollingElement;
      if (viewport && chatShouldFollowRef.current) viewport.scrollTop = viewport.scrollHeight;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [activeSurface, selectedChatId, visibleConversation, workspaceOwnsChatScroll]);

  const activeConversationApproval = activeConversationTurn?.task_id
    ? approvals.find((approval) => approval.task_id === activeConversationTurn.task_id)
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
  const filteredShortcuts = MCG_SHORTCUTS.filter(([name, description, shortcut]) =>
    !normalizedShortcutSearch || `${name} ${description} ${shortcut}`.toLocaleLowerCase().includes(normalizedShortcutSearch)
  );
  const selectedAgentRun = agentRuns.find((run) => run.run.id === selectedAgentRunId) ?? null;
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
  const workspaceProject = projects.find((project) => project.project_id === (workspaceConversation?.project_id ?? workspaceTask?.project_id)) ?? selectedProject ?? undefined;
  const workspaceProjectName = workspaceProject?.root.split(/[\\/]/).filter(Boolean).at(-1) ?? "No project selected";
  const activeLocalSites = sites.filter((site) => site.state !== "archived");
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
    <div className={`desktop-frame${sidebarOpen ? "" : " sidebar-collapsed"}${workspaceSidePanelOpen && workspacePanelsVisible ? " side-panel-open" : ""}${workspaceBottomPanelOpen && workspacePanelsVisible ? " bottom-panel-open" : ""}`} style={workspaceLayoutStyle}>
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
                <button type="button" onClick={() => { setWorkspaceBottomPanelOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Bottom Panel</span><span className="menu-shortcut">Ctrl+J</span></button>
                <button type="button" onClick={() => { setWorkspacePinnedSummaryOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Pinned Summary</span></button>
                <button type="button" onClick={() => { setWorkspaceBottomPanelOpen(true); setDesktopMenu(null); window.requestAnimationFrame(() => terminalInputRef.current?.focus()); }}><span>Open Terminal</span><span className="menu-shortcut">Ctrl+`</span></button>
                <button type="button" onClick={() => { setProjectTreeOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle File Tree</span><span className="menu-shortcut">Ctrl+Shift+E</span></button>
                <button type="button" onClick={() => { setWorkspaceSidePanelOpen((current) => !current); setDesktopMenu(null); }}><span>Toggle Side Panel</span><span className="menu-shortcut">Ctrl+Alt+B</span></button>
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
                <button type="button" disabled title="MCG documentation is being prepared"><span>Documentation</span></button>
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
                <button type="button" onClick={() => openSettings("configuration")}><span>About MCG</span></button>
              </>}
            </div>
          </div>)}
        </div>
        <div className="window-drag-region" data-tauri-drag-region onDoubleClick={() => void toggleCurrentWindowMaximized()} />
        <div className="window-controls" aria-label="Window controls">
          <button type="button" aria-label="Minimize" title="Minimize" onClick={() => void minimizeCurrentWindow()}><span aria-hidden="true">—</span></button>
          <button type="button" aria-label="Maximize or restore" title="Maximize or restore" onClick={() => void toggleCurrentWindowMaximized()}><span aria-hidden="true">□</span></button>
          <button className="window-close" type="button" aria-label="Close window" title="Close" onClick={() => void closeCurrentWindow()}><span aria-hidden="true">×</span></button>
        </div>
      </header>
      <div
        className={`app-shell ${advanced ? "advanced-mode" : "simple-mode"}`}
        data-active-surface={activeSurface}
      >
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="sidebar-heading">
          <div className="sidebar-brand-row"><div className="brand" aria-label="MetaCognitive Governor"><span>MCG</span></div><div className="sidebar-brand-actions">
            <button type="button" aria-label="Search chats" title="Search" aria-pressed={sidebarSearchOpen} onClick={() => { setSidebarSearchOpen((current) => !current); window.requestAnimationFrame(() => chatSearchRef.current?.focus()); }}><span aria-hidden="true">⌕</span></button>
            <button type="button" aria-label="Open activity" title="Activity" onClick={() => { setWorkspaceSidePanelTab("activity"); setWorkspaceSidePanelOpen(true); }}><span aria-hidden="true">♧</span></button>
          </div></div>
          <label className={`chat-search${sidebarSearchOpen || chatSearch ? " open" : ""}`}><span aria-hidden="true">⌕</span><span className="sr-only">Search chats</span><input ref={chatSearchRef} aria-label="Search chats" value={chatSearch} onChange={(event) => setChatSearch(event.target.value)} onBlur={() => { if (!chatSearch) setSidebarSearchOpen(false); }} placeholder="Search" /></label>
        </div>
        <nav className="sidebar-shortcuts" aria-label="Workspace pages">
          <button className={activeSurface === "tasks" ? "nav-item active" : "nav-item"} type="button" onClick={() => startNewChat()}><span className="shortcut-glyph" aria-hidden="true">✎</span>New chat</button>
          {[(["pull-requests", "Pull requests", "⑂"]), (["sites", "Sites", "▦"]), (["scheduler", "Scheduled", "◷"]), (["agents", "Agents", "⌘"]), (["plugins", "Plugins", "◉"])].map(([surface, label, glyph]) => {
            const selected = activeSurface === surface || (surface === "agents" && activeSurface === "improvement");
            return <a className={`nav-item${selected ? " active" : ""}`} href={`#${surface}`} aria-current={selected ? "page" : undefined} onClick={(event) => openSurface(event, surface)} key={surface}><span className="shortcut-glyph" aria-hidden="true">{glyph}</span>{label}</a>;
          })}
        </nav>
        <div className="sidebar-library">
          {pinnedSummaryOpen && <section className="sidebar-section" aria-label="Pinned chats"><h2>Pinned</h2>{pinnedChats.length === 0 ? <p>No pinned chats</p> : pinnedChats.map((chat) => renderChatRow(chat, "pinned"))}</section>}
          {projectTreeOpen && <section className="sidebar-section project-tree" aria-label="Projects"><h2>Projects</h2>{projects.length === 0 && <p>No projects</p>}{projects.map((project) => {
            const name = project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root;
            const projectChats = chatSummaries.filter((chat) => chat.conversation.project_id === project.project_id);
            return <div className="project-tree-item" key={project.project_id}><div className="project-tree-heading"><button className="project-tree-name" type="button" onClick={() => startNewChat(project)}><span aria-hidden="true">▱</span>{name}</button><button className="project-new-chat" type="button" aria-label={`New chat in ${name}`} title={`New chat in ${name}`} onClick={() => startNewChat(project)}><span aria-hidden="true">＋</span></button></div>{projectChats.length === 0 ? <p>No chats</p> : projectChats.map((chat) => renderChatRow(chat, "project"))}</div>;
          })}{unassignedChats.length > 0 && <div className="project-tree-item"><button type="button" onClick={() => navigateSurface("threads")}><span aria-hidden="true">▱</span>General</button>{unassignedChats.map((chat) => renderChatRow(chat, "project"))}</div>}</section>}
          <section className="sidebar-section" aria-label="Recent chats">
            <button className="sidebar-section-toggle" type="button" aria-expanded={recentsOpen} onClick={() => setRecentsOpen((current) => { window.localStorage.setItem("mcg.sidebar-recents-open", String(!current)); return !current; })}><span>Recents</span><span aria-hidden="true">{recentsOpen ? "⌄" : "›"}</span></button>
            {recentsOpen && (recentChats.length === 0 ? <p>No recent chats</p> : recentChats.map((chat) => renderChatRow(chat, "recent")))}
          </section>
          <section className="sidebar-section" aria-label="Archived chats">
            <button className="sidebar-section-toggle" type="button" aria-expanded={archivedChatsOpen} onClick={() => setArchivedChatsOpen((current) => { window.localStorage.setItem("mcg.sidebar-archived-open", String(!current)); return !current; })}><span>Archived</span><span aria-hidden="true">{archivedChatsOpen ? "⌄" : "›"}</span></button>
            {archivedChatsOpen && (archivedConversations.length === 0 ? <p>No archived chats</p> : archivedConversations.map(renderArchivedChatRow))}
          </section>
        </div>
        <div className="sidebar-account-area">
          <button className="owner-menu-button" type="button" aria-expanded={ownerMenuOpen} onClick={() => setOwnerMenuOpen((current) => !current)}><AccountAvatar className="owner-avatar" profile={signedInWithOpenAi ? openAiProfile : undefined} avatarUrl={signedInWithOpenAi ? openAiAvatarUrl : undefined} fallback={signedInWithOpenAi ? "O" : "M"} /><span><strong>{signedInWithOpenAi ? openAiAccountName : "Local owner"}</strong><small>{signedInWithOpenAi ? openAiAccountDetail : "Not signed in"}</small></span><span aria-hidden="true">⋯</span></button>
          <div className={`owner-menu${ownerMenuOpen ? " open" : ""}`} aria-label="Owner menu">
            <div className="owner-menu-identity"><AccountAvatar className="owner-avatar" profile={signedInWithOpenAi ? openAiProfile : undefined} avatarUrl={signedInWithOpenAi ? openAiAvatarUrl : undefined} fallback={signedInWithOpenAi ? "O" : "M"} /><span><strong>{signedInWithOpenAi ? openAiAccountName : "Local owner"}</strong><small>{signedInWithOpenAi ? openAiAccountDetail : "MCG workspace"}</small></span></div>
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
        role="menu"
        aria-label={`${contextSite.displayName} site menu`}
        style={{ left: siteContextMenu.x, top: siteContextMenu.y }}
        onPointerDown={(event) => event.stopPropagation()}
        onContextMenu={(event) => event.preventDefault()}
      >
        <button role="menuitem" type="button" title="Uses MCG's separate, persistent browser profile" onClick={() => void openSiteInApp(contextSite)}><span aria-hidden="true">◎</span>Open in MCG browser</button>
        <button role="menuitem" type="button" onClick={() => void openSiteExternally(contextSite)}><span aria-hidden="true">↗</span>Open in external browser</button>
      </div>}

      <main className="workspace" id="tasks" ref={workspaceRef}>
        <header className="topbar">
          <div className="topbar-title">
            <p className="eyebrow">METACOGNITIVE GOVERNOR</p>
            <h1><span aria-hidden="true">▱</span>{activeSurfaceTitle}</h1>
            <p className="topbar-subtitle">Build, review, and run work with a local AI team.</p>
          </div>
          <div className="workspace-toolbar" aria-label="Workspace controls">
            <span className={`runtime-dot${localService && !localService.ready ? " status-error" : ""}`} title={localService?.detail} role="status"><i />{localService?.ready ? `v${localService.version}` : "Local runtime needs attention"}</span>
            <div className="workspace-toolbar-menu">
              <button className="workspace-icon-button" type="button" aria-label="Share" title="Share" aria-expanded={workspacePopover === "share"} onClick={() => setWorkspacePopover((current) => current === "share" ? null : "share")}><span aria-hidden="true">⇧</span></button>
              {workspacePopover === "share" && <div className="workspace-popover share-popover" role="menu" aria-label="Share workspace">
                <button type="button" role="menuitem" onClick={() => void shareWorkspaceSummary()}><span aria-hidden="true">⧉</span><span><strong>Copy share summary</strong><small>Chat, status, and project</small></span></button>
                <button type="button" role="menuitem" disabled={!workspaceProject} onClick={() => workspaceProject && void copyWorkspaceText(workspaceProject.root, "Project path copied")}><span aria-hidden="true">▱</span><span><strong>Copy project path</strong><small>{workspaceProjectName}</small></span></button>
              </div>}
            </div>
            <div className="workspace-toolbar-menu">
              <button className="open-in-button" type="button" disabled={!workspaceProject} aria-haspopup="menu" aria-expanded={workspacePopover === "open-in"} title={workspaceProject ? `Open ${workspaceProjectName} in another application` : "Choose a project first"} onClick={() => setWorkspacePopover((current) => current === "open-in" ? null : "open-in")}><span aria-hidden="true">▣</span>Open in<span aria-hidden="true">⌄</span></button>
              {workspacePopover === "open-in" && <div className="workspace-popover open-in-popover" role="menu" aria-label="Open project in">
                <div className="workspace-popover-heading"><strong>Open {workspaceProjectName} in</strong></div>
                {openInApplications.filter((application) => application.available).map((application) => <button type="button" role="menuitem" key={application.id} onClick={() => void openCurrentWorkspace(application.id)}><span className="open-in-glyph" aria-hidden="true">{application.id === "file_explorer" ? "▱" : application.id === "terminal" || application.id === "git_bash" ? ">_" : "◆"}</span><span>{application.label}</span></button>)}
                {openInApplications.filter((application) => application.available).length === 0 && <p>Available in the installed desktop app.</p>}
              </div>}
            </div>
            <button className="workspace-icon-button" type="button" disabled={!selectedChatId || !workspaceProject || codexReviewBusy} aria-label="Review uncommitted changes" title="Review uncommitted project changes in this owned Codex thread" onClick={() => void startConversationReview()}><span aria-hidden="true">✓</span></button>
            <button className={`workspace-icon-button${workspacePinnedSummaryOpen ? " active" : ""}`} type="button" aria-label="Toggle pinned summary" title="Toggle pinned summary" aria-pressed={workspacePinnedSummaryOpen} onClick={() => setWorkspacePinnedSummaryOpen((current) => !current)}><span aria-hidden="true">☷</span></button>
            <button className={`workspace-icon-button${workspaceBottomPanelOpen ? " active" : ""}`} type="button" aria-label="Toggle bottom panel" title="Toggle bottom panel (Ctrl+J)" aria-pressed={workspaceBottomPanelOpen} onClick={() => setWorkspaceBottomPanelOpen((current) => !current)}><span aria-hidden="true">▱</span></button>
            <button className={`workspace-icon-button${workspaceSidePanelOpen ? " active" : ""}`} type="button" aria-label="Toggle side panel" title="Toggle side panel (Ctrl+Alt+B)" aria-pressed={workspaceSidePanelOpen} onClick={() => setWorkspaceSidePanelOpen((current) => !current)}><span aria-hidden="true">◧</span></button>
          </div>
        </header>

        {workspacePinnedSummaryOpen && workspacePanelsVisible && <aside className="pinned-summary-card" aria-label="Pinned summary">
          <div><strong>{workspaceTask?.goal ?? activeSurfaceTitle}</strong><button type="button" aria-label="Close pinned summary" onClick={() => setWorkspacePinnedSummaryOpen(false)}>×</button></div>
          <p>{workspaceTask ? `This conversation is ${workspaceTask.state.replaceAll("_", " ")}.` : "No chat is selected."} {workspaceProject ? `Working in ${workspaceProjectName}.` : "Choose a project to enable workspace tools."}</p>
          {workspaceTask && <dl><div><dt>Status</dt><dd>{workspaceTask.state.replaceAll("_", " ")}</dd></div><div><dt>Project</dt><dd>{workspaceProjectName}</dd></div></dl>}
        </aside>}
        {workspaceActionStatus && <button className="workspace-action-status" type="button" title="Dismiss" onClick={() => setWorkspaceActionStatus(null)}>{workspaceActionStatus}<span aria-hidden="true">×</span></button>}

        {(advanced || activeSurface === "tasks") && <section className="hero-card">
          <div>
            <p className="eyebrow">NEW CHAT</p>
            <h2>What would you like to talk about?</h2>
            <p className="muted">
              Ask a question, explore an idea, or open a folder when you want MCG to
              work with a project. Governance stays with the conversation.
            </p>
          </div>
          <div className="composer">
            <label htmlFor="goal">Message</label>
            <textarea
              ref={goalInputRef}
              id="goal"
              value={taskGoal}
              onChange={(event) => setTaskGoal(event.target.value)}
              onKeyDown={handleComposerKeyDown}
              placeholder="Message MCG..."
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
              <PermissionSelector value={newChatRunPreferences.approval_mode} onChange={(approval_mode) => setNewChatRunPreferences((current) => ({ ...current, approval_mode }))} hasProject={Boolean(selectedProject)} />
              <ChatRunSelector providers={providers} codexModels={codexModels} value={newChatRunPreferences} onChange={setNewChatRunPreferences} discovering={modelDiscoveryBusy} discoveryError={modelDiscoveryError} onRefresh={() => void refreshCodexCatalog()} label="New chat model, effort, and speed" />
              {projects.length > 0 && <label className="project-select">
                <span className="sr-only">Project</span>
                <select
                  aria-label="Project"
                  value={selectedProject?.project_id ?? ""}
                  onChange={(event) => {
                    const project = projects.find((candidate) => candidate.project_id === event.target.value) ?? null;
                    setSelectedProject(project);
                  }}
                >
                  <option value="">None</option>
                  {projects.map((project) => <option value={project.project_id} key={project.project_id}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                </select>
              </label>}
              <button className="secondary icon-button folder-button" type="button" aria-label="Open folder" title="Create a project from a folder" disabled={taskBusy} onClick={() => chooseProject("task")}><span aria-hidden="true">▱</span></button>
              <button className="primary" type="button" disabled={taskBusy} onClick={() => void addTask()}>
                {taskBusy ? "Sending..." : "Send"}
              </button>
            </div>
            {taskError && <p className="error-message" role="alert">{taskError}</p>}
            {projectError && projectErrorSurface === "task" && <p className="error-message" role="alert">{projectError}</p>}
          </div>
        </section>}

        {(advanced || activeSurface === "threads" || (activeSurface === "tasks" && Boolean(workspaceTask))) && <section className="panel task-panel" aria-label={activeSurface === "threads" ? "Chat conversation" : "Tasks"}>
          {activeSurface === "threads" ? <div className="chat-conversation">
            <div className="chat-message-list">
              {!visibleConversation || (visibleConversation.messages.length === 0 && visibleConversation.turns.length === 0)
                ? <p className="empty-state">This conversation has no messages yet.</p>
                : visibleConversation.turns.map((turn) => <ConversationTurnTranscript
                  key={turn.id}
                  turn={turn}
                  messages={visibleConversation.messages.filter((message) => message.turn_id === turn.id)}
                  activities={(visibleConversation.activities ?? []).filter((activity) => activity.turn_id === turn.id)}
                  active={activeConversationTurn?.id === turn.id}
                />)}
              <CodingAgentCards team={visibleConversation?.team} conversationId={visibleConversation?.conversation.id} onView={(view) => setConversationViews((current) => ({ ...current, [view.conversation.id]: view }))} />
              {activeConversationTurn?.state === "waiting_for_approval" && activeConversationApproval && <section className="chat-permission" aria-label="Codex permission request">
                <strong>Permission needed to continue</strong>
                <p>{activeConversationApproval.reason}</p>
                <code>{activeConversationApproval.resource}</code>
                <div>
                  <button className="primary" type="button" disabled={conversationApprovalBusy} onClick={() => void resolveConversationApproval(activeConversationApproval.id, true)}>{conversationApprovalBusy ? "Applying…" : "Allow once"}</button>
                  <button className="secondary" type="button" disabled={conversationApprovalBusy} onClick={() => void resolveConversationApproval(activeConversationApproval.id, false)}>Deny</button>
                </div>
              </section>}
              {indeterminateConversationTurn && <section className="chat-recovery" aria-label="Codex turn recovery">
                <strong>Completion could not be proven</strong>
                <p>Codex accepted this turn before the harness disconnected. MCG will not resend it without your choice.</p>
                <div>
                  <button className="primary" type="button" disabled={codexRecoveryBusy} onClick={() => void recoverConversation("resume")}>Resume safely</button>
                  <button className="secondary" type="button" disabled={codexRecoveryBusy} onClick={() => void recoverConversation("retry")}>Retry as a new turn</button>
                  <button className="secondary" type="button" disabled={codexRecoveryBusy} onClick={() => void recoverConversation("fork")}>Fork</button>
                </div>
              </section>}
              {visibleConversation?.turns.filter((turn) => turn.error).map((turn) => <p className="error-message" role="alert" key={`${turn.id}-error`}>{turn.error}</p>)}
            </div>
            <div className={`chat-composer-stack${visibleQueuedMessages.length > 0 ? " has-queue" : ""}`}>
              {(visibleConversation?.team?.delegations.filter((delegation) => ["requested", "preparing", "running", "waiting", "blocked", "reported", "integrating"].includes(delegation.state)).length ?? 0) > 0 && <div className="coding-agent-active-count" role="status">
                <span className="thinking-dots" aria-hidden="true"><i /><i /><i /></span>
                {visibleConversation?.team?.delegations.filter((delegation) => ["requested", "preparing", "running", "waiting", "blocked", "reported", "integrating"].includes(delegation.state)).length} coding agent{visibleConversation?.team?.delegations.filter((delegation) => ["requested", "preparing", "running", "waiting", "blocked", "reported", "integrating"].includes(delegation.state)).length === 1 ? "" : "s"} active
              </div>}
              {visibleQueuedMessages.length > 0 && <section className="chat-queue-drawer" aria-label="Queued messages">
                <div className="chat-queue-heading">
                  <span aria-hidden="true">Ⅱ</span>
                  <strong>{conversationIsActive ? `${visibleQueuedMessages.length} queued` : queueDispatchErrorId ? "Queue paused" : "Sending next queued message"}</strong>
                </div>
                {visibleQueuedMessages.map((message) => <div className="chat-queue-row" key={message.id}>
                  <span className="chat-queue-icon" aria-hidden="true">↳</span>
                  <span className="chat-queue-content" title={message.content}>{message.content}</span>
                  {conversationIsActive
                    ? <button className="chat-queue-steer" type="button" disabled={steeringMessageId !== null} onClick={() => void steerMessage(message.content, message.id)}>{steeringMessageId === message.id ? "Steering…" : "↪ Steer"}</button>
                    : queueDispatchErrorId === message.id && <button className="chat-queue-steer" type="button" onClick={() => setQueueDispatchErrorId(null)}>Retry</button>}
                  <button className="chat-queue-remove" type="button" aria-label="Remove queued message" title="Remove" onClick={() => setQueuedChatMessages((current) => current.filter((item) => item.id !== message.id))}>⌫</button>
                </div>)}
              </section>}
              <div className="chat-followup-composer">
              <textarea aria-label="Message" value={taskGoal} onChange={(event) => setTaskGoal(event.target.value)} onKeyDown={handleComposerKeyDown} placeholder="Message MCG..." />
              <div className="composer-actions"><PermissionSelector value={newChatRunPreferences.approval_mode} onChange={(approval_mode) => setNewChatRunPreferences((current) => ({ ...current, approval_mode }))} hasProject={Boolean(workspaceProject)} /><ChatRunSelector providers={providers} codexModels={codexModels} value={newChatRunPreferences} onChange={setNewChatRunPreferences} discovering={modelDiscoveryBusy} discoveryError={modelDiscoveryError} onRefresh={() => void refreshCodexCatalog()} label="Chat model, effort, and speed" /><button className="primary" type="button" disabled={!taskGoal.trim() || steeringMessageId !== null} title={conversationIsActive ? "Queue message (Ctrl+Enter steers now)" : "Send message"} onClick={() => conversationIsActive ? queueDraftMessage() : void addTask()}>{conversationIsActive ? "Queue" : "Send"}</button></div>
              {taskError && <p className="error-message" role="alert">{taskError}</p>}
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
                    {advanced && <code>{approvals.find((approval) => approval.task_id === task.id)?.resource}</code>}
                  </div>}
                  {taskEvidence(task)}
                </div>
                <div className="task-actions">
                  <ChatRunSelector
                    providers={providers}
                    value={taskRunPreferences[task.id] ?? newChatRunPreferences}
                    onChange={(preferences) => setTaskRunPreferences((current) => ({ ...current, [task.id]: preferences }))}
                    discovering={modelDiscoveryBusy}
                    discoveryError={modelDiscoveryError}
                    onRefresh={() => void discoverModelCatalogs(providers, true)}
                    label={`Model, effort, and speed for ${task.goal}`}
                  />
                  {task.state === "pending" && <label className="task-team-select"><span>Team</span><select aria-label={`Team for ${task.goal}`} value={taskOrganizationIds[task.id] ?? ""} onChange={(event) => setTaskOrganizationIds((current) => ({ ...current, [task.id]: event.target.value }))}>
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
          {!advanced ? <div className="pull-request-workspace">
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
              <div className="pull-request-search-row">
                <label className="pull-request-search"><span aria-hidden="true">⌕</span><span className="sr-only">Search pull requests</span><input type="search" aria-label="Search pull requests" value={codeReviewSearch} onChange={(event) => setCodeReviewSearch(event.target.value)} placeholder="Search pull requests" /></label>
                <div className="pull-request-filter-menu">
                  <button className={`pull-request-filter${codeReviewFilterOpen || codeReviewStatusFilter !== "all" ? " active" : ""}`} type="button" aria-label="Filter pull requests" title="Filter" aria-expanded={codeReviewFilterOpen} onClick={() => setCodeReviewFilterOpen((current) => !current)}><svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 5h16l-6.2 7.1v5.2l-3.6 1.8v-7z" /></svg></button>
                  {codeReviewFilterOpen && <div className="pull-request-filter-popover" role="menu" aria-label="Pull request filters"><strong>Review state</strong>{([['all', 'All'], ['open', 'Open'], ['finished', 'Finished']] as const).map(([value, label]) => <button type="button" role="menuitemradio" aria-checked={codeReviewStatusFilter === value} onClick={() => { setCodeReviewStatusFilter(value); setCodeReviewFilterOpen(false); }} key={value}><span aria-hidden="true">{codeReviewStatusFilter === value ? "●" : "○"}</span>{label}</button>)}<hr /><button type="button" role="menuitem" onClick={() => { setCodeReviewFilterOpen(false); setCodeReviewSetupOpen(true); setSelectedCodeReviewTaskId(null); }}><span aria-hidden="true">＋</span>New local review</button></div>}
                </div>
              </div>
              <div className="pull-request-group-heading"><span>{codeReviewTab === "reviewing" ? "Reviewing" : codeReviewTab === "authored" ? "Authored locally" : "Local reviews"}</span><span>{filteredCodeReviewTasks.length}</span></div>
              <div className="pull-request-list">
                {filteredCodeReviewTasks.length === 0 ? <div className="pull-request-list-empty"><strong>{codeReviewSearch.trim() ? "No matching reviews" : codeReviewTab === "reviewing" ? "No reviews in progress" : "No local reviews yet"}</strong><span>{codeReviewSearch.trim() ? "Try another search." : "Start a read-only review of a Git project."}</span><button type="button" onClick={() => { setCodeReviewSetupOpen(true); setSelectedCodeReviewTaskId(null); }}>Start local review</button></div> : filteredCodeReviewTasks.map((task) => {
                  const project = projects.find((candidate) => candidate.project_id === task.project_id);
                  const projectName = project?.root.split(/[\\/]/).filter(Boolean).at(-1) ?? "Repository unavailable";
                  return <button className={`pull-request-row${selectedCodeReviewTaskId === task.id ? " selected" : ""}`} type="button" aria-pressed={selectedCodeReviewTaskId === task.id} onClick={() => { setSelectedCodeReviewTaskId(task.id); setCodeReviewSetupOpen(false); }} key={task.id}>
                    <span className="pull-request-state-icon" data-state={task.state} aria-hidden="true">⑂</span>
                    <span className="pull-request-row-copy"><strong>{task.goal.replace(CODE_REVIEW_GOAL_PREFIX, "").trim() || "Code review"}</strong><small>{projectName} · {task.state.replaceAll("_", " ")}</small></span>
                    <time dateTime={task.updated_at}>{new Date(task.updated_at).toLocaleDateString(undefined, { month: "short", day: "numeric" })}</time>
                  </button>;
                })}
              </div>
              <p className="pull-request-sync-note">GitHub titles, comments, checks, and changed files will appear after GitHub is connected. Local Git review works now.</p>
            </section>
            <section className="pull-request-detail-pane" aria-label="Pull request details">
              {codeReviewSetupOpen ? <div className="pull-request-review-setup">
                <div className="pull-request-detail-heading"><span className="pull-request-detail-icon" aria-hidden="true">⑂</span><div><h2>Start a local review</h2><p>Review code without changing the repository.</p></div></div>
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
                <header><span className="pull-request-detail-icon" aria-hidden="true">⑂</span><div><span className={`task-state state-${selectedCodeReviewTask.state}`}>{selectedCodeReviewTask.state.replaceAll("_", " ")}</span><h2>{selectedCodeReviewTask.goal.replace(CODE_REVIEW_GOAL_PREFIX, "").trim() || "Code review"}</h2><p>{projects.find((project) => project.project_id === selectedCodeReviewTask.project_id)?.root ?? "Repository unavailable"}</p></div></header>
                <div className="pull-request-detail-summary"><div><span>Review type</span><strong>Local, read-only</strong></div><div><span>Updated</span><strong>{new Date(selectedCodeReviewTask.updated_at).toLocaleString()}</strong></div></div>
                <p>The review findings and discussion live in the associated MCG chat.</p>
                <button className="primary" type="button" onClick={() => openChat(selectedCodeReviewTask.id)}>Open review</button>
              </article> : <div className="pull-request-detail-empty"><span aria-hidden="true">⑂</span><strong>Select a review to view</strong><p>Choose a local review from the list, or start a new one.</p></div>}
            </section>
          </div> : <>
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
          </>}
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
            <div><p className="eyebrow">GOVERNED AGENTS</p><h2>Agents</h2><p className="muted">See how MCG governs teams, assignments, and live work.</p></div>
            <button className="secondary" type="button" disabled={agentBusy} onClick={() => void refreshAgents()}>{agentBusy ? "Refreshing…" : "Refresh"}</button>
          </div>
          <nav className="segmented-tabs agents-tabs" aria-label="Agents pages">
            {(["overview", "teams", "activity"] as const).map((tab) => <button type="button" className={agentTab === tab ? "active" : ""} aria-current={agentTab === tab ? "page" : undefined} onClick={() => setAgentTab(tab)} key={tab}>{tab.charAt(0).toUpperCase() + tab.slice(1)}</button>)}
            {advanced && <button type="button" onClick={() => navigateSurface("improvement")}>Improvement</button>}
          </nav>
          {agentError && <p className="error-message" role="alert">{agentError}</p>}

          {agentTab === "overview" && <div className="agents-stack">
            <article className="agent-governor-card">
              <div className="governor-mark" aria-hidden="true">MCG</div>
              <div className="agent-card-copy"><span className="agent-kicker">DETERMINISTIC RUNTIME · NOT A MODEL AGENT</span><h3>MCG runtime</h3><p>Enforces owner policy above the Chat and Coding Agents. It brokers tools, permissions, budgets, leases, memory, and evidence.</p></div>
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
                    <div className="subsystem-row"><span className={`health-dot health-${codexStableDiscovery ? "healthy" : "degraded"}`} aria-hidden="true" /><span><strong>Stable Codex capabilities</strong><small>{codexStableDiscovery ? `${codexStableDiscovery.skills.length} skills · ${codexStableDiscovery.hooks.length} hooks` : "Discovery unavailable"}{codexStableDiscovery?.appsUnavailableReason ? ` · Apps excluded: experimental in 0.144.6` : ""}</small></span><em>{codexStableDiscovery ? "loaded" : "checking"}</em></div>
                    <div className="subsystem-row"><span className="health-dot health-healthy" aria-hidden="true" /><span><strong>Harness lifecycle</strong><small>{codexHarnessStatus.last_unknown_notification ? `Unknown notification retained: ${codexHarnessStatus.last_unknown_notification}` : "Unknown notifications are retained for diagnostics"}</small></span><em>{codexHarnessStatus.restart_count} starts</em></div>
                  </>}
                </div>
              </details>
            </article>

            <article className="agent-section-card">
              <div className="section-heading compact-heading"><div><h3>Live conversation agents</h3><p className="muted">Only the user-facing Chat Agent and currently working Coding Agents are shown.</p></div></div>
              <div className="instance-list">
                {(visibleConversation?.team?.sessions ?? []).filter((session) => session.kind === "chat" || (session.kind === "coder" && ["starting", "working", "waiting"].includes(session.state))).map((session) => <div className="instance-row" key={session.id}><span><strong>{session.kind === "chat" ? "Chat Agent" : "Coding Agent"}</strong><small>{session.model ?? "Automatic model"} · prompt revision {session.prompt_revision}</small></span><em>{session.state}</em></div>)}
                {!visibleConversation?.team?.sessions.some((session) => session.kind === "chat" || (session.kind === "coder" && ["starting", "working", "waiting"].includes(session.state))) && <p className="muted">Open a conversation to inspect its Chat Agent and active Coding Agents.</p>}
              </div>
            </article>

            <article className="agent-section-card topology-card">
              <div className="section-heading compact-heading"><div><h3>Active team</h3><p className="muted">{activeOrganization ? `${activeOrganization.name} · revision ${activeOrganization.revision}` : "Loading assignment…"}</p></div>{activeOrganization && <button className="secondary" type="button" onClick={() => editOrganization(activeOrganization)}>Edit team</button>}</div>
              {activeOrganization && <><div className="topology-diagram" aria-hidden="true"><div className="topology-governor">MCG runtime</div><span className="topology-line" /><div className="topology-team">{activeOrganization.name}</div><div className="topology-roles">{activeOrganization.roles.filter((role) => role.kind === "conversation" || role.kind === "coder").map((role) => <div className={`topology-role role-${role.kind}`} key={role.id}><strong>{role.name}</strong><small>{role.kind}</small></div>)}</div></div><ul className="topology-accessible-list" aria-label="Active conversation roles">{activeOrganization.roles.filter((role) => role.kind === "conversation" || role.kind === "coder").map((role) => <li key={`accessible-${role.id}`}><strong>{role.name}</strong><span>{role.kind.replaceAll("_", " ")} · {role.purpose}</span></li>)}</ul></>}
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
                {organizationDraft.roles.map((role, index) => <fieldset className="role-editor" key={role.id || `new-role-${index}`}><legend>{role.name || `Role ${index + 1}`}</legend><div className="editor-grid"><label>Name<input value={role.name} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, name: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label><label>Kind<select value={role.kind} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, kind: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }}>{["executive", "conversation", "coder", "evaluator", "tool_worker", "researcher", "tester", "temporary_specialist"].map((kind) => <option value={kind} key={kind}>{kind.replaceAll("_", " ")}</option>)}</select></label></div><label>Purpose<textarea value={role.purpose} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, purpose: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label>{advanced && <div className="advanced-fields"><label>Prompt policy<textarea value={role.prompt_policy} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, prompt_policy: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label><label>Preferred model<select value={role.model_policy.preferred_models[0] ?? ""} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, model_policy: { ...role.model_policy, preferred_models: event.target.value ? [event.target.value] : [] } }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }}><option value="">Automatic routing</option>{discoveredModels.map((model) => <option value={model.id} key={`${role.id}-${model.id}`}>{model.display_name}</option>)}</select></label><label>Memory namespace<input value={role.memory_namespace} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, memory_namespace: event.target.value }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label><label>Tool capabilities<input value={role.tool_capabilities.join(", ")} onChange={(event) => { const roles = [...organizationDraft.roles]; roles[index] = { ...role, tool_capabilities: event.target.value.split(",").map((value) => value.trim()).filter(Boolean) }; setOrganizationDraft({ ...organizationDraft, roles }); setOrganizationPreview(null); }} /></label></div>}</fieldset>)}
              </div>
              {advanced && <div className="relationship-editor"><div className="section-heading compact-heading"><h4>Relationships</h4><button className="secondary" type="button" disabled={organizationDraft.roles.filter((role) => role.id).length < 2} onClick={() => { const roles = organizationDraft.roles.filter((role) => role.id); setOrganizationDraft({ ...organizationDraft, relationships: [...organizationDraft.relationships, { from: roles[0].id, to: roles[1].id, kind: "delegates_to" }] }); setOrganizationPreview(null); }}>Add relationship</button></div>{organizationDraft.relationships.map((relationship, index) => <div className="relationship-row" key={`${relationship.from}-${relationship.to}-${index}`}><select aria-label="From role" value={relationship.from} onChange={(event) => { const relationships = [...organizationDraft.relationships]; relationships[index] = { ...relationship, from: event.target.value }; setOrganizationDraft({ ...organizationDraft, relationships }); setOrganizationPreview(null); }}>{organizationDraft.roles.filter((role) => role.id).map((role) => <option value={role.id} key={`from-${index}-${role.id}`}>{role.name}</option>)}</select><span>→</span><select aria-label="Relationship kind" value={relationship.kind} onChange={(event) => { const relationships = [...organizationDraft.relationships]; relationships[index] = { ...relationship, kind: event.target.value }; setOrganizationDraft({ ...organizationDraft, relationships }); setOrganizationPreview(null); }}>{["delegates_to", "reviews", "escalates_to", "reports_to", "shares_information_with", "observed_by"].map((kind) => <option value={kind} key={kind}>{kind.replaceAll("_", " ")}</option>)}</select><span>→</span><select aria-label="To role" value={relationship.to} onChange={(event) => { const relationships = [...organizationDraft.relationships]; relationships[index] = { ...relationship, to: event.target.value }; setOrganizationDraft({ ...organizationDraft, relationships }); setOrganizationPreview(null); }}>{organizationDraft.roles.filter((role) => role.id).map((role) => <option value={role.id} key={`to-${index}-${role.id}`}>{role.name}</option>)}</select><button className="link-button" type="button" onClick={() => { setOrganizationDraft({ ...organizationDraft, relationships: organizationDraft.relationships.filter((_, candidate) => candidate !== index) }); setOrganizationPreview(null); }}>Remove</button></div>)}</div>}
              <div className="composer-actions"><button className="primary" type="button" disabled={agentBusy} onClick={() => void previewOrganizationChanges()}>{agentBusy ? "Validating…" : "Review changes"}</button></div>
              {organizationPreview && <div className="revision-preview"><h4>Review revision {organizationPreview.organization.revision}</h4>{organizationPreview.changes.length === 0 ? <p className="muted">No material changes detected.</p> : <ul>{organizationPreview.changes.map((change) => <li key={change}>{change}</li>)}</ul>}{organizationPreview.warnings.length > 0 && <div className="warning-box"><strong>Warnings</strong><ul>{organizationPreview.warnings.map((warning) => <li key={warning}>{warning}</li>)}</ul></div>}<label>Change summary<input value={organizationChangeSummary} onChange={(event) => setOrganizationChangeSummary(event.target.value)} placeholder="Why this revision should become active" /></label><button className="primary" type="button" disabled={agentBusy || !organizationChangeSummary.trim() || organizationPreview.changes.length === 0} onClick={() => void publishOrganizationChanges()}>Confirm and activate revision</button></div>}
            </article>}
          </div>}

          {agentTab === "activity" && <div className="activity-layout"><div className="agent-run-list" aria-label="Agent runs">{agentRuns.length === 0 ? <p className="empty-state">No agent runs yet.</p> : agentRuns.map((view) => <button className={`agent-run-row${selectedAgentRunId === view.run.id ? " active" : ""}`} type="button" onClick={() => setSelectedAgentRunId(view.run.id)} key={view.run.id}><span><strong>{tasks.find((task) => task.id === view.run.task_id)?.goal ?? "Agent run"}</strong><small>{new Date(view.run.updated_at).toLocaleString()} · {view.plan.selected_roles.length} roles</small></span><em className={`agent-state state-${view.run.state}`}>{view.run.state}</em></button>)}</div><article className="agent-section-card run-inspector">{!selectedAgentRun ? <div className="empty-state">Select a run to inspect its agents and evidence.</div> : <><div className="section-heading compact-heading"><div><h3>{selectedAgentTask?.goal ?? "Agent run"}</h3><p className="muted">Pinned revision <code>{selectedAgentRun.run.organization_revision_id}</code></p></div><div className="card-actions">{selectedAgentTask?.state === "blocked" && <button className="primary" type="button" disabled={taskBusy} onClick={() => void retryBlockedTask(selectedAgentTask.id)}>Retry task</button>}{selectedAgentTask?.state === "waiting_for_approval" && selectedAgentApproval && <button className="primary" type="button" disabled={taskBusy} onClick={() => void approveAndResume(selectedAgentTask.id, selectedAgentApproval.id)}>Approve and resume</button>}{["planned", "running", "waiting", "recovering"].includes(selectedAgentRun.run.state) && <button className="secondary" type="button" disabled={agentBusy} onClick={() => void cancelRun(selectedAgentRun.run.id)}>Cancel run</button>}</div></div><p>{selectedAgentRun.plan.selection_reason}</p><div className="run-metrics"><span><strong>{selectedAgentRun.instances.length}</strong><small>agents</small></span><span><strong>{selectedAgentRun.plan.items.length}</strong><small>work items</small></span><span><strong>{selectedAgentRun.usage.reduce((sum, usage) => sum + usage.input_tokens + usage.output_tokens, 0).toLocaleString()}</strong><small>tokens</small></span><span><strong>{selectedAgentRun.evaluations.length}</strong><small>evaluations</small></span></div><div className="instance-list">{selectedAgentRun.instances.map((instance) => { const role = organizations.flatMap((organization) => organization.roles).find((candidate) => candidate.id === instance.role_id); return <div className="instance-row" key={instance.id}><span><strong>{role?.name ?? instance.role_id}</strong><small>{instance.model ? `${instance.model.model_id} · ${instance.model.route_reason}` : "Model not routed yet"}</small></span><em>{instance.state}</em></div>; })}</div>{advanced && <><details><summary>Work plan and leases</summary><ul>{selectedAgentRun.plan.items.map((item) => <li key={item.id}><strong>{item.goal}</strong> · {item.state}</li>)}</ul><ul>{selectedAgentRun.leases.map((lease) => <li key={lease.id}><strong>{lease.goal}</strong> · {lease.state} · {lease.budget.consumed_tokens.toLocaleString()} tokens</li>)}</ul></details><details><summary>Evaluations and replay evidence</summary>{selectedAgentRun.evaluations.length === 0 ? <p className="muted">No evaluation recorded yet.</p> : selectedAgentRun.evaluations.map((evaluation) => <div className="evaluation-row" key={evaluation.id}><strong>{evaluation.outcome}</strong><span>{evaluation.confidence.basis_points / 100}% confidence</span>{evaluation.evidence.map((evidence, index) => <small key={`${evaluation.id}-${index}`}>{evidence.passed ? "Pass" : "Fail"} · {evidence.method}: {evidence.detail}</small>)}</div>)}</details></>}</>}</article></div>}
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
                        <small>{project ? project.root.split(/[\\/]/).filter(Boolean).at(-1) : "OpenAI Sites"}{deployment ? ` · ${deployment.state}` : " · Ready to refine"}</small>
                      </div>
                      <div className="site-library-actions">
                        <button className="secondary" type="button" title="Uses MCG's separate, persistent browser profile" onClick={() => void openSiteInApp(entry)}>Open in MCG browser</button>
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
                  : "Finish signing in in your browser. MCG will connect automatically."}</p>
                <button className="link-button" type="button" onClick={() => openVerification(login.verification_url)}>
                  {login.user_code ? "Open device authorization page" : "Open sign-in page"}
                </button>
                {login.user_code && <p className="device-code">Code: <strong>{login.user_code}</strong></p>}
                <small>{login.user_code
                  ? "MCG checks the device authorization automatically after you enter the code."
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
            <div className="settings-nav-shell">
            <nav className="settings-nav" aria-label="Settings sections">
              {settingsGroups.map((group) => {
                const items = group.items.filter((item) => !normalizedSettingsSearch || item.label.toLocaleLowerCase().includes(normalizedSettingsSearch));
                return items.length > 0 && <section key={group.label}><h3>{group.label}</h3>{items.map((item) => <button className={settingsTab === item.id ? "active" : ""} type="button" aria-current={settingsTab === item.id ? "page" : undefined} onClick={() => { setSettingsTab(item.id); window.scrollTo({ top: 0, behavior: "auto" }); }} key={item.id}><span aria-hidden="true">{item.glyph}</span>{item.label}</button>)}</section>;
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
                  <div><h2 id="settings-heading">General</h2><p className="muted">Choose how MCG works by default.</p></div>
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
                </div><h3>General</h3><div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Default file destination</strong><small>Files and folders remain in their attached project or explicit global folder.</small></span><span className="count-badge">Project</span></div>
                  <div className="settings-control"><span><strong>Agent environment</strong><small>MCG runs locally on this computer through the governed daemon.</small></span><span className="count-badge">Local</span></div>
                  <div className="settings-control"><span><strong>Language</strong><small>Language for the app interface</small></span><span className="count-badge">Auto detect</span></div>
                  <div className="settings-control"><span><strong>Suggested prompts</strong><small>Show focused starting points on consumer workflows.</small></span><span className="count-badge">On</span></div>
                </div><h3>Composer</h3><div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Send shortcut</strong><small>Enter sends a prompt; Shift+Enter inserts a line.</small></span><span className="count-badge">Enter</span></div>
                  <div className="settings-control"><span><strong>Follow-up behavior</strong><small>New messages are queued while MCG is working.</small></span><span className="count-badge">Queue</span></div>
                </div><h3>Notifications</h3><div className="settings-card settings-card-stack"><div className="settings-control"><span><strong>Permission and question notifications</strong><small>MCG surfaces requests that require owner input without pretending a background action completed.</small></span><span className="count-badge">On</span></div></div></> : <p className="empty-state">Loading durable settings…</p>}
              </div>
              <div className="settings-pane" data-settings-pane="import">
                <div className="section-heading"><div><h2>Import</h2><p className="muted">Bring local folders and portable MCG data into this installation.</p></div></div>
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
          <h3>Application</h3>
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
                <div className="settings-card"><div><strong>Private by design</strong><p>MCG shows only activity recorded by this local installation. It does not invent streaks, public handles, or subscription details that providers do not expose.</p></div><button className="secondary" type="button" onClick={() => setSettingsTab("account")}>Manage account</button></div>
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
                  <div className="settings-control"><span><strong>UI font size</strong><small>Base size used throughout MCG</small></span><span className="count-badge">14 px</span></div>
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
                <div className="settings-card settings-card-stack settings-unavailable"><div className="settings-control"><span><strong>Dictation controls</strong><small>MCG will not request microphone authority or save recordings until this capability is implemented and tested.</small></span><span className="count-badge">Unavailable</span></div></div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="personalization">
                <h2>Personalization</h2>
                <h3>Custom instructions</h3>
                <p className="muted">Give MCG durable guidance for future chats.</p>
                <textarea className="settings-instructions" aria-label="Custom instructions" disabled placeholder="Custom instructions are not connected to the runtime yet." />
                <h3>Working style</h3>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Autonomy</strong><small>Choose how independently MCG acts within its owner constitution.</small></span><button className="secondary" type="button" onClick={() => setSettingsTab("general")}>{userSettings?.autonomy ?? "Loading"}</button></div>
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
                <p className="form-note">Only shortcuts that work in this MCG build are listed. Shortcut reassignment will appear after conflict-safe binding is implemented.</p>
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
                {!signedInWithOpenAi && <div className="settings-empty-card"><strong>Connect ChatGPT to see Codex usage</strong><p>Sign in from Account, then MCG can show the account plan and rolling limits OpenAI reports.</p><button className="secondary" type="button" onClick={() => setSettingsTab("account")}>Open Account</button></div>}
                <h3>Provider billing</h3>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>ChatGPT subscription</strong><small>{signedInWithOpenAi ? `${openAiPlanLabel}. Connected through explicit OpenAI OAuth.` : "Not connected. You can sign in from Account without enabling API-key billing."}</small></span><button className="secondary" type="button" onClick={() => setSettingsTab("account")}>{signedInWithOpenAi ? "Manage" : "Connect"}</button></div>
                  <div className="settings-control"><span><strong>API and compatible providers</strong><small>API keys, OpenRouter, and OpenAI-compatible endpoints are billed by their provider. Unknown custom-model costs remain unknown.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("providers")}>Provider details</button></div>
                </div>
                <p className="form-note">MCG does not sell credits, collect subscription payments, or silently move subscription work to API billing.</p>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="account"><h2>Account</h2><div className="settings-card account-card"><AccountAvatar className="owner-avatar" profile={signedInWithOpenAi ? openAiProfile : undefined} avatarUrl={signedInWithOpenAi ? openAiAvatarUrl : undefined} fallback={signedInWithOpenAi ? "O" : "M"} /><div><strong>{signedInWithOpenAi ? openAiAccountName : "Connect your OpenAI subscription"}</strong><p>{signedInWithOpenAi ? `Signed in with ChatGPT${codexAccount?.email ? ` as ${codexAccount.email}` : openAiProfile?.email ? ` as ${openAiProfile.email}` : ""}.` : "A browser window will open for OpenAI sign-in and return authorization to MCG automatically."}</p><small>{signedInWithOpenAi ? `${openAiPlanLabel} · Using shared Codex account` : "Subscription OAuth never silently falls back to API-key billing."}</small></div>{signedInWithOpenAi ? <button className="secondary" type="button" disabled={busy} onClick={() => void logOutOfOpenAi()}>{busy ? "Disconnecting…" : "Disconnect MCG"}</button> : <button className="primary" type="button" disabled={busy || Boolean(login || codexLogin)} onClick={() => void startLogin("browser")}>{busy ? "Opening sign-in…" : "Sign in with ChatGPT"}</button>}</div>
                {codexAccount?.authenticated && <div className="settings-card settings-card-stack"><div className="settings-control"><span><strong>Shared Codex authentication</strong><small>MCG uses the global Codex account without changing its plugins or settings.</small></span><button className="secondary" type="button" disabled={busy} onClick={() => void signOutOfCodexDevice()}>Sign out of Codex on this device</button></div></div>}
                {(login || codexLogin) && <div className="oauth-card" role="status"><p>Finish signing in in your browser. MCG will connect automatically.</p><button className="link-button" type="button" onClick={() => openVerification(codexLogin?.authUrl ?? login?.verification_url ?? "")}>Open sign-in page</button><small>The callback listener closes after 3 minutes. If it expires, the sign-in option will return here.</small></div>}
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
                <div className="settings-page-heading"><h2>Browser</h2><p>Manage MCG's governed browser sessions and site boundaries.</p></div>
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
                <div className="settings-page-heading"><h2>Computer use</h2><p>Manage how MCG uses other applications on this computer.</p></div>
                <div className="settings-section-heading"><strong>Control</strong><span>{activeComputerSessions.length} active</span></div>
                <div className="settings-integration-list" aria-label="Computer use controls">
                  <div className="settings-integration-row"><span className="settings-app-icon settings-app-icon-gradient" aria-hidden="true">⌁</span><span className="settings-integration-copy"><strong>Governed app control</strong><small>{computerCapability?.detail ?? "Checking computer-use capability…"}</small></span><label className="settings-switch"><span className="sr-only">Computer use availability</span><input type="checkbox" checked={computerCapability?.availability === "available"} readOnly disabled /><i aria-hidden="true" /></label></div>
                  <div className="settings-integration-row"><span className="settings-app-icon" aria-hidden="true">◎</span><span className="settings-integration-copy"><strong>Screen reading</strong><small>Captured frames remain scoped to an explicit session and target region.</small></span><span className="count-badge">Governed</span></div>
                  <div className="settings-integration-row"><span className="settings-app-icon" aria-hidden="true">⌨</span><span className="settings-integration-copy"><strong>Input control</strong><small>Mouse, keyboard, and clipboard capabilities must be granted per session.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("platform")}>Manage</button></div>
                </div>
                <div className="settings-section-heading"><strong>Always-allowed apps</strong></div>
                <div className="settings-empty-card"><p>None yet</p></div>
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="hooks"><h2>Hooks</h2><div className="settings-card"><div><strong>No hooks configured</strong><p>Hook management will appear here when the typed, governed hook runtime is implemented.</p></div></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="connections"><h2>Connections</h2><div className="settings-card"><div><strong>Providers and tools</strong><p>Manage model providers, ChatGPT subscription access, MCP servers, and remote hosts.</p></div><div className="settings-card-actions"><button className="secondary" type="button" onClick={() => navigateSurface("providers")}>Providers</button><button className="secondary" type="button" onClick={() => navigateSurface("mcp")}>Tools</button><button className="secondary" type="button" onClick={() => navigateSurface("ssh")}>SSH</button></div></div></div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="git">
                <div className="settings-page-heading"><h2>Git</h2><p>Choose defaults for governed repository workflows.</p></div>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Branch prefix</strong><small>Prefix used when MCG creates a task branch.</small></span><span className="settings-value-field">codex/</span></div>
                  <div className="settings-control"><span><strong>Pull request review</strong><small>Reviews are read-only and findings remain separate from fixes.</small></span><button className="secondary" type="button" onClick={() => navigateSurface("pull-requests")}>Open reviews</button></div>
                  <div className="settings-control"><span><strong>Force push</strong><small>MCG does not enable destructive history rewriting by default.</small></span><span className="count-badge">Off</span></div>
                  <div className="settings-control"><span><strong>Review delivery</strong><small>Keep review findings in the current task when possible.</small></span><span className="count-badge">Inline</span></div>
                </div>
                <div className="settings-section-heading"><span><strong>Commit instructions</strong><small>Project and repository instructions remain the source of commit guidance.</small></span></div>
                <textarea className="settings-instructions" aria-label="Commit instructions" disabled placeholder="Custom commit guidance is not connected yet." />
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="environments">
                <div className="settings-page-heading settings-page-heading-actions"><div><h2>Environments</h2><p>Local environments tell MCG how each attached project is set up.</p></div><button className="secondary" type="button" onClick={() => navigateSurface("projects")}>Add project</button></div>
                <div className="settings-section-heading"><strong>Select a project</strong><span>{projects.length} attached</span></div>
                {projects.length === 0 ? <div className="settings-empty-card"><strong>No projects attached</strong><p>Add a project to discover its repository, languages, tools, and runnable commands.</p><button className="secondary" type="button" onClick={() => navigateSurface("projects")}>Add project</button></div> : <div className="settings-project-list" aria-label="Project environments">{projects.map((project) => <button type="button" onClick={() => navigateSurface("projects")} key={project.id}><span className="settings-app-icon" aria-hidden="true">▤</span><span><strong>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.project_id}</strong><small>{project.repository.is_git ? `${project.repository.branch ?? "Git repository"}${project.repository.dirty ? " · changes present" : ""}` : "Local project"}</small></span><i aria-hidden="true">＋</i></button>)}</div>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="worktrees">
                <div className="settings-page-heading"><h2>Worktrees</h2><p>Review isolated task workspace defaults and managed worktrees.</p></div>
                <div className="settings-card settings-card-stack">
                  <div className="settings-control"><span><strong>Worktree root</strong><small>Managed worktrees will use an owner-controlled local directory.</small></span><span className="settings-value-field">Runtime managed</span></div>
                  <div className="settings-control"><span><strong>Cleanup policy</strong><small>Integrated worktrees are removed; failed, cancelled, conflicting, and unapplied work is retained.</small></span><span className="count-badge">Safe retention</span></div>
                  <div className="settings-control"><span><strong>Isolation policy</strong><small>Scheduled and coding work must remain within reviewed project boundaries.</small></span><span className="count-badge">Governed</span></div>
                </div>
                <div className="settings-section-heading"><strong>Managed worktrees</strong><span>{managedConversationWorktrees.length}</span></div>
                {managedConversationWorktrees.length === 0 ? <div className="settings-empty-card"><p>No managed coding worktrees have been created in loaded conversations.</p></div> : <div className="settings-card settings-card-stack" aria-label="Managed worktrees">{managedConversationWorktrees.map(({ worktree, conversationId, delegation }) => {
                  const blockingExclusions = worktree.baseline_exclusions?.filter((entry) => entry.review_blocker).length ?? 0;
                  const journal = worktree.integration_journal;
                  return <div className="settings-control" key={worktree.id}><span><strong>{worktree.branch}</strong><small>{worktree.state}{worktree.retained_reason ? ` · ${worktree.retained_reason}` : ""} · {(worktree.storage_bytes / (1024 * 1024)).toFixed(1)} MiB</small><small>{(worktree.workspace_mode ?? "git_worktree").replaceAll("_", " ")} · baseline {(worktree.baseline_confidence ?? "verified").replaceAll("_", " ")}{blockingExclusions > 0 ? ` · ${blockingExclusions} review blocker${blockingExclusions === 1 ? "" : "s"}` : ""}{journal ? ` · integration ${journal.state.replaceAll("_", " ")}` : ""}</small></span><span className="settings-inline-actions"><button className="secondary" type="button" onClick={() => openChat(conversationId)}>Review</button>{worktree.state !== "removed" && delegation && !["requested", "preparing", "running", "waiting", "integrating"].includes(delegation.state) && <button className="secondary danger" type="button" onClick={() => void discardConversationDelegation(conversationId, delegation.id, worktree.revision).then((view) => setConversationViews((current) => ({ ...current, [conversationId]: view }))).catch((error) => setTaskError(errorMessage(error)))}>Discard</button>}</span></div>;
                })}</div>}
              </div>
              <div className="settings-pane settings-summary-pane" data-settings-pane="archived"><h2>Archived chats</h2>{archivedConversations.length === 0 ? <div className="settings-card"><div><strong>No archived chats</strong><p>Chats you archive will appear here and can be restored at any time.</p></div></div> : <div className="settings-card archived-chat-list">{archivedConversations.map((conversation) => <div className="settings-row" key={conversation.id}><div><strong>{conversation.title}</strong><p>{conversation.project_id ? projects.find((project) => project.project_id === conversation.project_id)?.root ?? "Project chat" : "General chat"}</p></div><button className="secondary" type="button" disabled={chatMutationBusy} onClick={() => void setChatArchived(conversation.id, false)}>Restore</button></div>)}</div>}</div>
            </div>
          </div>
        </section>
      </main>
      {workspaceSidePanelOpen && workspacePanelsVisible && <aside className="workspace-side-panel" aria-label="Workspace side panel">
        <div className="workspace-side-resizer" role="separator" aria-orientation="vertical" aria-label="Resize side panel" aria-valuemin={280} aria-valuemax={window.innerWidth <= 820 ? Math.max(320, window.innerWidth - 48) : Math.max(320, window.innerWidth - (sidebarOpen ? sidebarWidth : 0) - 320)} aria-valuenow={workspaceSideWidth} tabIndex={0} onPointerDown={beginSidePanelResize} onDoubleClick={() => setWorkspaceSideWidth(520)} onKeyDown={(event) => {
          if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
          event.preventDefault();
          const maximum = window.innerWidth <= 820 ? Math.max(320, window.innerWidth - 48) : Math.max(320, window.innerWidth - (sidebarOpen ? sidebarWidth : 0) - 320);
          setWorkspaceSideWidth((current) => Math.min(maximum, Math.max(280, current + (event.key === "ArrowLeft" ? 24 : -24))));
        }} />
        <header>
          <div className="workspace-panel-tabs" role="tablist" aria-label="Side panel tools">
            {([["browser", "Browser", "◎"], ["files", "Files", "▱"], ["activity", "Activity", "◷"]] as Array<[WorkspacePanelTab, string, string]>).map(([tab, label, glyph]) => <button type="button" role="tab" aria-selected={workspaceSidePanelTab === tab} className={workspaceSidePanelTab === tab ? "active" : ""} onClick={() => setWorkspaceSidePanelTab(tab)} key={tab}><span aria-hidden="true">{glyph}</span>{label}</button>)}
          </div>
          <button type="button" aria-label="Close side panel" title="Close side panel" onClick={() => setWorkspaceSidePanelOpen(false)}>×</button>
        </header>
        <div className={`workspace-side-content${workspaceSidePanelTab === "browser" && siteBrowserTabs.length > 0 ? " browser-workspace" : ""}`}>
          {workspaceSidePanelTab === "browser" && <section className={`side-browser${siteBrowserTabs.length > 0 ? " tabbed-browser" : ""}`} aria-label="Browser">
            {siteBrowserTabs.length > 0 ? <>
              <div className="browser-tab-strip" role="tablist" aria-label="Browser tabs">
                {siteBrowserTabs.map((tab) => <div className={`browser-tab${tab.id === activeSiteBrowserTabId ? " active" : ""}`} role="tab" aria-selected={tab.id === activeSiteBrowserTabId} tabIndex={tab.id === activeSiteBrowserTabId ? 0 : -1} onClick={() => selectSiteBrowserTab(tab)} key={tab.id}>
                  <span aria-hidden="true">◎</span><span title={tab.title}>{tab.title}</span><button type="button" aria-label={`Close ${tab.title}`} onClick={(event) => { event.stopPropagation(); closeSiteBrowserTab(tab.id); }}>×</button>
                </div>)}
                <button className="browser-new-tab" type="button" aria-label="New browser tab" title="New tab" onClick={createSiteBrowserTab}>＋</button>
              </div>
              <form className="browser-toolbar" onSubmit={(event) => { event.preventDefault(); navigateSiteBrowserPanel(); }}>
                <button type="button" aria-label="Back" title="Back" disabled={!activeSiteBrowserTab?.url} onClick={() => activeSiteBrowserTab && void controlSiteInAppBrowser(activeSiteBrowserTab.id, "back").catch((error) => setPlatformError(errorMessage(error)))}>←</button>
                <button type="button" aria-label="Forward" title="Forward" disabled={!activeSiteBrowserTab?.url} onClick={() => activeSiteBrowserTab && void controlSiteInAppBrowser(activeSiteBrowserTab.id, "forward").catch((error) => setPlatformError(errorMessage(error)))}>→</button>
                <button type="button" aria-label="Reload" title="Reload" disabled={!activeSiteBrowserTab?.url} onClick={() => activeSiteBrowserTab && void controlSiteInAppBrowser(activeSiteBrowserTab.id, "reload").catch((error) => setPlatformError(errorMessage(error)))}>↻</button>
                <input aria-label="Address" type="url" value={sideBrowserUrl} onChange={(event) => setSideBrowserUrl(event.target.value)} placeholder="Enter a URL" />
                <button type="submit" aria-label="Go" title="Go">↗</button>
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
              : <div className="browser-start-page"><span aria-hidden="true">◎</span><strong>Start browsing</strong><p>Enter a URL to open a page</p></div>
              : <><div className="side-session-list">
              {activeBrowserSessions.length === 0 && <div className="workspace-panel-empty"><span aria-hidden="true">◎</span><strong>No browser sessions</strong><p>Enter a URL to start a private, governed session.</p></div>}
              {activeBrowserSessions.map((session) => {
                const browser = platformBrowsers[session.id];
                const selected = selectedBrowserSession?.id === session.id;
                return <article className={`side-session${selected ? " selected" : ""}`} key={session.id}>
                  <div><span className="session-favicon" aria-hidden="true">◎</span><span><strong>{browser?.title || session.display_name}</strong><small>{browser?.safe_url || session.allowed_origins.join(", ")}</small></span><i className={`session-state state-${session.state}`} title={session.state} /></div>
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
              <button className="side-project-root" type="button" onClick={() => void openCurrentWorkspace("file_explorer")}><span aria-hidden="true">▱</span><span><strong>Project root</strong><small title={workspaceProject.root}>{workspaceProject.root}</small></span><span aria-hidden="true">↗</span></button>
              <div className="side-detail-list"><div><span>Repository</span><strong>{workspaceProject.repository.is_git ? "Git" : "Folder"}</strong></div><div><span>Branch</span><strong>{workspaceProject.repository.branch ?? "—"}</strong></div><div><span>Changes</span><strong>{workspaceProject.repository.dirty ? "Present" : "Clean"}</strong></div></div>
              <button className="secondary" type="button" onClick={() => { setWorkspaceBottomPanelOpen(true); if (workspaceProject.root) setTerminalWorkingDirectory(workspaceProject.root); }}>Open project terminal</button>
            </> : <div className="workspace-panel-empty"><span aria-hidden="true">▱</span><strong>No project selected</strong><p>Choose a project or open one from the left sidebar.</p></div>}
          </section>}
          {workspaceSidePanelTab === "activity" && <section className="side-activity" aria-label="Task activity">
            <div className="side-panel-heading"><div><strong>Task activity</strong><small>Current state and durable evidence.</small></div></div>
            {workspaceTask ? <>
              <article className="side-task-summary"><span className={`task-state state-${workspaceTask.state}`}>{workspaceTask.state.replaceAll("_", " ")}</span><strong>{workspaceTask.goal}</strong><small>{workspaceProjectName}</small></article>
              <button className="secondary" type="button" disabled={taskBusy} onClick={() => void toggleTaskHistory(workspaceTask.id)}>{taskHistory[workspaceTask.id] ? "Refresh evidence" : "Load evidence"}</button>
              {taskHistory[workspaceTask.id] && <div className="side-event-list">{taskHistory[workspaceTask.id]!.events.slice(-12).reverse().map((event) => <div key={event.sequence}><span>{event.kind.replaceAll("_", " ")}</span><time>{new Date(event.occurred_at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</time></div>)}</div>}
            </> : <div className="workspace-panel-empty"><span aria-hidden="true">◷</span><strong>No task selected</strong><p>Open a chat to inspect its live activity.</p></div>}
          </section>}
        </div>
      </aside>}
      {workspaceBottomPanelOpen && workspacePanelsVisible && <section className="workspace-bottom-panel" aria-label="Terminal">
        <div className="workspace-bottom-resizer" role="separator" aria-orientation="horizontal" aria-label="Resize terminal" onMouseDown={beginBottomPanelResize} />
        <header><div className="terminal-tabs"><button className="active" type="button"><span aria-hidden="true">›_</span>Terminal</button><button type="button" aria-label="New terminal" title="Clear terminal" onClick={() => setTerminalEntries([])}>＋</button></div><div><button type="button" disabled={!workspaceProject} title="Use project root" onClick={() => workspaceProject && setTerminalWorkingDirectory(workspaceProject.root)}>⌂</button><button type="button" aria-label="Close terminal" title="Close terminal" onClick={() => setWorkspaceBottomPanelOpen(false)}>×</button></div></header>
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
