import { useEffect, useState } from "react";

import {
  approveRequest,
  applyComputerActions,
  attachBrowserSession,
  attachComputerSession,
  cancelOpenAiOAuth,
  closePlatformSession,
  checkSiteHealth,
  createBackup,
  createSiteMonitor,
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
  handoffBrowserSession,
  handoffComputerSession,
  listMcpConnections,
  listExperiments,
  listSupervisionObservations,
  listProviders,
  providerProxyConversations,
  providerProxyUsage,
  listApprovals,
  listBackups,
  listSchedules,
  listSchedulerNotifications,
  listProjects,
  listPlatformSessions,
  listSites,
  listSshHosts,
  listTasks,
  launchTask,
  loadTaskAudit,
  loadTaskEvents,
  loadExperimentEvidence,
  loadScheduleHistory,
  loadPlatformCapabilities,
  loadPlatformSessionHistory,
  loadSiteDeployments,
  loadSiteHealth,
  loadSshOperations,
  getSiteMonitor,
  getUserSettings,
  loadUserSettingsHistory,
  navigateBrowserSession,
  type McpAuthMode,
  type ExperimentDefinition,
  type ExperimentState,
  type McpListItem,
  type McpProtocolMode,
  type McpTransportKind,
  openExternalUrl,
  type OAuthLoginStart,
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
  type RiskLevel,
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
  retryTask,
  resumeTask,
  requestPlatformSession,
  reclaimBrowserSession,
  reclaimComputerSession,
  registerSite,
  recordCandidateRevision,
  recordExperimentBenchmark,
  recordShadowExecution,
  recordSupervisionObservation,
  registerSshHost,
  runSshOperation,
  startOpenAiOAuth,
  setScheduleState,
  type TaskEnvelope,
  type ApprovalRequest,
  type BackupSummary,
  type TaskEventsResponse,
  type ToolAuditResponse,
  type UserSettings,
  type UserSettingsRevision,
  transitionTask,
  transitionExperiment,
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

function positiveWholeNumber(value: string, label: string): number {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new Error(`${label} must be a positive whole number.`);
  }
  return parsed;
}

const SCHEDULE_EVENT_KINDS = [
  "task_created", "task_started", "task_state_changed", "task_completed", "task_failed",
  "approval_requested", "approval_resolved", "provider_call_started", "provider_call_completed",
  "provider_connection_recorded", "provider_catalog_discovered", "provider_credentials_refreshed",
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

function TaskEvidence({
  history,
  audit,
  advanced,
  state
}: {
  history: TaskEventsResponse;
  audit: ToolAuditResponse;
  advanced: boolean;
  state: TaskEnvelope["state"];
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
  const [advanced, setAdvanced] = useState(false);
  const [userSettings, setUserSettings] = useState<UserSettings | null>(null);
  const [settingsHistory, setSettingsHistory] = useState<UserSettingsRevision[]>([]);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [backups, setBackups] = useState<BackupSummary[]>([]);
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupError, setBackupError] = useState<string | null>(null);
  const [oauthMethod, setOauthMethod] = useState<OpenAiOAuthMethod>("browser");
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
  const [tasks, setTasks] = useState<TaskEnvelope[]>([]);
  const [taskGoal, setTaskGoal] = useState("");
  const [taskConstraints, setTaskConstraints] = useState("");
  const [taskSuccess, setTaskSuccess] = useState("");
  const [taskRisk, setTaskRisk] = useState<RiskLevel>("medium");
  const [taskMaxCost, setTaskMaxCost] = useState("");
  const [taskBusy, setTaskBusy] = useState(false);
  const [taskError, setTaskError] = useState<string | null>(null);
  const [taskLaunch, setTaskLaunch] = useState<Record<string, string>>({});
  const [taskHistory, setTaskHistory] = useState<Record<string, TaskEventsResponse | undefined>>({});
  const [taskAudit, setTaskAudit] = useState<Record<string, ToolAuditResponse | undefined>>({});
  const [selectedProject, setSelectedProject] = useState<ProjectModel | null>(null);
  const [projects, setProjects] = useState<ProjectModel[]>([]);
  const [projectError, setProjectError] = useState<string | null>(null);
  const [projectErrorSurface, setProjectErrorSurface] = useState<"task" | "projects">("projects");
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([]);
  const [schedules, setSchedules] = useState<ScheduleDefinition[]>([]);
  const [scheduleNotifications, setScheduleNotifications] = useState<SchedulerNotification[]>([]);
  const [scheduleHistory, setScheduleHistory] = useState<Record<string, ScheduleHistoryResponse | undefined>>({});
  const [scheduleId, setScheduleId] = useState(freshScheduleId);
  const [scheduleName, setScheduleName] = useState("");
  const [scheduleGoal, setScheduleGoal] = useState("");
  const [scheduleKind, setScheduleKind] = useState<ScheduleTrigger["kind"]>("interval");
  const [scheduleDetail, setScheduleDetail] = useState("3600");
  const [scheduleSecondary, setScheduleSecondary] = useState("30");
  const [scheduleRisk, setScheduleRisk] = useState<RiskLevel>("medium");
  const [scheduleMaxTokens, setScheduleMaxTokens] = useState("32768");
  const [scheduleMaxCost, setScheduleMaxCost] = useState("");
  const [scheduleMaxDuration, setScheduleMaxDuration] = useState("3600");
  const [scheduleModelCalls, setScheduleModelCalls] = useState("8");
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
  const [sshOperationJson, setSshOperationJson] = useState<Record<string, string>>({});
  const [sshOperations, setSshOperations] = useState<Record<string, SshOperationListResponse | undefined>>({});
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
    let active = true;
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
    listProviders()
      .then((response) => {
        if (active) setProviders(response.providers);
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
    listSupervisionObservations()
      .then((response) => { if (active) setObservations(response.observations); })
      .catch((error) => { if (active) setExperimentError(errorMessage(error)); });
    return () => {
      active = false;
    };
  }, []);

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
    const nextAdvanced = !advanced;
    if (!nextAdvanced && scheduleKind !== "interval") {
      setScheduleKind("interval");
      setScheduleDetail("3600");
      setScheduleSecondary("30");
    }
    await saveUserSettings(
      { ...userSettings, advanced_mode: nextAdvanced },
      nextAdvanced ? "Enabled advanced interface" : "Enabled simple interface"
    );
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

  async function startLogin() {
    setBusy(true);
    setProviderError(null);
    try {
      const started = await startOpenAiOAuth(oauthMethod);
      setLogin(started);
      setProviderStatus("Waiting for authorization");
      try {
        await openExternalUrl(started.verification_url);
      } catch (error) {
        setProviderError(`${errorMessage(error)}. Use “Open verification page” below.`);
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
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
  }

  function taskEvidence(task: TaskEnvelope) {
    const history = taskHistory[task.id];
    const audit = taskAudit[task.id];
    return history && audit
      ? <TaskEvidence history={history} audit={audit} advanced={advanced} state={task.state} />
      : null;
  }

  async function chooseProject(surface: "task" | "projects") {
    setTaskBusy(true);
    setProjectError(null);
    setProjectErrorSurface(surface);
    try {
      const root = await pickProjectDirectory();
      if (!root) return;
      const mutation = await discoverProject(root);
      setSelectedProject(mutation.project);
      setProjects((current) => [
        mutation.project,
        ...current.filter((project) => project.project_id !== mutation.project.project_id)
      ]);
    } catch (error) {
      setProjectError(errorMessage(error));
    } finally {
      setTaskBusy(false);
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
        estimated_output_tokens: 4096,
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
          ? platformOrigins.split(/\r?\n/).map((value) => value.trim()).filter(Boolean)
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
        known_hosts_entry: sshKnownHosts.trim(), host_key_sha256: sshFingerprint.trim(), state: "active"
      });
      setSshHosts((current) => [host, ...current.filter((item) => item.id !== host.id)]);
      setSshHostId(crypto.randomUUID()); setSshName(""); setSshHostname(""); setSshUsername("");
      setSshKnownHosts(""); setSshFingerprint("");
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

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand" aria-label="MetaCognitive Governor">
          <span className="brand-mark">M</span>
          <span>MCG</span>
        </div>
        <nav>
          <a className="nav-item active" href="#tasks">Tasks</a>
          <a className="nav-item" href="#projects">Projects</a>
          <a className="nav-item" href="#threads">Threads</a>
          <a className="nav-item" href="#scheduler">Scheduler</a>
          <a className="nav-item" href="#improvement">Improvement</a>
          <a className="nav-item" href="#sites">Sites</a>
          <a className="nav-item" href="#ssh">SSH</a>
          <a className="nav-item" href="#platform">Computer & browser</a>
          <a className="nav-item" href="#providers">Providers</a>
          <a className="nav-item" href="#mcp">Tools</a>
          <a className="nav-item" href="#settings">Settings</a>
        </nav>
        <button
          className="mode-button"
          type="button"
          aria-pressed={advanced}
          disabled={!userSettings || settingsBusy}
          onClick={toggleAdvancedMode}
        >
          {advanced ? "Simple mode" : "Advanced mode"}
        </button>
      </aside>

      <main className="workspace" id="tasks">
        <header className="topbar">
          <div>
            <p className="eyebrow">METACOGNITIVE GOVERNOR</p>
            <h1>Your AI workspace</h1>
          </div>
          <span className="status-pill"><i /> Local runtime</span>
        </header>

        <section className="hero-card">
          <div>
            <p className="eyebrow">NEW TASK</p>
            <h2>What would you like MCG to do?</h2>
            <p className="muted">
              Open a folder or describe a goal. MCG will choose the smallest useful
              organization, work within your permissions, and verify the result.
            </p>
          </div>
          <div className="composer">
            <label htmlFor="goal">Goal</label>
            <textarea
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
            {selectedProject && <div className="selected-project" role="status">
              <div>
                <strong>{selectedProject.root.split(/[\\/]/).filter(Boolean).at(-1) ?? selectedProject.root}</strong>
                <span>{selectedProject.root}</span>
              </div>
              <button className="link-button" type="button" onClick={() => setSelectedProject(null)}>Remove</button>
            </div>}
            <div className="composer-actions">
              {projects.length > 0 && <label className="project-select">
                Project
                <select
                  value={selectedProject?.project_id ?? ""}
                  onChange={(event) => setSelectedProject(
                    projects.find((project) => project.project_id === event.target.value) ?? null
                  )}
                >
                  <option value="">None</option>
                  {projects.map((project) => <option value={project.project_id} key={project.project_id}>{project.root.split(/[\\/]/).filter(Boolean).at(-1) ?? project.root}</option>)}
                </select>
              </label>}
              <button className="secondary" type="button" disabled={taskBusy} onClick={() => chooseProject("task")}>Open folder</button>
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
            <span className="count-badge">{tasks.length} total</span>
          </div>
          <div className="task-list">
            {tasks.length === 0 ? <p className="empty-state">No tasks yet.</p> : tasks.map((task) => (
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
                  <button className="secondary" type="button" disabled={taskBusy} onClick={() => toggleTaskHistory(task.id)}>{taskHistory[task.id] ? "Hide evidence" : "Evidence"}</button>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section className="grid" id="projects">
          <article className="panel project-panel">
            <div className="section-heading">
              <div>
                <p className="eyebrow">PROJECTS</p>
                <h2>Your workspaces</h2>
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
                    <p>{project.languages.length > 0 ? project.languages.join(", ") : "Language not detected"}</p>
                    <div className="project-tags">
                      {project.build_systems.map((system) => <span key={`build-${system}`}>{system}</span>)}
                      {project.test_systems.map((system) => <span key={`test-${system}`}>{system}</span>)}
                      {project.repository.is_git && <span>{project.repository.dirty ? "Git · changes" : "Git · clean"}</span>}
                    </div>
                    {advanced && <><code>{project.root}</code><small>{project.repository.branch ? `Branch ${project.repository.branch} · ` : ""}{project.instructions.conflicts.length} instruction conflicts</small></>}
                  </div>
                  <button className={selected ? "project-selected" : "secondary"} type="button" disabled={selected} onClick={() => setSelectedProject(project)}>{selected ? "Selected" : "Use project"}</button>
                </article>;
              })}
            </div>}
            {projectError && projectErrorSurface === "projects" && <p className="error-message" role="alert">{projectError}</p>}
          </article>

          <article className="panel runtime-panel">
            {advanced ? (
              <>
                <p className="eyebrow">ADVANCED</p>
                <h2>Runtime status</h2>
                <dl>
                  <div><dt>Daemon API</dt><dd>Authenticated local loopback</dd></div>
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
          <div className="scheduler-layout">
            <div className="schedule-composer">
              <label>Organization ID<input value={experimentOrganizationId} onChange={(event) => setExperimentOrganizationId(event.target.value)} placeholder="Organization UUID" /></label>
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
                return <article className="schedule-card" key={experiment.id}>
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
                    {advanced && <><code>{host.id}</code><code>revision {host.revision} · {host.host_key_sha256}</code></>}
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
                Allowed sites {advanced ? "(one origin per line)" : "(required before starting)"}
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
          <div className="section-heading">
            <div>
              <p className="eyebrow">SCHEDULER</p>
              <h2>Unattended work</h2>
              <p className="muted">Schedules survive restarts and pause safely for permission or budget.</p>
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
              {!advanced ? <label>
                Repeat
                <select value={scheduleDetail} onChange={(event) => {
                  setScheduleKind("interval");
                  setScheduleDetail(event.target.value);
                }}>
                  <option value="900">Every 15 minutes</option>
                  <option value="3600">Every hour</option>
                  <option value="86400">Every day</option>
                </select>
              </label> : <div className="scheduler-advanced">
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
              </div>}
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
                    {advanced && <><code>{schedule.id}</code><small>{schedule.goal}</small></>}
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
                    {advanced && schedule.state !== "completed" && <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => requestScheduleWake(schedule.id)}>Wake now</button>}
                    <button className="secondary" type="button" disabled={scheduleBusy} onClick={() => toggleScheduleHistory(schedule.id)}>{scheduleHistory[schedule.id] ? "Hide history" : "History"}</button>
                    {advanced && schedule.state !== "completed" && <button className="link-button" type="button" disabled={scheduleBusy} onClick={() => changeScheduleState(schedule.id, "completed")}>Remove</button>}
                  </div>
                </article>)}
              </div>
            </div>
          </div>
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
            {advanced && !login && (
              <label>
                Login method
                <select
                  value={oauthMethod}
                  onChange={(event) => setOauthMethod(event.target.value as OpenAiOAuthMethod)}
                >
                  <option value="browser">Browser</option>
                  <option value="device_code">One-time device code</option>
                </select>
              </label>
            )}
            {!login ? (
              <button className="primary" type="button" disabled={busy} onClick={startLogin}>
                {busy ? "Connecting…" : "Connect with OpenAI"}
              </button>
            ) : (
              <div className="oauth-card" role="status">
                <p>Authorize MCG in the page that opened.</p>
                <button className="link-button" type="button" onClick={() => openVerification(login.verification_url)}>
                  Open verification page
                </button>
                {login.user_code && <p className="device-code">Code: <strong>{login.user_code}</strong></p>}
                <div className="composer-actions">
                  <button className="secondary" type="button" disabled={busy} onClick={cancelLogin}>Cancel</button>
                  <button className="primary" type="button" disabled={busy} onClick={finishLogin}>
                    {busy ? "Checking…" : "I’ve authorized MCG"}
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
                <button className="secondary" type="button" disabled={busy} onClick={() => refreshProvider(item.connection.id)}>Refresh models</button>
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

        <section className="panel" id="settings" aria-labelledby="settings-heading">
          <div className="section-heading">
            <div>
              <p className="eyebrow">OWNER CONTROLS</p>
              <h2 id="settings-heading">Settings</h2>
              <p className="muted">Choose how independently MCG may act. Changes are durable and apply to new authority decisions.</p>
            </div>
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
          <div className="provider-connections" aria-label="Portable backups">
            <div className="section-heading compact-heading">
              <div>
                <h3>Portable backups</h3>
                <p className="muted">Preserve the database and immutable artifacts. Credentials and the local authentication token are always excluded.</p>
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
                  <p>{formatBytes(backup.size_bytes)} · {new Date(backup.manifest.created_at).toLocaleString()} · secrets excluded</p>
                  {advanced && <><code>{backup.sha256}</code><small>{backup.manifest.entries.length} verified entries · format {backup.manifest.schema_version}</small></>}
                </div>
                <button className="secondary" type="button" disabled={backupBusy} onClick={() => checkBackup(backup.backup_id)}>Verify</button>
              </article>
            ))}
            <p className="form-note">Restore is intentionally offline: stop the daemon, then run <code>mcg backup restore PATH --confirm</code>. MCG retains the previous database for rollback.</p>
          </div>
        </section>
      </main>
    </div>
  );
}
