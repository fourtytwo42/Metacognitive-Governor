import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const client = vi.hoisted(() => ({
  start: vi.fn(),
  complete: vi.fn(),
  cancel: vi.fn(),
  open: vi.fn(),
  listProviders: vi.fn(),
  proxyUsage: vi.fn(),
  proxyConversations: vi.fn(),
  configureProvider: vi.fn(),
  discoverProvider: vi.fn(),
  disconnectProvider: vi.fn(),
  listMcp: vi.fn(),
  listPlugins: vi.fn(),
  installLocalPlugin: vi.fn(),
  installRegistryPlugin: vi.fn(),
  setPluginGrants: vi.fn(),
  setPluginEnabled: vi.fn(),
  invokePlugin: vi.fn(),
  getPluginInvocation: vi.fn(),
  configureMcp: vi.fn(),
  discoverMcp: vi.fn(),
  listTasks: vi.fn(),
  createTask: vi.fn(),
  launchTask: vi.fn(),
  retryTask: vi.fn(),
  resumeTask: vi.fn(),
  listApprovals: vi.fn(),
  approveRequest: vi.fn(),
  denyRequest: vi.fn(),
  transitionTask: vi.fn(),
  taskEvents: vi.fn(),
  taskAudit: vi.fn(),
  replayRun: vi.fn(),
  pickProject: vi.fn(),
  discoverProject: vi.fn(),
  listProjects: vi.fn(),
  projectTopologyPlan: vi.fn(),
  listReviewedTopologyPlans: vi.fn(),
  createReviewedTopologyPlan: vi.fn(),
  runReviewedTopologyPlan: vi.fn(),
  cancelReviewedTopologyPlan: vi.fn(),
  listProjectMemory: vi.fn(),
  createProjectMemory: vi.fn(),
  correctProjectMemory: vi.fn(),
  transitionProjectMemory: vi.fn(),
  reviewProjectMemoryTransfer: vi.fn(),
  transferProjectMemory: vi.fn(),
  listProjectMemoryTransfers: vi.fn(),
  loadProjectMemoryHistory: vi.fn(),
  loadManagedProjectMemory: vi.fn(),
  listOrganizations: vi.fn(),
  loadOrganizationHistory: vi.fn(),
  createOrganization: vi.fn(),
  inheritOrganization: vi.fn(),
  exportOrganization: vi.fn(),
  importOrganization: vi.fn(),
  compareOrganizations: vi.fn(),
  listOrganizationPublishers: vi.fn(),
  trustOrganizationPublisher: vi.fn(),
  revokeOrganizationPublisher: vi.fn(),
  listOrganizationShareReviews: vi.fn(),
  reviewSharedOrganization: vi.fn(),
  importSharedOrganization: vi.fn(),
  rejectSharedOrganization: vi.fn(),
  listSchedules: vi.fn(),
  createSchedule: vi.fn(),
  setScheduleState: vi.fn(),
  scheduleHistory: vi.fn(),
  wakeSchedule: vi.fn(),
  listSchedulerNotifications: vi.fn(),
  acknowledgeSchedulerNotification: vi.fn(),
  loadPlatformCapabilities: vi.fn(),
  listPlatformSessions: vi.fn(),
  requestPlatformSession: vi.fn(),
  loadPlatformSessionHistory: vi.fn(),
  closePlatformSession: vi.fn(),
  attachBrowserSession: vi.fn(),
  navigateBrowserSession: vi.fn(),
  handoffBrowserSession: vi.fn(),
  reclaimBrowserSession: vi.fn(),
  attachComputerSession: vi.fn(),
  captureComputerSession: vi.fn(),
  applyComputerActions: vi.fn(),
  handoffComputerSession: vi.fn(),
  reclaimComputerSession: vi.fn(),
  listSites: vi.fn(),
  registerSite: vi.fn(),
  loadSiteHealth: vi.fn(),
  checkSiteHealth: vi.fn(),
  loadSiteDeployments: vi.fn(),
  deploySite: vi.fn(),
  updateSite: vi.fn(),
  getSiteMonitor: vi.fn(),
  createSiteMonitor: vi.fn()
  ,listSshHosts: vi.fn()
  ,registerSshHost: vi.fn()
  ,loadSshOperations: vi.fn()
  ,runSshOperation: vi.fn()
  ,listSshWorkflows: vi.fn()
  ,createSshWorkflow: vi.fn()
  ,runSshWorkflow: vi.fn()
  ,cancelSshWorkflow: vi.fn()
  ,listSupervisionObservations: vi.fn()
  ,recordSupervisionObservation: vi.fn()
  ,recordCandidateRevision: vi.fn()
  ,listExperiments: vi.fn()
  ,loadExperimentEvidence: vi.fn()
  ,createExperiment: vi.fn()
  ,transitionExperiment: vi.fn()
  ,recordShadowExecution: vi.fn()
  ,recordExperimentBenchmark: vi.fn()
  ,decideExperiment: vi.fn()
  ,getSettings: vi.fn()
  ,updateSettings: vi.fn()
  ,settingsHistory: vi.fn()
  ,listBackups: vi.fn()
  ,createBackup: vi.fn()
  ,createEncryptedBackup: vi.fn()
  ,verifyBackup: vi.fn()
  ,runDiagnostics: vi.fn()
  ,getUpdateConfiguration: vi.fn()
  ,checkForUpdate: vi.fn()
  ,installUpdate: vi.fn()
  ,localServiceStatus: vi.fn()
}));

vi.mock("./desktop-client", () => ({
  startOpenAiOAuth: client.start,
  completeOpenAiOAuth: client.complete,
  cancelOpenAiOAuth: client.cancel,
  openExternalUrl: client.open,
  listProviders: client.listProviders,
  providerProxyUsage: client.proxyUsage,
  providerProxyConversations: client.proxyConversations,
  configureProvider: client.configureProvider,
  discoverProvider: client.discoverProvider,
  disconnectProvider: client.disconnectProvider,
  listMcpConnections: client.listMcp,
  listPlugins: client.listPlugins,
  installLocalPlugin: client.installLocalPlugin,
  installRegistryPlugin: client.installRegistryPlugin,
  setPluginGrants: client.setPluginGrants,
  setPluginEnabled: client.setPluginEnabled,
  invokePlugin: client.invokePlugin,
  getPluginInvocation: client.getPluginInvocation,
  configureMcpConnection: client.configureMcp,
  discoverMcpConnection: client.discoverMcp,
  listTasks: client.listTasks,
  createTask: client.createTask,
  launchTask: client.launchTask,
  retryTask: client.retryTask,
  resumeTask: client.resumeTask,
  listApprovals: client.listApprovals,
  approveRequest: client.approveRequest,
  denyRequest: client.denyRequest,
  transitionTask: client.transitionTask,
  loadTaskEvents: client.taskEvents,
  loadTaskAudit: client.taskAudit,
  replayAgentRun: client.replayRun,
  pickProjectDirectory: client.pickProject,
  discoverProject: client.discoverProject,
  listProjects: client.listProjects,
  projectTopologyPlan: client.projectTopologyPlan,
  listReviewedTopologyPlans: client.listReviewedTopologyPlans,
  createReviewedTopologyPlan: client.createReviewedTopologyPlan,
  runReviewedTopologyPlan: client.runReviewedTopologyPlan,
  cancelReviewedTopologyPlan: client.cancelReviewedTopologyPlan,
  listProjectMemory: client.listProjectMemory,
  createProjectMemory: client.createProjectMemory,
  correctProjectMemory: client.correctProjectMemory,
  transitionProjectMemory: client.transitionProjectMemory,
  reviewProjectMemoryTransfer: client.reviewProjectMemoryTransfer,
  transferProjectMemory: client.transferProjectMemory,
  listProjectMemoryTransfers: client.listProjectMemoryTransfers,
  loadProjectMemoryHistory: client.loadProjectMemoryHistory,
  loadManagedProjectMemory: client.loadManagedProjectMemory,
  listOrganizations: client.listOrganizations,
  loadOrganizationHistory: client.loadOrganizationHistory,
  createOrganization: client.createOrganization,
  inheritOrganization: client.inheritOrganization,
  exportOrganization: client.exportOrganization,
  importOrganization: client.importOrganization,
  compareOrganizations: client.compareOrganizations,
  listOrganizationPublishers: client.listOrganizationPublishers,
  trustOrganizationPublisher: client.trustOrganizationPublisher,
  revokeOrganizationPublisher: client.revokeOrganizationPublisher,
  listOrganizationShareReviews: client.listOrganizationShareReviews,
  reviewSharedOrganization: client.reviewSharedOrganization,
  importSharedOrganization: client.importSharedOrganization,
  rejectSharedOrganization: client.rejectSharedOrganization,
  listSchedules: client.listSchedules,
  createSchedule: client.createSchedule,
  setScheduleState: client.setScheduleState,
  loadScheduleHistory: client.scheduleHistory,
  wakeSchedule: client.wakeSchedule,
  listSchedulerNotifications: client.listSchedulerNotifications,
  acknowledgeSchedulerNotification: client.acknowledgeSchedulerNotification,
  loadPlatformCapabilities: client.loadPlatformCapabilities,
  listPlatformSessions: client.listPlatformSessions,
  requestPlatformSession: client.requestPlatformSession,
  loadPlatformSessionHistory: client.loadPlatformSessionHistory,
  closePlatformSession: client.closePlatformSession,
  attachBrowserSession: client.attachBrowserSession,
  navigateBrowserSession: client.navigateBrowserSession,
  handoffBrowserSession: client.handoffBrowserSession,
  reclaimBrowserSession: client.reclaimBrowserSession,
  attachComputerSession: client.attachComputerSession,
  captureComputerSession: client.captureComputerSession,
  applyComputerActions: client.applyComputerActions,
  handoffComputerSession: client.handoffComputerSession,
  reclaimComputerSession: client.reclaimComputerSession,
  listSites: client.listSites,
  registerSite: client.registerSite,
  loadSiteHealth: client.loadSiteHealth,
  checkSiteHealth: client.checkSiteHealth,
  loadSiteDeployments: client.loadSiteDeployments,
  deploySite: client.deploySite,
  updateSite: client.updateSite,
  getSiteMonitor: client.getSiteMonitor,
  createSiteMonitor: client.createSiteMonitor,
  listSshHosts: client.listSshHosts,
  registerSshHost: client.registerSshHost,
  loadSshOperations: client.loadSshOperations,
  runSshOperation: client.runSshOperation,
  listSshWorkflows: client.listSshWorkflows,
  createSshWorkflow: client.createSshWorkflow,
  runSshWorkflow: client.runSshWorkflow,
  cancelSshWorkflow: client.cancelSshWorkflow,
  listSupervisionObservations: client.listSupervisionObservations,
  recordSupervisionObservation: client.recordSupervisionObservation,
  recordCandidateRevision: client.recordCandidateRevision,
  listExperiments: client.listExperiments,
  loadExperimentEvidence: client.loadExperimentEvidence,
  createExperiment: client.createExperiment,
  transitionExperiment: client.transitionExperiment,
  recordShadowExecution: client.recordShadowExecution,
  recordExperimentBenchmark: client.recordExperimentBenchmark,
  decideExperiment: client.decideExperiment,
  getUserSettings: client.getSettings,
  updateUserSettings: client.updateSettings,
  loadUserSettingsHistory: client.settingsHistory,
  listBackups: client.listBackups,
  createBackup: client.createBackup,
  createEncryptedBackup: client.createEncryptedBackup,
  verifyBackup: client.verifyBackup,
  runDiagnostics: client.runDiagnostics,
  getUpdateConfiguration: client.getUpdateConfiguration,
  checkForUpdate: client.checkForUpdate,
  installUpdate: client.installUpdate,
  getLocalServiceStatus: client.localServiceStatus
}));

import { App } from "./App";

beforeEach(() => {
  client.localServiceStatus.mockReset();
  client.start.mockReset();
  client.complete.mockReset();
  client.cancel.mockReset();
  client.open.mockReset();
  client.listProviders.mockReset();
  client.proxyUsage.mockReset();
  client.proxyConversations.mockReset();
  client.configureProvider.mockReset();
  client.discoverProvider.mockReset();
  client.disconnectProvider.mockReset();
  client.listMcp.mockReset();
  client.listPlugins.mockReset();
  client.installLocalPlugin.mockReset();
  client.installRegistryPlugin.mockReset();
  client.setPluginGrants.mockReset();
  client.setPluginEnabled.mockReset();
  client.invokePlugin.mockReset();
  client.getPluginInvocation.mockReset();
  client.configureMcp.mockReset();
  client.discoverMcp.mockReset();
  client.listTasks.mockReset();
  client.createTask.mockReset();
  client.launchTask.mockReset();
  client.retryTask.mockReset();
  client.resumeTask.mockReset();
  client.listApprovals.mockReset();
  client.approveRequest.mockReset();
  client.denyRequest.mockReset();
  client.transitionTask.mockReset();
  client.taskEvents.mockReset();
  client.taskAudit.mockReset();
  client.replayRun.mockReset();
  client.pickProject.mockReset();
  client.discoverProject.mockReset();
  client.listProjects.mockReset();
  client.projectTopologyPlan.mockReset();
  client.listReviewedTopologyPlans.mockReset();
  client.createReviewedTopologyPlan.mockReset();
  client.runReviewedTopologyPlan.mockReset();
  client.cancelReviewedTopologyPlan.mockReset();
  client.listProjectMemory.mockReset();
  client.createProjectMemory.mockReset();
  client.correctProjectMemory.mockReset();
  client.transitionProjectMemory.mockReset();
  client.reviewProjectMemoryTransfer.mockReset();
  client.transferProjectMemory.mockReset();
  client.listProjectMemoryTransfers.mockReset();
  client.loadProjectMemoryHistory.mockReset();
  client.loadManagedProjectMemory.mockReset();
  client.listOrganizations.mockReset();
  client.loadOrganizationHistory.mockReset();
  client.createOrganization.mockReset();
  client.inheritOrganization.mockReset();
  client.exportOrganization.mockReset();
  client.importOrganization.mockReset();
  client.compareOrganizations.mockReset();
  client.listOrganizationPublishers.mockReset();
  client.trustOrganizationPublisher.mockReset();
  client.revokeOrganizationPublisher.mockReset();
  client.listOrganizationShareReviews.mockReset();
  client.reviewSharedOrganization.mockReset();
  client.importSharedOrganization.mockReset();
  client.rejectSharedOrganization.mockReset();
  client.listSchedules.mockReset();
  client.createSchedule.mockReset();
  client.setScheduleState.mockReset();
  client.scheduleHistory.mockReset();
  client.wakeSchedule.mockReset();
  client.listSchedulerNotifications.mockReset();
  client.acknowledgeSchedulerNotification.mockReset();
  client.loadPlatformCapabilities.mockReset();
  client.listPlatformSessions.mockReset();
  client.requestPlatformSession.mockReset();
  client.loadPlatformSessionHistory.mockReset();
  client.closePlatformSession.mockReset();
  client.attachBrowserSession.mockReset();
  client.navigateBrowserSession.mockReset();
  client.handoffBrowserSession.mockReset();
  client.reclaimBrowserSession.mockReset();
  client.attachComputerSession.mockReset();
  client.captureComputerSession.mockReset();
  client.applyComputerActions.mockReset();
  client.handoffComputerSession.mockReset();
  client.reclaimComputerSession.mockReset();
  client.listSites.mockReset();
  client.registerSite.mockReset();
  client.loadSiteHealth.mockReset();
  client.checkSiteHealth.mockReset();
  client.loadSiteDeployments.mockReset();
  client.deploySite.mockReset();
  client.updateSite.mockReset();
  client.getSiteMonitor.mockReset();
  client.createSiteMonitor.mockReset();
  client.listSshHosts.mockReset();
  client.registerSshHost.mockReset();
  client.loadSshOperations.mockReset();
  client.runSshOperation.mockReset();
  client.listSshWorkflows.mockReset();
  client.createSshWorkflow.mockReset();
  client.runSshWorkflow.mockReset();
  client.cancelSshWorkflow.mockReset();
  client.listSupervisionObservations.mockReset();
  client.recordSupervisionObservation.mockReset();
  client.recordCandidateRevision.mockReset();
  client.listExperiments.mockReset();
  client.loadExperimentEvidence.mockReset();
  client.createExperiment.mockReset();
  client.transitionExperiment.mockReset();
  client.recordShadowExecution.mockReset();
  client.recordExperimentBenchmark.mockReset();
  client.decideExperiment.mockReset();
  client.getSettings.mockReset();
  client.updateSettings.mockReset();
  client.settingsHistory.mockReset();
  client.listBackups.mockReset();
  client.createBackup.mockReset();
  client.createEncryptedBackup.mockReset();
  client.verifyBackup.mockReset();
  client.runDiagnostics.mockReset();
  client.getUpdateConfiguration.mockReset();
  client.checkForUpdate.mockReset();
  client.installUpdate.mockReset();
  client.open.mockResolvedValue(undefined);
  client.listProviders.mockResolvedValue({ providers: [] });
  client.proxyUsage.mockResolvedValue({
    conversation_count: 0,
    call_count: 0,
    usage: { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0, reasoning_tokens: 0, total_tokens: 0 },
    known_actual_cost_usd: 0,
    unknown_cost_call_count: 0
  });
  client.proxyConversations.mockResolvedValue({ conversations: [] });
  client.listMcp.mockResolvedValue({ connections: [] });
  client.listPlugins.mockResolvedValue({ plugins: [] });
  client.listTasks.mockResolvedValue({ tasks: [] });
  client.listProjects.mockResolvedValue({ projects: [] });
  client.projectTopologyPlan.mockResolvedValue({
    schema_version: 2,
    sha256: "0".repeat(64),
    project_id: "00000000-0000-4000-8000-000000000000",
    project_snapshot_id: "00000000-0000-4000-8000-000000000000",
    status: "empty",
    stages: [],
    unresolved_nodes: []
  });
  client.listReviewedTopologyPlans.mockResolvedValue({ plans: [] });
  client.listProjectMemory.mockResolvedValue({ claims: [] });
  client.listProjectMemoryTransfers.mockResolvedValue({ transfers: [] });
  client.loadManagedProjectMemory.mockResolvedValue({ markdown: "# Managed Project Memory\n" });
  client.listOrganizations.mockResolvedValue({ organizations: [] });
  client.listOrganizationPublishers.mockResolvedValue([]);
  client.listOrganizationShareReviews.mockResolvedValue([]);
  client.listApprovals.mockResolvedValue({ approvals: [] });
  client.listSchedules.mockResolvedValue({ schedules: [] });
  client.listSchedulerNotifications.mockResolvedValue({ notifications: [] });
  client.loadPlatformCapabilities.mockResolvedValue({
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000070",
    os: "windows",
    architecture: "x86_64",
    capabilities: [
      { kind: "browser_automation", availability: "unavailable", detail: "no local browser runtime configuration has been verified" },
      { kind: "computer_use", availability: "unavailable", detail: "product computer-use executor is not attached" },
      { kind: "native_desktop", availability: "available", detail: "native desktop target is supported" }
    ],
    observed_at: "2026-08-14T00:00:00Z"
  });
  client.listPlatformSessions.mockResolvedValue({ sessions: [] });
  client.listSites.mockResolvedValue({ sites: [] });
  client.listSshHosts.mockResolvedValue({ hosts: [] });
  client.listSshWorkflows.mockResolvedValue({ workflows: [] });
  client.listSupervisionObservations.mockResolvedValue({ observations: [] });
  client.listExperiments.mockResolvedValue({ experiments: [] });
  client.loadExperimentEvidence.mockResolvedValue({
    shadow: { receipts: [] },
    benchmarks: { benchmarks: [] },
    decisions: { decisions: [] }
  });
  client.getSiteMonitor.mockRejectedValue(new Error("Site has no monitor"));
  const settings = {
    schema_version: 1,
    revision: 1,
    advanced_mode: false,
    autonomy: "autonomous" as const,
    shadow_experiments: true,
    updated_at: "2026-08-15T00:00:00Z"
  };
  client.getSettings.mockResolvedValue(settings);
  client.settingsHistory.mockResolvedValue({
    revisions: [{ settings, reason: "initial settings", changed_at: settings.updated_at }]
  });
  client.listBackups.mockResolvedValue({ backups: [] });
  client.runDiagnostics.mockResolvedValue({
    schema_version: 1,
    generated_at: "2026-08-15T00:00:00Z",
    service: "mcg-daemon",
    version: "0.1.0",
    os: "windows",
    architecture: "x86_64",
    overall: "warning",
    checks: [
      { code: "database_integrity", label: "Database integrity", severity: "pass", detail: "SQLite quick-check completed successfully." },
      { code: "provider_dispatch_recovery", label: "Provider dispatch recovery", severity: "warning", detail: "1 physical provider request(s) are active or indeterminate; 4 validated request(s) completed; 2 definite request(s) failed.", remediation: "Review the affected task and provider billing/logs, then retry with a new owner-authorized call identity; MCG will not resend an indeterminate request automatically." },
      { code: "portable_backups", label: "Portable backups", severity: "warning", detail: "No portable backup has been created yet.", remediation: "Create and retain a portable backup before relying on the installation." }
    ],
    counts: { events: 4, tasks: 1, artifacts: 0, backups: 0, providers: 0, models: 0, mcp_connections: 0, mcp_tools: 0, schedules: 0, pending_approvals: 0, provider_dispatch_started: 1, provider_dispatch_completed: 4, provider_dispatch_failed: 2 }
  });
  client.getUpdateConfiguration.mockResolvedValue({
    schema_version: 1,
    configured: false,
    current_version: "0.1.0",
    detail: "This build has no trusted update endpoint.",
    trusted_origin: null,
    recovery_notice: null
  });
  client.installUpdate.mockResolvedValue(undefined);
  client.localServiceStatus.mockResolvedValue({
    ready: true,
    launched: true,
    service: "mcg-daemon",
    version: "0.1.0",
    detail: "The packaged local service started successfully."
  });
  client.updateSettings.mockImplementation(async (request) => ({
    schema_version: 1,
    revision: request.expected_revision + 1,
    advanced_mode: request.advanced_mode,
    autonomy: request.autonomy,
    shadow_experiments: request.shadow_experiments,
    updated_at: "2026-08-15T00:01:00Z"
  }));
});

async function enableAdvancedMode() {
  const button = screen.getByRole("button", { name: "Advanced mode" });
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
  await screen.findByRole("button", { name: "Simple mode" });
}

async function enableSimpleMode() {
  const button = screen.getByRole("button", { name: "Simple mode" });
  fireEvent.click(button);
  await screen.findByRole("button", { name: "Advanced mode" });
}

test("packaged local service readiness is visible and failures are not disguised", async () => {
  client.localServiceStatus.mockResolvedValue({
    ready: false,
    launched: false,
    service: "mcg-daemon",
    version: "0.1.0",
    detail: "The packaged service is missing. Repair or reinstall MCG."
  });
  render(<App />);
  const status = (await screen.findByText("Local runtime needs attention")).closest("[role='status']");
  expect(status).toHaveAttribute("title", "The packaged service is missing. Repair or reinstall MCG.");
});

test("owner settings persist autonomy with optimistic revision control", async () => {
  render(<App />);
  const panel = await screen.findByLabelText("Owner settings");
  expect(within(panel).getByLabelText("Autonomy")).toHaveValue("autonomous");
  fireEvent.change(within(panel).getByLabelText("Autonomy"), { target: { value: "guided" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Save settings" }));
  await waitFor(() => expect(client.updateSettings).toHaveBeenCalledWith({
    expected_revision: 1,
    advanced_mode: false,
    autonomy: "guided",
    shadow_experiments: true,
    reason: "Updated owner settings from desktop"
  }));
  expect(await screen.findByText("Revision 2")).toBeInTheDocument();
  expect(within(panel).getByText("Ask before consequential local actions.")).toBeInTheDocument();
});

test("unsigned development builds fail closed instead of offering an update fallback", async () => {
  render(<App />);
  const panel = await screen.findByLabelText("Application updates");
  expect(within(panel).getByText(/Updates are unavailable in this build/)).toBeInTheDocument();
  expect(within(panel).getByText(/never accept an unsigned fallback/)).toBeInTheDocument();
  expect(within(panel).queryByRole("button", { name: "Check for updates" })).not.toBeInTheDocument();
});

test("owner-signed updates require review and invoke verified backup installation", async () => {
  client.getUpdateConfiguration.mockResolvedValue({
    schema_version: 1,
    configured: true,
    current_version: "0.1.0",
    trusted_origin: "https://updates.example.test",
    detail: "MCG will accept only packages signed by this build's trusted update key.",
    recovery_notice: "The update did not complete. MCG restarted the previous installation and preserved its data."
  });
  client.checkForUpdate.mockResolvedValue({
    schema_version: 1,
    configured: true,
    current_version: "0.1.0",
    available_version: "0.2.0",
    notes: "Security and recovery improvements.",
    published_at: "2026-08-18T12:00:00Z",
    target: "windows-x86_64-nsis",
    detail: "A newer owner-signed MCG release is available."
  });
  render(<App />);
  const panel = await screen.findByLabelText("Application updates");
  expect(within(panel).getByRole("status")).toHaveTextContent(/restarted the previous installation/);
  fireEvent.click(within(panel).getByRole("button", { name: "Check for updates" }));
  expect(await within(panel).findByText("MCG 0.2.0")).toBeInTheDocument();
  expect(within(panel).getByText("Security and recovery improvements.")).toBeInTheDocument();
  expect(client.installUpdate).not.toHaveBeenCalled();

  fireEvent.click(within(panel).getByRole("button", { name: "Review update" }));
  const confirmation = within(panel).getByLabelText("Confirm application update");
  expect(within(confirmation).getByText(/create and verify a credential-free portable backup/)).toBeInTheDocument();
  fireEvent.click(within(confirmation).getByRole("button", { name: "Back up and install" }));
  await waitFor(() => expect(client.installUpdate).toHaveBeenCalledWith("0.2.0", expect.any(String)));
});

test("portable backup creation is explicit secret-free and verifiable", async () => {
  const backup = {
    backup_id: "00000000-0000-4000-8000-000000000401",
    file_name: "00000000-0000-4000-8000-000000000401.mcgb",
    sha256: "a".repeat(64),
    size_bytes: 4096,
    manifest: {
      schema_version: 1,
      application_version: "0.1.0",
      created_at: "2026-08-15T00:00:00Z",
      secrets_included: false,
      entries: [{ path: "state/mcg.sqlite3", size_bytes: 2048, sha256: "b".repeat(64) }]
    }
  };
  client.createBackup.mockResolvedValue(backup);
  client.verifyBackup.mockResolvedValue(backup);
  render(<App />);
  const panel = await screen.findByLabelText("Portable backups");
  fireEvent.click(within(panel).getByRole("button", { name: "Create backup" }));
  await waitFor(() => expect(client.createBackup).toHaveBeenCalledWith(expect.any(String)));
  expect(await within(panel).findByText(backup.file_name)).toBeInTheDocument();
  expect(within(panel).getByText(/secrets excluded/)).toBeInTheDocument();
  fireEvent.click(within(panel).getByRole("button", { name: "Verify" }));
  await waitFor(() => expect(client.verifyBackup).toHaveBeenCalledWith(backup.backup_id));
});

test("encrypted credential backups are opt-in advanced actions with passphrase confirmation", async () => {
  const backup = {
    backup_id: "00000000-0000-4000-8000-000000000402",
    file_name: "00000000-0000-4000-8000-000000000402.mcgb",
    sha256: "c".repeat(64),
    size_bytes: 8192,
    manifest: {
      schema_version: 2,
      application_version: "0.1.0",
      created_at: "2026-08-16T00:00:00Z",
      secrets_included: true,
      secret_encryption: {
        schema_version: 1,
        entry_path: "secrets/credentials.v1.enc",
        algorithm: "xchacha20-poly1305",
        kdf: "argon2id",
        argon2_version: 19,
        memory_kib: 65536,
        iterations: 3,
        parallelism: 1,
        salt_base64: "public-salt",
        nonce_base64: "public-nonce"
      },
      entries: [
        { path: "state/mcg.sqlite3", size_bytes: 2048, sha256: "d".repeat(64) },
        { path: "secrets/credentials.v1.enc", size_bytes: 1024, sha256: "e".repeat(64) }
      ]
    }
  };
  client.createEncryptedBackup.mockResolvedValue(backup);
  render(<App />);
  expect(screen.queryByLabelText("Encrypted credential backup")).not.toBeInTheDocument();

  await enableAdvancedMode();
  const form = await screen.findByLabelText("Encrypted credential backup");
  const passphrase = within(form).getByLabelText("Passphrase");
  const confirmation = within(form).getByLabelText("Confirm passphrase");
  fireEvent.change(passphrase, { target: { value: "correct horse battery staple" } });
  fireEvent.change(confirmation, { target: { value: "different secure passphrase" } });
  fireEvent.click(within(form).getByRole("button", { name: "Create encrypted backup" }));
  expect(client.createEncryptedBackup).not.toHaveBeenCalled();
  expect(await screen.findByText("Encrypted backup passphrases do not match.")).toBeInTheDocument();

  fireEvent.change(confirmation, { target: { value: "correct horse battery staple" } });
  fireEvent.click(within(form).getByRole("button", { name: "Create encrypted backup" }));
  await waitFor(() =>
    expect(client.createEncryptedBackup).toHaveBeenCalledWith(
      expect.any(String),
      "correct horse battery staple"
    )
  );
  expect(await screen.findByText(backup.file_name)).toBeInTheDocument();
  expect(screen.getByText(/credentials encrypted/)).toBeInTheDocument();
  expect(passphrase).toHaveValue("");
  expect(confirmation).toHaveValue("");
});

test("system diagnostics are explicit concise and actionable", async () => {
  render(<App />);
  const panel = await screen.findByLabelText("System diagnostics");
  expect(within(panel).getByText("Diagnostics have not been run in this session.")).toBeInTheDocument();
  fireEvent.click(within(panel).getByRole("button", { name: "Run diagnostics" }));
  await waitFor(() => expect(client.runDiagnostics).toHaveBeenCalledOnce());
  expect(await within(panel).findByText(/Overall: WARNING/)).toBeInTheDocument();
  expect(within(panel).getByText(/Portable backups/)).toBeInTheDocument();
  expect(within(panel).getByText(/Provider dispatch recovery/)).toBeInTheDocument();
  expect(within(panel).getByText(/will not resend an indeterminate request automatically/)).toBeInTheDocument();
  expect(within(panel).getByText(/Create and retain a portable backup/)).toBeInTheDocument();
  expect(within(panel).queryByText(/Database integrity/)).not.toBeInTheDocument();

  await enableAdvancedMode();
  expect(within(panel).getByText(/Database integrity/)).toBeInTheDocument();
  expect(within(panel).getByText(/4 events/)).toBeInTheDocument();
  expect(within(panel).getByText(/1 active or indeterminate provider calls/)).toBeInTheDocument();
});

test("advanced organization inheritance pins a visible base revision", async () => {
  const base = {
    schema_version: 2,
    organization_id: "00000000-0000-4000-8000-000000000281",
    revision_id: "00000000-0000-4000-8000-000000000282",
    revision: 1,
    name: "Consumer Default",
    roles: [{ id: "role", kind: "coder", name: "Coder", purpose: "Build", tool_capabilities: ["filesystem.project"] }],
    relationships: [],
    policy: { prefer_minimal_organization: true, max_parallel_agents: 4, max_total_agents: 8, max_organizational_overhead_ratio: 0.3, default_lease_seconds: 900 },
    evaluation: { require_independent_evaluation: false, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 },
    created_at: "2026-08-18T00:00:00Z"
  };
  const inherited = {
    ...base,
    organization_id: "00000000-0000-4000-8000-000000000283",
    revision_id: "00000000-0000-4000-8000-000000000284",
    inherited_from_revision_id: base.revision_id,
    name: "Website specialists"
  };
  client.listOrganizations
    .mockResolvedValueOnce({ organizations: [base] })
    .mockResolvedValueOnce({ organizations: [base, inherited] });
  client.inheritOrganization.mockResolvedValue({ organization: inherited, event: {} });
  render(<App />);
  const panel = await screen.findByRole("region", { name: "Measured improvement" });
  expect((await within(panel).findAllByText("Consumer Default")).length).toBeGreaterThan(0);
  await enableAdvancedMode();
  fireEvent.change(within(panel).getByLabelText("New organization name"), { target: { value: "Website specialists" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Create inherited organization" }));
  await waitFor(() => expect(client.inheritOrganization).toHaveBeenCalledWith(base.organization_id, "Website specialists"));
  expect((await within(panel).findAllByText("Website specialists")).length).toBeGreaterThan(0);
  expect(within(panel).getByText("Inherited from a pinned revision")).toBeInTheDocument();
  expect(within(panel).getByText(`base ${base.revision_id}`)).toBeInTheDocument();
});

test("organization portability is simple to duplicate and advanced to export import and compare", async () => {
  const base = {
    schema_version: 2,
    organization_id: "00000000-0000-4000-8000-000000000291",
    revision_id: "00000000-0000-4000-8000-000000000292",
    revision: 1,
    name: "Portable team",
    roles: [{ id: "role", kind: "coder", name: "Coder", purpose: "Build", tool_capabilities: ["filesystem.project"] }],
    relationships: [],
    policy: { prefer_minimal_organization: true, max_parallel_agents: 4, max_total_agents: 8, max_organizational_overhead_ratio: 0.3, default_lease_seconds: 900 },
    evaluation: { require_independent_evaluation: false, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 },
    created_at: "2026-08-18T00:00:00Z"
  };
  const copy = {
    ...base,
    organization_id: "00000000-0000-4000-8000-000000000293",
    revision_id: "00000000-0000-4000-8000-000000000294",
    name: "Portable team copy"
  };
  const portable = {
    schema_version: 1,
    source: { organization_id: base.organization_id, revision_id: base.revision_id, revision: 1, exported_at: "2026-08-18T00:00:00Z" },
    definition: {
      name: base.name,
      roles: [{
        key: "role-000",
        kind: "coder",
        name: "Coder",
        purpose: "Build",
        prompt_policy: "Stay bounded",
        model_policy: { capability_class: "coding", preferred_models: [], allow_fallback: true, max_escalation_tier: 2 },
        tool_capabilities: ["filesystem.project"],
        memory_namespace: "role/coder"
      }],
      relationships: [],
      policy: base.policy,
      evaluation: base.evaluation
    },
    content_sha256: "a".repeat(64)
  };
  client.listOrganizations
    .mockResolvedValueOnce({ organizations: [base] })
    .mockResolvedValue({ organizations: [base, copy] });
  client.exportOrganization.mockResolvedValue(portable);
  client.loadOrganizationHistory.mockResolvedValue({ revisions: [base] });
  client.importOrganization
    .mockRejectedValueOnce(new Error("Connection closed after import"))
    .mockResolvedValue({ organization: copy, portable_sha256: portable.content_sha256, replayed: false });
  client.compareOrganizations.mockResolvedValue({
    equivalent: true,
    left_sha256: portable.content_sha256,
    right_sha256: portable.content_sha256,
    sections_changed: [],
    roles_added: [],
    roles_removed: [],
    roles_changed: [],
    relationships_added: [],
    relationships_removed: []
  });
  render(<App />);
  const panel = await screen.findByRole("region", { name: "Measured improvement" });
  fireEvent.click(await within(panel).findByRole("button", { name: "Duplicate" }));
  expect(await within(panel).findByRole("alert")).toHaveTextContent("Connection closed after import");
  const firstDuplicateId = client.importOrganization.mock.calls[0][1];
  fireEvent.click(within(panel).getByRole("button", { name: "Duplicate" }));
  await waitFor(() => expect(client.exportOrganization).toHaveBeenCalledWith(base.revision_id));
  expect(client.importOrganization).toHaveBeenLastCalledWith(
    portable,
    firstDuplicateId,
    "Portable team copy"
  );
  expect((await within(panel).findAllByText("Portable team copy")).length).toBeGreaterThan(0);

  await enableAdvancedMode();
  fireEvent.click(within(panel).getAllByRole("button", { name: "Show portable JSON" })[0]);
  expect((await within(panel).findByLabelText("Portable organization JSON") as HTMLTextAreaElement).value).toContain(portable.content_sha256);
  fireEvent.click(within(panel).getAllByRole("button", { name: "Revision history" })[0]);
  expect(await within(panel).findByRole("list", { name: "Portable team revision history" })).toHaveTextContent("Revision 1");
  fireEvent.click(within(panel).getByRole("button", { name: "Compare portable definitions" }));
  expect(await within(panel).findByText("Definitions are equivalent.")).toBeInTheDocument();
  fireEvent.click(within(panel).getByRole("button", { name: "Import as new organization" }));
  await waitFor(() => expect(client.importOrganization).toHaveBeenCalledTimes(3));
  expect(client.importOrganization.mock.calls[2][1]).not.toBe(firstDuplicateId);
});

test("signed organization sharing separates verification from owner import and exposes advanced trust controls", async () => {
  const organization = {
    schema_version: 2,
    organization_id: "00000000-0000-4000-8000-0000000002a1",
    revision_id: "00000000-0000-4000-8000-0000000002a2",
    revision: 1,
    name: "Verified website team",
    roles: [],
    relationships: [],
    policy: { prefer_minimal_organization: true, max_parallel_agents: 4, max_total_agents: 8, max_organizational_overhead_ratio: 0.3, default_lease_seconds: 900 },
    evaluation: { require_independent_evaluation: false, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 },
    created_at: "2026-08-20T00:00:00Z"
  };
  const portable = {
    schema_version: 1,
    source: { organization_id: organization.organization_id, revision_id: organization.revision_id, revision: 1, exported_at: organization.created_at },
    definition: { name: organization.name, roles: [], relationships: [], policy: organization.policy, evaluation: organization.evaluation },
    content_sha256: "a".repeat(64)
  };
  const organizationPackage = {
    schema_version: 1,
    registry: "https://organizations.example.com",
    package: "verified-website-team",
    publisher: "example-publisher",
    portable,
    signature: { scheme: "ed25519-v1", signature_base64: "c".repeat(88) }
  };
  const review = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-0000000002a3",
    package: organizationPackage,
    evidence: {
      schema_version: 1,
      registry: organizationPackage.registry,
      package: organizationPackage.package,
      publisher: organizationPackage.publisher,
      publisher_key_sha256: "b".repeat(64),
      portable_sha256: portable.content_sha256,
      package_sha256: "c".repeat(64),
      signature_sha256: "d".repeat(64)
    },
    state: "pending" as const,
    created_at: organization.created_at,
    updated_at: organization.created_at
  };
  const publisher = {
    schema_version: 1,
    id: organizationPackage.publisher,
    display_name: "Example publisher",
    public_key_base64: "e".repeat(44),
    registries: [organizationPackage.registry],
    state: "trusted" as const,
    revision: 1,
    reason: "Owner reviewed",
    created_at: organization.created_at,
    updated_at: organization.created_at
  };
  client.listOrganizationPublishers.mockResolvedValue([publisher]);
  client.listOrganizationShareReviews.mockResolvedValueOnce([]).mockResolvedValue([review]);
  client.reviewSharedOrganization.mockResolvedValue({ review, replayed: false, event: {} });
  client.importSharedOrganization.mockResolvedValue({
    review: { ...review, state: "imported", import_id: "00000000-0000-4000-8000-0000000002a4", organization_id: organization.organization_id, decision_reason: "Reviewed for this project" },
    organization,
    replayed: false,
    events: []
  });
  client.trustOrganizationPublisher.mockResolvedValue({ publisher, replayed: true });
  client.revokeOrganizationPublisher.mockResolvedValue({ publisher: { ...publisher, state: "revoked", revision: 2 }, replayed: false });
  render(<App />);

  const sharing = await screen.findByLabelText("Signed organization sharing");
  fireEvent.change(within(sharing).getByLabelText("Signed organization package"), { target: { value: JSON.stringify(organizationPackage) } });
  fireEvent.click(within(sharing).getByRole("button", { name: "Verify for review" }));
  await waitFor(() => expect(client.reviewSharedOrganization).toHaveBeenCalledWith(expect.any(String), organizationPackage));
  expect(client.importSharedOrganization).not.toHaveBeenCalled();
  expect(await within(sharing).findByText(organization.name)).toBeInTheDocument();
  expect(within(sharing).getByText(review.evidence.publisher_key_sha256)).toBeInTheDocument();
  expect(within(sharing).getByText(review.evidence.package_sha256)).toBeInTheDocument();
  expect(within(sharing).getByRole("button", { name: "Import verified organization" })).toBeDisabled();
  fireEvent.change(within(sharing).getByLabelText("Owner decision reason"), { target: { value: "Reviewed for this project" } });
  fireEvent.click(within(sharing).getByRole("button", { name: "Import verified organization" }));
  await waitFor(() => expect(client.importSharedOrganization).toHaveBeenCalledWith(
    review.id,
    expect.any(String),
    "Reviewed for this project",
    undefined
  ));

  await enableAdvancedMode();
  fireEvent.change(within(sharing).getByLabelText("Publisher identity"), { target: { value: publisher.id } });
  fireEvent.change(within(sharing).getByLabelText("Display name"), { target: { value: publisher.display_name } });
  fireEvent.change(within(sharing).getByLabelText("Ed25519 public key"), { target: { value: publisher.public_key_base64 } });
  fireEvent.change(within(sharing).getByLabelText("Trusted organization registries"), { target: { value: publisher.registries[0] } });
  fireEvent.change(within(sharing).getByLabelText("Owner trust or revocation reason"), { target: { value: "Rechecked exact registry and key" } });
  fireEvent.click(within(sharing).getByRole("button", { name: "Trust or revise publisher" }));
  await waitFor(() => expect(client.trustOrganizationPublisher).toHaveBeenCalledWith(expect.objectContaining({
    id: publisher.id,
    expected_revision: publisher.revision,
    registries: publisher.registries,
    reason: "Rechecked exact registry and key"
  })));
  fireEvent.change(within(sharing).getByLabelText("Owner trust or revocation reason"), { target: { value: "Publisher relationship ended" } });
  fireEvent.click(within(sharing).getByRole("button", { name: "Revoke permanently" }));
  await waitFor(() => expect(client.revokeOrganizationPublisher).toHaveBeenCalledWith(publisher.id, publisher.revision, "Publisher relationship ended"));
});

test("measured improvement stays simple by default and exposes controlled transitions in advanced mode", async () => {
  const experiment = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000301",
    organization_id: "00000000-0000-4000-8000-000000000302",
    baseline_revision_id: "00000000-0000-4000-8000-000000000303",
    candidate_revision_id: "00000000-0000-4000-8000-000000000304",
    mode: "shadow" as const,
    state: "draft" as const,
    hypothesis: "Reduce repeated build failures",
    task_classes: ["coding/build"],
    required_benchmarks: 2,
    maximum_regressions: 0,
    minimum_confidence: { basis_points: 8000, sample_size: 10 },
    observation_ids: [],
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.listOrganizations.mockResolvedValue({
    organizations: [{
      schema_version: 2,
      organization_id: experiment.organization_id,
      revision_id: experiment.baseline_revision_id,
      revision: 1,
      name: "Consumer Fixture",
      roles: [],
      relationships: [],
      policy: { prefer_minimal_organization: true, max_parallel_agents: 4, max_total_agents: 8, max_organizational_overhead_ratio: 0.3, default_lease_seconds: 900 },
      evaluation: { require_independent_evaluation: false, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 },
      created_at: "2026-08-14T00:00:00Z"
    }]
  });
  client.listExperiments.mockResolvedValue({ experiments: [experiment] });
  client.loadExperimentEvidence.mockResolvedValue({
    shadow: { receipts: [{}] },
    benchmarks: { benchmarks: [{}] },
    decisions: { decisions: [] }
  });
  client.transitionExperiment.mockResolvedValue({});
  render(<App />);
  const panel = await screen.findByRole("region", { name: "Measured improvement" });
  expect(within(panel).getByText("Candidates stay inactive until benchmark evidence supports promotion. Shadow work can evaluate, but cannot change a real task.")).toBeTruthy();
  fireEvent.change(within(panel).getByLabelText("Organization for experiments"), { target: { value: experiment.organization_id } });
  fireEvent.click(within(panel).getByRole("button", { name: "Load experiments" }));
  const hypothesis = await within(panel).findByText(experiment.hypothesis);
  expect(hypothesis.closest("article")).toHaveClass("experiment-card");
  expect(within(panel).getByText("1 shadow · 1/2 benchmarks · 0 decisions")).toBeTruthy();
  expect(within(panel).queryByLabelText("Measured improvement JSON")).toBeNull();
  await enableAdvancedMode();
  expect(within(panel).getByLabelText("Measured improvement JSON")).toBeTruthy();
  fireEvent.click(within(panel).getByRole("button", { name: "Move to ready" }));
  await waitFor(() => expect(client.transitionExperiment).toHaveBeenCalledWith(
    experiment.id,
    "draft",
    "ready"
  ));
});

test("simple SSH flow registers a pinned host and performs read-only inspection", async () => {
  const sshHost = {
    id: "00000000-0000-4000-8000-000000000191",
    revision: 1,
    definition_hash: "a".repeat(64),
    display_name: "Build VM",
    hostname: "192.168.50.109",
    port: 22,
    username: "builder",
    known_hosts_entry: "192.168.50.109 ssh-ed25519 AAAAfixture",
    host_key_sha256: "SHA256:fixture",
    capabilities: ["os:linux"],
    state: "active" as const,
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.registerSshHost.mockResolvedValue(sshHost);
  client.runSshOperation.mockResolvedValue({
    operation: { id: "op", host_id: sshHost.id, host_revision: 1, state: "succeeded", kind: { kind: "inspect" }, detail: "ok", artifact_sha256: [], created_at: sshHost.created_at, updated_at: sshHost.updated_at },
    replayed: false
  });
  client.loadSshOperations.mockResolvedValue({ operations: [{
    id: "00000000-0000-4000-8000-000000000192", host_id: sshHost.id, host_revision: 1,
    state: "failed", kind: { kind: "inspect" }, detail: "OpenSSH operation exited with 255",
    exit_code: 255, artifact_sha256: ["b".repeat(64)], created_at: sshHost.created_at,
    updated_at: sshHost.updated_at
  }] });
  render(<App />);
  const panel = await screen.findByRole("region", { name: "SSH hosts" });
  fireEvent.change(within(panel).getByLabelText("Host name"), { target: { value: "Build VM" } });
  fireEvent.change(within(panel).getByLabelText("Hostname or IP"), { target: { value: sshHost.hostname } });
  fireEvent.change(within(panel).getByLabelText("SSH username"), { target: { value: sshHost.username } });
  fireEvent.change(within(panel).getByLabelText("Pinned known_hosts line"), { target: { value: sshHost.known_hosts_entry } });
  fireEvent.change(within(panel).getByLabelText("Verified host fingerprint"), { target: { value: sshHost.host_key_sha256 } });
  fireEvent.click(within(panel).getByRole("button", { name: "Register pinned host" }));
  await within(panel).findByRole("heading", { name: "Build VM" });
  fireEvent.click(within(panel).getByRole("button", { name: "Inspect host" }));
  await waitFor(() => expect(client.runSshOperation).toHaveBeenCalledWith(
    sshHost.id,
    expect.objectContaining({ operation: { kind: "inspect" }, timeout_seconds: 60 })
  ));
  expect(await within(panel).findByText("OpenSSH operation exited with 255 · exit 255")).toBeInTheDocument();
});

test("simple SSH workflow pins selected hosts and exposes run state", async () => {
  const now = "2026-08-18T00:00:00Z";
  const hosts = [
    { id: "00000000-0000-4000-8000-000000000201", revision: 3, definition_hash: "a".repeat(64), display_name: "Builder", hostname: "builder.test", port: 22, username: "mcg", known_hosts_entry: "builder.test ssh-ed25519 AAAA", host_key_sha256: "SHA256:a", capabilities: ["os:linux"], state: "active" as const, created_at: now, updated_at: now },
    { id: "00000000-0000-4000-8000-000000000202", revision: 2, definition_hash: "b".repeat(64), display_name: "Tester", hostname: "tester.test", port: 22, username: "mcg", known_hosts_entry: "tester.test ssh-ed25519 BBBB", host_key_sha256: "SHA256:b", capabilities: ["os:linux"], state: "active" as const, created_at: now, updated_at: now }
  ];
  client.listSshHosts.mockResolvedValue({ hosts });
  client.createSshWorkflow.mockImplementation(async (request) => ({
    workflow: { schema_version: 1, definition_hash: "c".repeat(64), state: "planned", created_at: now, updated_at: now, ...request, steps: request.steps.map((step: Record<string, unknown>) => ({ ...step, operation_id: crypto.randomUUID(), state: "pending", detail: "" })) },
    replayed: false
  }));
  client.runSshWorkflow.mockImplementation(async (workflowId) => ({
    workflow: { schema_version: 1, id: workflowId, definition_hash: "c".repeat(64), display_name: "Cross-platform check", state: "succeeded", created_at: now, updated_at: now, steps: [] },
    replayed: false
  }));
  render(<App />);
  const workflows = await screen.findByRole("region", { name: "SSH workflows" });
  fireEvent.change(within(workflows).getByLabelText("Workflow name"), { target: { value: "Cross-platform check" } });
  fireEvent.click(within(workflows).getByRole("checkbox", { name: /Builder/ }));
  fireEvent.click(within(workflows).getByRole("checkbox", { name: /Tester/ }));
  fireEvent.click(within(workflows).getByRole("button", { name: "Create durable workflow" }));
  await within(workflows).findByRole("heading", { name: "Cross-platform check" });
  const request = client.createSshWorkflow.mock.calls[0][0];
  expect(request.steps.map((step: { host_revision: number }) => step.host_revision)).toEqual([3, 2]);
  expect(request.steps[1].dependencies).toEqual([request.steps[0].id]);
  fireEvent.click(within(workflows).getByRole("button", { name: "Run workflow" }));
  await waitFor(() => expect(client.runSshWorkflow).toHaveBeenCalledWith(request.id));
  expect(await within(workflows).findByText("succeeded")).toBeInTheDocument();
});

test("simple Sites flow registers a selected project and records governed health", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000121",
    project_id: "00000000-0000-4000-8000-000000000122",
    root: "C:\\Projects\\site",
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  const site = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000123",
    definition_hash: "a".repeat(64),
    revision: 1,
    project_id: project.project_id,
    display_name: "Production site",
    environment: "production",
    public_url: "https://example.com/",
    health_url: "https://example.com/health",
    hosting_provider: "fixture",
    state: "active",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.registerSite.mockResolvedValue(site);
  client.checkSiteHealth.mockResolvedValue({
    replayed: false,
    check: {
      id: "00000000-0000-4000-8000-000000000124",
      site_id: site.id,
      state: "healthy",
      status_code: 204,
      detail: "HTTP 204",
      checked_at: "2026-08-14T00:01:00Z"
    }
  });
  client.updateSite.mockResolvedValue({
    replayed: false,
    site: { ...site, revision: 2, state: "monitoring_paused" }
  });
  client.createSiteMonitor.mockResolvedValue({
    replayed: false,
    monitor: {
      schema_version: 1,
      site_id: site.id,
      schedule_id: "00000000-0000-4000-8000-000000000125",
      interval_seconds: 300,
      expected_status: 200,
      created_at: "2026-08-14T00:00:00Z",
      updated_at: "2026-08-14T00:00:00Z"
    },
    schedule: {}
  });
  render(<App />);
  const panel = await screen.findByRole("region", { name: "Sites" });
  fireEvent.change(within(panel).getByLabelText("Site project"), { target: { value: project.project_id } });
  fireEvent.change(within(panel).getByLabelText("Site name"), { target: { value: "Production site" } });
  fireEvent.change(within(panel).getByLabelText("Public URL"), { target: { value: "https://example.com" } });
  fireEvent.change(within(panel).getByLabelText("Health URL"), { target: { value: "https://example.com/health" } });
  fireEvent.change(within(panel).getByLabelText("Hosting provider"), { target: { value: "fixture" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Register Site" }));
  await waitFor(() => expect(client.registerSite).toHaveBeenCalledWith(expect.objectContaining({
    project_id: project.project_id,
    display_name: "Production site",
    state: "active"
  })));
  fireEvent.click(await within(panel).findByRole("button", { name: "Check health" }));
  await waitFor(() => expect(client.checkSiteHealth).toHaveBeenCalledWith(site.id, expect.any(String)));
  expect(await within(panel).findByText("Health: healthy · HTTP 204")).toBeInTheDocument();
  fireEvent.click(within(panel).getByRole("button", { name: "Enable automatic monitoring" }));
  await waitFor(() => expect(client.createSiteMonitor).toHaveBeenCalledWith(site.id, expect.objectContaining({
    interval_seconds: 300,
    expected_status: 200
  })));
  expect(await within(panel).findByText("Automatic checks: every 300 seconds")).toBeInTheDocument();
  fireEvent.click(within(panel).getByRole("button", { name: "Pause monitoring" }));
  await waitFor(() => expect(client.updateSite).toHaveBeenCalledWith(site.id, expect.objectContaining({
    expected_revision: 1,
    state: "monitoring_paused"
  })));
  expect(await within(panel).findByRole("button", { name: "Resume monitoring" })).toBeVisible();
});

test("simple scheduler creates restart-safe recurring work for a selected project", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000021",
    project_id: "00000000-0000-4000-8000-000000000022",
    root: "C:\\Projects\\scheduled",
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  const schedule = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000023",
    definition_hash: "fixture-definition",
    name: "Hourly health check",
    project_id: project.project_id,
    goal: "Check project health",
    trigger: { kind: "interval", seconds: 3600 },
    state: "enabled",
    next_run_at: "2026-08-14T01:00:00Z",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.createSchedule.mockResolvedValue(schedule);
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Use project" }));
  const scheduler = screen.getByRole("region", { name: "Scheduler" });
  await enableAdvancedMode();
  fireEvent.change(within(scheduler).getByLabelText("Trigger"), { target: { value: "cron" } });
  expect(within(scheduler).getByLabelText("Five-field cron")).toHaveValue("0 2 * * *");
  await enableSimpleMode();
  expect(within(scheduler).getByLabelText("Repeat")).toHaveValue("3600");
  fireEvent.change(within(scheduler).getByLabelText("Name"), { target: { value: schedule.name } });
  fireEvent.change(within(scheduler).getByLabelText("Scheduled goal"), { target: { value: schedule.goal } });
  fireEvent.click(within(scheduler).getByRole("button", { name: "Create schedule" }));

  expect(await within(scheduler).findByRole("heading", { name: schedule.name })).toBeInTheDocument();
  expect(client.createSchedule).toHaveBeenCalledWith(expect.objectContaining({
    schedule_id: expect.any(String),
    project_id: project.project_id,
    trigger: { kind: "interval", seconds: 3600 },
    budget: {
      max_tokens: 32768,
      max_cost_usd: undefined,
      max_duration_seconds: 3600
    },
    max_model_calls: 16
  }));
});

test("platform reservation stays truthful and reuses identity after an uncertain failure", async () => {
  const session = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000071",
    definition_hash: "a".repeat(64),
    kind: "browser",
    state: "requested",
    owner: "mcg",
    display_name: "Research checkout",
    allowed_origins: ["https://example.com"],
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z",
    expires_at: "2026-08-14T01:00:00Z",
    recovery_count: 0
  };
  client.requestPlatformSession
    .mockRejectedValueOnce(new Error("response lost"))
    .mockResolvedValueOnce(session);
  render(<App />);
  const panel = screen.getByRole("region", { name: "Computer and browser sessions" });
  await within(panel).findByText("browser automation");
  fireEvent.change(within(panel).getByLabelText("Name"), { target: { value: session.display_name } });
  fireEvent.change(within(panel).getByLabelText("Allowed sites (required before starting)"), { target: { value: "https://example.com" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Reserve session" }));
  expect(await within(panel).findByRole("alert")).toHaveTextContent("response lost");
  const firstId = client.requestPlatformSession.mock.calls[0][0].session_id;
  fireEvent.click(within(panel).getByRole("button", { name: "Reserve session" }));
  expect(await within(panel).findByRole("heading", { name: session.display_name })).toBeInTheDocument();
  expect(within(panel).getByText("Waiting for a verified executor")).toBeInTheDocument();
  expect(client.requestPlatformSession.mock.calls[1][0]).toMatchObject({
    session_id: firstId,
    kind: "browser",
    allowed_origins: ["https://example.com"]
  });
});

test("private browser starts, navigates, and displays only the safe URL", async () => {
  const requested = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000072",
    definition_hash: "b".repeat(64),
    kind: "browser",
    state: "requested",
    owner: "mcg",
    display_name: "Private research",
    allowed_origins: ["https://example.com"],
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z",
    expires_at: "2026-08-14T01:00:00Z",
    recovery_count: 0
  };
  const active = { ...requested, state: "active" };
  client.listPlatformSessions.mockResolvedValue({ sessions: [requested] });
  client.attachBrowserSession.mockResolvedValue({
    session: active,
    browser: { session_id: requested.id, mode: "headless", safe_url: "about:blank", handed_off: false, denied_requests: 0 }
  });
  client.navigateBrowserSession.mockResolvedValue({
    session_id: requested.id,
    mode: "headless",
    safe_url: "https://example.com/result",
    title: "Safe result",
    handed_off: false,
    denied_requests: 0
  });
  render(<App />);
  const panel = screen.getByRole("region", { name: "Computer and browser sessions" });
  await within(panel).findByRole("heading", { name: requested.display_name });
  fireEvent.click(within(panel).getByRole("button", { name: "Start private browser" }));
  const url = await within(panel).findByLabelText(`URL for ${requested.display_name}`);
  fireEvent.change(url, { target: { value: "https://example.com/result?token=secret" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Go" }));
  await within(panel).findByText("Safe result · https://example.com/result");
  expect(client.attachBrowserSession).toHaveBeenCalledWith(requested.id, "headless");
  expect(client.navigateBrowserSession).toHaveBeenCalledWith(
    requested.id,
    "https://example.com/result?token=secret"
  );
  expect(within(panel).queryByText(/token=secret/)).not.toBeInTheDocument();
});

test("advanced computer reservation validates and submits a stable target region", async () => {
  const requested = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000075",
    definition_hash: "9".repeat(64),
    kind: "computer_use",
    state: "requested",
    owner: "mcg",
    display_name: "Stable desktop region",
    allowed_origins: [],
    computer_capabilities: ["screen_read", "mouse", "keyboard"],
    computer_target: { region: { x: 20, y: 30, width: 320, height: 240 } },
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z",
    expires_at: "2026-08-14T01:00:00Z",
    recovery_count: 0
  };
  client.requestPlatformSession.mockResolvedValue(requested);
  render(<App />);
  await enableAdvancedMode();
  const panel = screen.getByRole("region", { name: "Computer and browser sessions" });
  fireEvent.change(within(panel).getByLabelText("Session type"), { target: { value: "computer_use" } });
  fireEvent.change(within(panel).getByLabelText("Name"), { target: { value: requested.display_name } });
  fireEvent.change(within(panel).getByLabelText("Target region x"), { target: { value: "20" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Reserve session" }));
  expect(await within(panel).findByRole("alert")).toHaveTextContent("Enter all four region values");
  expect(client.requestPlatformSession).not.toHaveBeenCalled();
  fireEvent.change(within(panel).getByLabelText("Target region y"), { target: { value: "30" } });
  fireEvent.change(within(panel).getByLabelText("Target region width"), { target: { value: "320" } });
  fireEvent.change(within(panel).getByLabelText("Target region height"), { target: { value: "240" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Reserve session" }));
  await waitFor(() => expect(client.requestPlatformSession).toHaveBeenCalledWith(expect.objectContaining({
    kind: "computer_use",
    computer_target: { region: { x: 20, y: 30, width: 320, height: 240 } }
  })));
});

test("advanced computer session captures a digest-bound frame and submits typed input", async () => {
  const requested = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000073",
    definition_hash: "c".repeat(64),
    kind: "computer_use",
    state: "requested",
    owner: "mcg",
    display_name: "Desktop fixture",
    allowed_origins: [],
    computer_capabilities: ["screen_read", "mouse", "keyboard"],
    computer_target: {},
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z",
    expires_at: "2026-08-14T01:00:00Z",
    recovery_count: 0
  };
  const active = { ...requested, state: "active" };
  const frame = {
    observation: {
      session_id: requested.id,
      display_id: 1,
      region: { x: 0, y: 0, width: 800, height: 600 },
      capabilities: requested.computer_capabilities,
      handed_off: false
    },
    sha256: "d".repeat(64),
    media_type: "image/png",
    png_base64: "iVBORw0KGgo=",
    captured_at: "2026-08-14T00:00:00Z"
  };
  client.listPlatformSessions.mockResolvedValue({ sessions: [requested] });
  client.attachComputerSession.mockResolvedValue({ session: active, computer: frame.observation });
  client.captureComputerSession.mockResolvedValue(frame);
  client.applyComputerActions.mockResolvedValue({
    batch: {
      id: "00000000-0000-4000-8000-000000000074",
      session_id: requested.id,
      request_hash: "f".repeat(64),
      state: "succeeded",
      action_count: 2,
      result_frame_sha256: "e".repeat(64),
      detail: "computer input batch completed",
      created_at: "2026-08-14T00:00:00Z",
      updated_at: "2026-08-14T00:00:01Z"
    },
    replayed: false,
    result: {
      prior_frame_sha256: frame.sha256,
      actions_executed: 2,
      frame: { ...frame, sha256: "e".repeat(64) }
    }
  });
  render(<App />);
  await enableAdvancedMode();
  const panel = screen.getByRole("region", { name: "Computer and browser sessions" });
  await within(panel).findByRole("heading", { name: requested.display_name });
  fireEvent.click(within(panel).getByRole("button", { name: "Start computer session" }));
  expect(await within(panel).findByAltText(`Current governed computer frame for ${requested.display_name}`)).toBeInTheDocument();
  fireEvent.change(within(panel).getByLabelText(`X coordinate for ${requested.display_name}`), { target: { value: "25" } });
  fireEvent.change(within(panel).getByLabelText(`Y coordinate for ${requested.display_name}`), { target: { value: "40" } });
  fireEvent.change(within(panel).getByLabelText(`Text for ${requested.display_name}`), { target: { value: "hello" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Click and type" }));
  await waitFor(() => expect(client.applyComputerActions).toHaveBeenCalledWith(requested.id, {
    batch_id: expect.any(String),
    expected_frame_sha256: frame.sha256,
    actions: [
      { type: "click", x: 25, y: 40, button: "left" },
      { type: "type_text", text: "hello" }
    ]
  }));
});

test("scheduler retries reuse one identity and rotate it only after success", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000031",
    project_id: "00000000-0000-4000-8000-000000000032",
    root: "C:\\Projects\\retry-scheduled",
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  const completed = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000033",
    definition_hash: "retry-definition",
    name: "Retry-safe schedule",
    project_id: project.project_id,
    goal: "Survive a lost response",
    trigger: { kind: "interval", seconds: 3600 },
    state: "enabled",
    next_run_at: "2026-08-14T01:00:00Z",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.createSchedule
    .mockRejectedValueOnce(new Error("response lost"))
    .mockResolvedValueOnce(completed)
    .mockResolvedValueOnce({ ...completed, id: "00000000-0000-4000-8000-000000000034", name: "Next schedule" });
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Use project" }));
  const scheduler = screen.getByRole("region", { name: "Scheduler" });
  fireEvent.change(within(scheduler).getByLabelText("Name"), { target: { value: completed.name } });
  fireEvent.change(within(scheduler).getByLabelText("Scheduled goal"), { target: { value: completed.goal } });
  const create = within(scheduler).getByRole("button", { name: "Create schedule" });
  fireEvent.click(create);
  expect(await within(scheduler).findByRole("alert")).toHaveTextContent("response lost");
  const firstIdentity = client.createSchedule.mock.calls[0][0].schedule_id;

  fireEvent.click(create);
  expect(await within(scheduler).findByRole("heading", { name: completed.name })).toBeInTheDocument();
  expect(client.createSchedule.mock.calls[1][0].schedule_id).toBe(firstIdentity);

  fireEvent.change(within(scheduler).getByLabelText("Name"), { target: { value: "Next schedule" } });
  fireEvent.change(within(scheduler).getByLabelText("Scheduled goal"), { target: { value: "Run next" } });
  fireEvent.click(create);
  await waitFor(() => expect(client.createSchedule).toHaveBeenCalledTimes(3));
  expect(client.createSchedule.mock.calls[2][0].schedule_id).not.toBe(firstIdentity);
});

test("advanced mode reveals runtime details", async () => {
  render(<App />);

  expect(screen.queryByRole("heading", { name: "Runtime status" })).not.toBeInTheDocument();
  await enableAdvancedMode();
  expect(screen.getByRole("heading", { name: "Runtime status" })).toBeInTheDocument();
});

test("mode persistence restores keyboard focus after the temporarily disabled control returns", async () => {
  render(<App />);

  const advanced = await screen.findByRole("button", { name: "Advanced mode" });
  await waitFor(() => expect(advanced).toBeEnabled());
  advanced.focus();
  expect(advanced).toHaveFocus();
  fireEvent.click(advanced);

  const simple = await screen.findByRole("button", { name: "Simple mode" });
  await waitFor(() => expect(simple).toBeEnabled());
  await waitFor(() => expect(simple).toHaveFocus());
});

test("OpenAI subscription browser login is the clear default and confirms connection", async () => {
  client.start.mockResolvedValue({
    session_id: "fixture-session",
    connection_id: "openai-subscription",
    auth_mode: "chat_gpt_browser_oauth",
    verification_url: "https://auth.openai.com/oauth/authorize"
  });
  client.complete.mockResolvedValue({
    connection: {
      id: "openai-subscription",
      display_name: "OpenAI Subscription",
      auth_mode: "chat_gpt_browser_oauth"
    },
    catalog: { models: [] }
  });
  render(<App />);

  expect(screen.getByText(/No CLI pairing code\./)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Continue with OpenAI" }));
  expect(await screen.findByText("Finish signing in with OpenAI in your browser, then return here.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Open sign-in page" })).toBeInTheDocument();
  expect(screen.queryByText("Code:", { exact: false })).not.toBeInTheDocument();
  expect(client.start).toHaveBeenCalledWith("browser");
  expect(client.open).toHaveBeenCalledWith("https://auth.openai.com/oauth/authorize");

  fireEvent.click(screen.getByRole("button", { name: "Finish connection" }));
  expect(await screen.findByText("OpenAI Subscription connected")).toBeInTheDocument();
  expect(client.complete).toHaveBeenCalledWith("fixture-session");
});

test("advanced mode keeps device-code login an explicit headless fallback", async () => {
  client.start.mockResolvedValue({
    session_id: "device-session",
    connection_id: "openai-subscription",
    auth_mode: "chat_gpt_device_code",
    verification_url: "https://auth.openai.com/codex/device",
    user_code: "ABCD-1234"
  });
  render(<App />);

  await enableAdvancedMode();
  fireEvent.click(screen.getByText("Can't open a browser?"));
  fireEvent.click(screen.getByRole("button", { name: "Use a device code instead" }));

  expect(await screen.findByText("Code:", { exact: false })).toHaveTextContent("ABCD-1234");
  expect(screen.getByRole("button", { name: "Open device authorization page" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "I've entered the code" })).toBeInTheDocument();
  expect(client.start).toHaveBeenCalledWith("device_code");
  expect(client.open).toHaveBeenCalledWith("https://auth.openai.com/codex/device");
});

test("consumer provider setup discovers a local endpoint and advanced mode shows honest model cost", async () => {
  const connection = {
    contract_version: 1,
    id: "local_gpu",
    display_name: "Local GPU",
    kind: "open_ai_compatible",
    base_url: "http://127.0.0.1:1234/v1",
    auth_mode: "none"
  };
  const catalog = {
    connection_id: connection.id,
    models: [{
      id: "local-model",
      display_name: "Local Model",
      capabilities: { responses: true, streaming: true },
      metadata: {}
    }],
    discovered_at: "2026-08-14T00:00:00Z"
  };
  client.configureProvider.mockResolvedValue({ connection, catalog });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "Add API or local provider" }));
  fireEvent.change(screen.getByLabelText("Provider name"), { target: { value: connection.display_name } });
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: connection.base_url } });
  fireEvent.change(screen.getByLabelText("Authentication"), { target: { value: "none" } });
  expect(screen.queryByLabelText("API key")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Connect provider" }));

  expect(await screen.findByRole("heading", { name: connection.display_name })).toBeInTheDocument();
  expect(screen.getByText("Endpoint-reported billing · 1 models")).toBeInTheDocument();
  expect(client.configureProvider).toHaveBeenCalledWith(expect.objectContaining(connection), undefined);

  await enableAdvancedMode();
  expect(screen.getByText("Local Model")).toBeInTheDocument();
  expect(screen.getByText("Cost unknown")).toBeInTheDocument();
});

test("provider disconnect confirms intent and removes the connection from the consumer UI", async () => {
  const connection = {
    contract_version: 1,
    id: "subscription",
    display_name: "OpenAI Subscription",
    kind: "open_ai",
    base_url: "https://chatgpt.com/backend-api/codex",
    auth_mode: "chat_gpt_device_code"
  };
  client.listProviders.mockResolvedValue({
    providers: [{ connection, catalog: { connection_id: connection.id, models: [], discovered_at: "2026-08-15T00:00:00Z" } }]
  });
  client.disconnectProvider.mockResolvedValue(undefined);
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
  render(<App />);

  expect(await screen.findByRole("heading", { name: connection.display_name })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));

  await waitFor(() => expect(client.disconnectProvider).toHaveBeenCalledWith(connection.id));
  expect(confirm).toHaveBeenCalledWith(expect.stringContaining("protected credentials"));
  expect(screen.queryByRole("heading", { name: connection.display_name })).not.toBeInTheDocument();
  expect(screen.getByText("OpenAI Subscription disconnected")).toBeInTheDocument();
  confirm.mockRestore();
});

test("provider workspace shows durable privacy-preserving proxy usage", async () => {
  client.proxyUsage.mockResolvedValue({
    conversation_count: 2,
    call_count: 3,
    usage: { input_tokens: 90, cached_input_tokens: 10, output_tokens: 30, reasoning_tokens: 5, total_tokens: 120 },
    known_actual_cost_usd: 0.012345,
    unknown_cost_call_count: 1
  });
  client.proxyConversations.mockResolvedValue({ conversations: [{
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000081",
    created_at: "2026-08-15T00:00:00Z",
    updated_at: "2026-08-15T00:01:00Z"
  }] });
  render(<App />);

  expect(await screen.findByRole("heading", { name: "120 tokens" })).toBeInTheDocument();
  expect(screen.getByText(/2 conversations/)).toHaveTextContent("$0.012345 known cost");
  expect(screen.getByText(/not prompts or generated text/)).toBeInTheDocument();
  expect(screen.queryByLabelText("Proxy conversations")).not.toBeInTheDocument();

  await enableAdvancedMode();
  expect(screen.getByLabelText("Proxy conversations")).toHaveTextContent("Standalone conversation");
});

test("simple MCP setup discovers a server without exposing advanced credentials", async () => {
  client.configureMcp.mockResolvedValue({
    connection: {
      contract_version: 1,
      id: "my_tools",
      display_name: "My Tools",
      transport: { kind: "streamable_http", url: "https://tools.example.com/mcp" },
      auth_mode: "none",
      protocol_mode: "auto",
      enabled: true
    },
    catalog: {
      connection_id: "my_tools",
      protocol_version: "2026-07-28",
      server_info: { name: "fixture" },
      tools: [{ name: "lookup" }],
      discovered_at: "2026-08-14T00:00:00Z",
      expires_at: "2026-08-14T00:05:00Z"
    }
  });
  render(<App />);

  expect(screen.queryByLabelText("Bearer token")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Server name"), { target: { value: "My Tools" } });
  fireEvent.change(screen.getByLabelText("Server URL"), {
    target: { value: "https://tools.example.com/mcp" }
  });
  fireEvent.click(screen.getByRole("button", { name: "Add server" }));

  expect(await screen.findByRole("heading", { name: "My Tools" })).toBeInTheDocument();
  expect(screen.getByText("1 tools · 2026-07-28")).toBeInTheDocument();
  expect(client.configureMcp).toHaveBeenCalledWith(
    expect.objectContaining({
      id: "my_tools",
      auth_mode: "none",
      protocol_mode: "auto"
    }),
    undefined
  );
});

test("advanced MCP setup passes stdio executable arguments without a credential", async () => {
  client.configureMcp.mockResolvedValue({
    connection: {
      contract_version: 1,
      id: "local_tools",
      display_name: "Local Tools",
      transport: { kind: "stdio", command: "npx", args: ["-y", "fixture-server"] },
      auth_mode: "none",
      protocol_mode: "auto",
      enabled: true
    },
    catalog: {
      connection_id: "local_tools",
      protocol_version: "2026-07-28",
      server_info: { name: "fixture" },
      tools: [],
      discovered_at: "2026-08-14T00:00:00Z",
      expires_at: "2026-08-14T00:05:00Z"
    }
  });
  render(<App />);

  await enableAdvancedMode();
  fireEvent.change(screen.getByLabelText("Transport"), { target: { value: "stdio" } });
  expect(screen.queryByLabelText("Server URL")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Authentication")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Server name"), { target: { value: "Local Tools" } });
  fireEvent.change(screen.getByLabelText("Executable"), { target: { value: "npx" } });
  fireEvent.change(screen.getByLabelText("Arguments (one per line)"), {
    target: { value: "-y\nfixture-server" }
  });
  fireEvent.click(screen.getByRole("button", { name: "Add server" }));

  expect(await screen.findByRole("heading", { name: "Local Tools" })).toBeInTheDocument();
  expect(client.configureMcp).toHaveBeenCalledWith(
    expect.objectContaining({
      transport: { kind: "stdio", command: "npx", args: ["-y", "fixture-server"] },
      auth_mode: "none"
    }),
    undefined
  );
});

test("consumer task flow creates, starts, and reveals durable history", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000010",
    project_id: "00000000-0000-4000-8000-000000000011",
    root: "C:\\Projects\\sample",
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  const pendingTask = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000001",
    project_id: project.project_id,
    goal: "Build a durable site",
    constraints: [],
    success_conditions: [],
    risk: "medium",
    budget: {},
    state: "pending",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.createTask.mockResolvedValue({
    task: pendingTask,
    event: { sequence: 1, kind: "task_created", occurred_at: pendingTask.created_at }
  });
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.launchTask.mockResolvedValue({
    task: { ...pendingTask, state: "active" },
    replayed: false,
    run: { run: { id: "00000000-0000-4000-8000-000000000020", state: "running" } },
    execution: { state: "awaiting_evaluation", detail: "Objective evaluation is still required." }
  });
  client.taskEvents.mockResolvedValue({
    events: [
      { sequence: 1, kind: "task_created", occurred_at: pendingTask.created_at },
      { sequence: 2, kind: "task_transitioned", occurred_at: pendingTask.created_at },
      {
        sequence: 3,
        kind: "evaluation_recorded",
        occurred_at: pendingTask.created_at,
        payload: {
          outcome: "passed",
          conditions: [
            { passed: true, detail: "npm test exited with code 0" },
            { passed: true, detail: "npm run build exited with code 0" }
          ],
          evidence: [{ method: "governed_process_exit", detail: "exit code 0" }]
        }
      }
    ]
  });
  client.taskAudit.mockResolvedValue({
    records: [{
      schema_version: 1,
      execution_id: "00000000-0000-4000-8000-000000000060",
      request_id: "00000000-0000-4000-8000-000000000061",
      task_id: pendingTask.id,
      tool: "browser.workflow",
      resource: "http://127.0.0.1:3000",
      request_fingerprint: "fixture",
      phase: "completion",
      decision: "allow",
      result: "succeeded",
      artifacts: [{
        id: "00000000-0000-4000-8000-000000000062",
        sha256: "a".repeat(64),
        media_type: "image/png",
        size_bytes: 2048,
        relative_path: "aa/fixture"
      }],
      verification: [{ method: "browser.workflow_step", passed: true, detail: "step=1; matched" }],
      recorded_at: pendingTask.created_at
    }]
  });
  client.replayRun.mockResolvedValue({
    schema_version: 1,
    run_id: "00000000-0000-4000-8000-000000000020",
    task_id: pendingTask.id,
    organization_revision_id: "00000000-0000-4000-8000-000000000021",
    terminal: false,
    side_effect_free: true,
    evidence_sha256: "b".repeat(64),
    counts: { events: 3, tool_audit_records: 1, work_items: 1, instances: 1, leases: 1, usage_records: 1, evaluations: 1, turn_results: 1 },
    invariants: [
      { code: "identity_coherent", passed: true, detail: "identities agree" },
      { code: "observation_only", passed: true, detail: "no execution" }
    ],
    evidence: {}
  });
  render(<App />);

  await screen.findByRole("option", { name: "sample" });
  fireEvent.change(screen.getByLabelText("Project"), {
    target: { value: project.project_id }
  });
  fireEvent.change(screen.getByLabelText("Goal"), { target: { value: pendingTask.goal } });
  fireEvent.click(screen.getByRole("button", { name: "Create task" }));
  expect(await screen.findByRole("heading", { name: pendingTask.goal })).toBeInTheDocument();
  expect(client.createTask).toHaveBeenCalledWith({
    goal: pendingTask.goal,
    project_id: project.project_id,
    constraints: [],
    success_conditions: [],
    risk: undefined,
    budget: undefined
  });

  fireEvent.click(screen.getByRole("button", { name: "Start task" }));
  expect(await screen.findByText("active")).toBeInTheDocument();
  expect(client.launchTask).toHaveBeenCalledWith(pendingTask.id);
  expect(screen.getByText("Objective evaluation is still required.")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Evidence" }));
  expect(await screen.findByText("passed")).toBeInTheDocument();
  expect(screen.getByText("2 / 2")).toBeInTheDocument();
  expect(screen.getAllByText("1", { selector: ".evidence-overview strong" })).toHaveLength(2);
  expect(client.taskEvents).toHaveBeenCalledWith(pendingTask.id);
  expect(client.taskAudit).toHaveBeenCalledWith(pendingTask.id);
  fireEvent.click(screen.getByRole("button", { name: "Verify replay" }));
  expect(await screen.findByText("Durable replay verified")).toBeInTheDocument();
  expect(screen.getByText("No provider or tool side effects")).toBeInTheDocument();
  expect(screen.getByTitle("b".repeat(64))).toBeInTheDocument();
  expect(client.replayRun).toHaveBeenCalledWith("00000000-0000-4000-8000-000000000020");

  await enableAdvancedMode();
  expect(screen.getByText("task transitioned")).toBeInTheDocument();
  expect(screen.getByText("Governed tool activity")).toBeInTheDocument();
  expect(screen.getByText("browser.workflow")).toBeInTheDocument();
  expect(screen.getByText("image/png · 2.0 KiB")).toBeInTheDocument();
  expect(screen.getByTitle("a".repeat(64))).toBeInTheDocument();
});

test("blocked task retry is explicit and reports durable progress", async () => {
  const blockedTask = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000030",
    project_id: "00000000-0000-4000-8000-000000000031",
    goal: "Retry the governed build",
    constraints: [],
    success_conditions: [],
    risk: "medium",
    budget: {},
    state: "blocked",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.listTasks.mockResolvedValue({ tasks: [blockedTask] });
  client.retryTask.mockResolvedValue({
    task: { ...blockedTask, state: "active" },
    replayed: false,
    run: { run: { id: "00000000-0000-4000-8000-000000000032", state: "running" } },
    execution: { state: "awaiting_evaluation", detail: "Objective checks are next." }
  });
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
  expect(client.retryTask).toHaveBeenCalledWith(blockedTask.id);
  expect(await screen.findByText("Objective checks are next.")).toBeInTheDocument();
  expect(screen.getByText("active")).toBeInTheDocument();
});

test("pending approval can be reviewed and resumed from the task card", async () => {
  const waitingTask = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000040",
    project_id: "00000000-0000-4000-8000-000000000041",
    goal: "Write the approved file",
    constraints: [],
    success_conditions: [],
    risk: "medium",
    budget: {},
    state: "waiting_for_approval",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  const approval = {
    id: "00000000-0000-4000-8000-000000000042",
    task_id: waitingTask.id,
    action: "fs.write",
    resource: "C:\\Projects\\sample\\approved.txt",
    reason: "Writing a new project file requires approval in guided mode.",
    state: "pending"
  };
  client.listTasks.mockResolvedValue({ tasks: [waitingTask] });
  client.listApprovals.mockResolvedValue({ approvals: [approval] });
  client.approveRequest.mockResolvedValue({ ...approval, state: "approved" });
  client.resumeTask.mockResolvedValue({
    task: { ...waitingTask, state: "active" },
    replayed: false,
    run: { run: { id: "00000000-0000-4000-8000-000000000043", state: "running" } },
    execution: { state: "awaiting_evaluation", detail: "Approved work finished; checks are next." }
  });
  render(<App />);

  expect(await screen.findByText(approval.reason)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Approve and continue" }));
  expect(client.approveRequest).toHaveBeenCalledWith(approval.id);
  await waitFor(() => expect(client.resumeTask).toHaveBeenCalledWith(waitingTask.id));
  expect(await screen.findByText("Approved work finished; checks are next.")).toBeInTheDocument();
});

test("pending approval can be denied into a retryable blocked task", async () => {
  const waitingTask = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000050",
    project_id: "00000000-0000-4000-8000-000000000051",
    goal: "Do not write without permission",
    constraints: [],
    success_conditions: [],
    risk: "medium",
    budget: {},
    state: "waiting_for_approval",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  const approval = {
    id: "00000000-0000-4000-8000-000000000052",
    task_id: waitingTask.id,
    action: "fs.write",
    resource: "C:\\Projects\\sample\\denied.txt",
    reason: "Writing a project file requires approval.",
    state: "pending"
  };
  client.listTasks
    .mockResolvedValueOnce({ tasks: [waitingTask] })
    .mockResolvedValueOnce({ tasks: [{ ...waitingTask, state: "blocked" }] });
  client.listApprovals.mockResolvedValue({ approvals: [approval] });
  client.denyRequest.mockResolvedValue({ ...approval, state: "denied" });
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Deny" }));
  expect(client.denyRequest).toHaveBeenCalledWith(approval.id);
  expect(await screen.findByText("blocked")).toBeInTheDocument();
  expect(screen.getByText("Request denied; task is blocked and can be retried.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
});

test("native project selection discovers the folder and associates the new task", async () => {
  const root = "C:\\Projects\\sample";
  const project = {
    id: "00000000-0000-4000-8000-000000000010",
    project_id: "00000000-0000-4000-8000-000000000011",
    root,
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  client.pickProject.mockResolvedValue(root);
  client.discoverProject.mockResolvedValue({ project });
  client.createTask.mockResolvedValue({
    task: {
      schema_version: 1,
      id: "00000000-0000-4000-8000-000000000012",
      project_id: project.project_id,
      goal: "Improve the sample",
      constraints: [],
      success_conditions: [],
      risk: "medium",
      budget: {},
      state: "pending",
      created_at: project.observed_at,
      updated_at: project.observed_at
    },
    event: { sequence: 1, kind: "task_created", occurred_at: project.observed_at }
  });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
  expect((await screen.findAllByText("sample")).length).toBeGreaterThan(0);
  expect(screen.getByText(root)).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Goal"), { target: { value: "Improve the sample" } });
  fireEvent.click(screen.getByRole("button", { name: "Create task" }));

  expect(await screen.findByRole("heading", { name: "Improve the sample" })).toBeInTheDocument();
  expect(client.discoverProject).toHaveBeenCalledWith(root);
  expect(client.createTask).toHaveBeenCalledWith(expect.objectContaining({
    project_id: project.project_id
  }));
});

test("rediscovering a known Windows folder preserves its durable project identity", async () => {
  const selectedRoot = "C:\\Projects\\sample";
  const project = {
    id: "00000000-0000-4000-8000-000000000020",
    project_id: "00000000-0000-4000-8000-000000000021",
    root: "\\\\?\\C:\\Projects\\sample",
    observed_at: "2026-08-20T00:00:00Z",
    repository: { is_git: false, branch: null, dirty: false },
    languages: [], build_systems: [], test_systems: [], instructions: { conflicts: [] }
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.pickProject.mockResolvedValue(selectedRoot);
  client.discoverProject.mockResolvedValue({ project: { ...project, root: selectedRoot } });
  render(<App />);

  fireEvent.click(screen.getByRole("link", { name: "Projects" }));
  fireEvent.click(await screen.findByRole("button", { name: "Add project folder" }));

  await waitFor(() => expect(client.discoverProject).toHaveBeenCalledWith(selectedRoot, project.project_id));
});

test("desktop project memory is simple to add and advanced lifecycle evidence stays visible", async () => {
  const project = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000510",
    project_id: "00000000-0000-4000-8000-000000000511",
    root: "C:\\Projects\\remembered",
    fingerprint: "f".repeat(64),
    observed_at: "2026-08-18T00:00:00Z",
    repository: { is_git: true, branch: "main", head: "abc", dirty: false, recent_commits: [] },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    package_managers: ["pnpm"],
    services: ["web"],
    ci_systems: [],
    important_directories: ["src"],
    commands: [{ name: "test", command: "pnpm test", executable: "pnpm", arguments: ["test"], source: "package.json", confidence_basis: "manifest" }],
    instructions: { conflicts: [] },
    world: {
      nodes: [{ id: "service:web", kind: "service", label: "web", source: "package.json" }],
      relationships: [{ from: "repository", relation: "provides", to: "service:web", source: "package.json" }]
    },
    watched_inputs: {}
  };
  const claim = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000512",
    project_id: project.project_id,
    namespace: "project",
    key: "package_manager",
    value: "pnpm",
    state: "new",
    confidence: 0.8,
    provenance: { source_kind: "user", source: "Desktop owner" },
    observed_at: "2026-08-18T00:01:00Z",
    correction_count: 0,
    usefulness_count: 0
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.listProjectMemory
    .mockResolvedValueOnce({ claims: [] })
    .mockResolvedValue({ claims: [claim] });
  client.loadManagedProjectMemory.mockResolvedValue({ markdown: "# Managed Project Memory\n\n- package_manager: pnpm" });
  client.createProjectMemory.mockResolvedValue({ claim, event: { sequence: 1, kind: "memory_claim_recorded", occurred_at: claim.observed_at } });
  client.transitionProjectMemory.mockResolvedValue({ claim: { ...claim, state: "working", usefulness_count: 1 }, previous: claim, event: { sequence: 2, kind: "memory_claim_state_changed", occurred_at: claim.observed_at } });
  client.correctProjectMemory.mockResolvedValue({ claim: { ...claim, id: "00000000-0000-4000-8000-000000000513", value: "pnpm 10", state: "working" }, previous: claim, event: { sequence: 3, kind: "memory_claim_corrected", occurred_at: claim.observed_at } });
  client.loadProjectMemoryHistory.mockResolvedValue({ claims: [claim] });
  client.projectTopologyPlan.mockResolvedValue({
    schema_version: 2,
    sha256: "a".repeat(64),
    project_id: project.project_id,
    project_snapshot_id: project.id,
    status: "ready",
    stages: [
      { index: 0, nodes: [{ id: "database:main", kind: "database", label: "main database", source: "compose.yaml" }] },
      { index: 1, nodes: [{ id: "service:web", kind: "service", label: "web", source: "compose.yaml" }] }
    ],
    unresolved_nodes: []
  });

  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Use project" }));
  const intelligence = await screen.findByLabelText("Project intelligence and memory");
  expect(within(intelligence).getByRole("list", { name: "Project topology plan" })).toHaveTextContent("Stage 1main databaseStage 2web");
  expect(within(intelligence).getByRole("list", { name: "Project topology plan" })).not.toHaveTextContent("compose.yaml");
  fireEvent.change(within(intelligence).getByLabelText("Memory fact name"), { target: { value: "package_manager" } });
  fireEvent.change(within(intelligence).getByLabelText("Memory value"), { target: { value: "pnpm" } });
  fireEvent.click(within(intelligence).getByRole("button", { name: "Remember this" }));
  await waitFor(() => expect(client.createProjectMemory).toHaveBeenCalledWith(project.project_id, expect.objectContaining({
    namespace: "project",
    key: "package_manager",
    value: "pnpm",
    confidence: 0.8,
    provenance: { source_kind: "user", source: "Desktop owner" }
  })));
  expect(await within(intelligence).findByText("pnpm")).toBeInTheDocument();

  await enableAdvancedMode();
  expect(within(intelligence).getByRole("list", { name: "Project topology plan" })).toHaveTextContent("database · compose.yaml");
  fireEvent.change(within(intelligence).getByLabelText("Memory fact name"), { target: { value: "temporary_endpoint" } });
  fireEvent.change(within(intelligence).getByLabelText("Memory value"), { target: { value: "http://127.0.0.1:4173" } });
  fireEvent.change(within(intelligence).getByLabelText("Memory fresh until"), { target: { value: "2026-08-20T12:00" } });
  fireEvent.click(within(intelligence).getByRole("button", { name: "Remember this" }));
  await waitFor(() => expect(client.createProjectMemory).toHaveBeenLastCalledWith(project.project_id, expect.objectContaining({
    key: "temporary_endpoint",
    fresh_until: new Date("2026-08-20T12:00").toISOString()
  })));
  expect(within(intelligence).getByRole("list", { name: "Project world model" })).toHaveTextContent("service web");
  expect(within(intelligence).getByRole("list", { name: "Project world model" })).toHaveTextContent("repository → provides → web");
  fireEvent.change(within(intelligence).getByLabelText("Correction for package_manager"), { target: { value: "pnpm 10" } });
  fireEvent.click(within(intelligence).getByRole("button", { name: "Correct" }));
  await waitFor(() => expect(client.correctProjectMemory).toHaveBeenCalledWith(project.project_id, claim.id, expect.objectContaining({ value: "pnpm 10" })));
  fireEvent.click(within(intelligence).getByRole("button", { name: "Reinforce" }));
  await waitFor(() => expect(client.transitionProjectMemory).toHaveBeenCalledWith(project.project_id, claim.id, "reinforce"));
  fireEvent.click(within(intelligence).getByRole("button", { name: "History" }));
  await waitFor(() => expect(intelligence.querySelector(".memory-history")).toHaveTextContent("new"));
});

test("simple mode separately reviews and policy-runs a snapshot-pinned topology command", async () => {
  const project = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000530",
    project_id: "00000000-0000-4000-8000-000000000531",
    root: "C:\\Projects\\reviewed",
    fingerprint: "f".repeat(64),
    observed_at: "2026-08-20T12:00:00Z",
    repository: { is_git: true, branch: "main", head: "abc", dirty: false, recent_commits: [] },
    languages: ["Rust"], build_systems: ["Cargo"], test_systems: ["cargo test"],
    package_managers: ["Cargo"], services: ["api"], ci_systems: [], important_directories: ["src"],
    commands: [{ name: "test", command: "cargo test", executable: "cargo", arguments: ["test"], source: "Cargo.toml", confidence_basis: "declared project manifest" }],
    instructions: { conflicts: [] },
    world: { nodes: [{ id: "service:api", kind: "service", label: "api", source: "compose.yaml" }], relationships: [] },
    watched_inputs: {}
  };
  const topology = {
    schema_version: 2,
    sha256: "b".repeat(64),
    project_id: project.project_id,
    project_snapshot_id: project.id,
    status: "ready",
    stages: [{ index: 0, nodes: [{ id: "service:api", kind: "service", label: "api", source: "compose.yaml" }] }],
    unresolved_nodes: []
  };
  const reviewed = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000532",
    definition_hash: "c".repeat(64),
    project_id: project.project_id,
    project_snapshot_id: project.id,
    project_fingerprint: project.fingerprint,
    topology_sha256: topology.sha256,
    display_name: "test · reviewed",
    reason: "Verify the reviewed project",
    state: "reviewed",
    steps: [{
      id: "00000000-0000-4000-8000-000000000533", stage_index: 0, name: "Run test",
      command_name: "test", command: "cargo test", executable: "cargo", arguments: ["test"],
      source: "Cargo.toml", timeout_seconds: 1800, state: "pending", detail: "", log_artifact_sha256: []
    }],
    created_at: "2026-08-20T12:01:00Z",
    updated_at: "2026-08-20T12:01:00Z"
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.projectTopologyPlan.mockResolvedValue(topology);
  client.createReviewedTopologyPlan.mockImplementation(async (_projectId, request) => ({
    plan: { ...reviewed, id: request.id, steps: [{ ...reviewed.steps[0], id: request.steps[0].id }], reason: request.reason },
    replayed: false, events: [], outcomes: []
  }));
  let runCount = 0;
  client.runReviewedTopologyPlan.mockImplementation(async (_projectId, planId) => {
    runCount += 1;
    return runCount === 1
      ? { plan: { ...reviewed, id: planId, state: "waiting" }, replayed: false, events: [], approval_id: "00000000-0000-4000-8000-000000000534", outcomes: [] }
      : { plan: { ...reviewed, id: planId, state: "succeeded", steps: [{ ...reviewed.steps[0], state: "succeeded", detail: "process completed" }] }, replayed: false, events: [], outcomes: [] };
  });
  client.approveRequest.mockResolvedValue({});

  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Use project" }));
  const intelligence = await screen.findByLabelText("Project intelligence and memory");
  await enableAdvancedMode();
  const authority = within(intelligence).getByLabelText("Reviewed topology authority");
  expect(within(authority).getByText("Snapshot")).toBeInTheDocument();
  expect(within(authority).getByText("Topology SHA-256")).toBeInTheDocument();
  expect(within(authority).getByText("Retry identity")).toBeInTheDocument();
  fireEvent.change(within(intelligence).getByLabelText("Reviewed topology reason"), { target: { value: "Verify the reviewed project" } });
  fireEvent.click(within(intelligence).getByRole("button", { name: "Review this plan" }));
  await waitFor(() => expect(client.createReviewedTopologyPlan).toHaveBeenCalledWith(project.project_id, expect.objectContaining({
    project_snapshot_id: project.id,
    topology_sha256: topology.sha256,
    reason: "Verify the reviewed project",
    steps: [expect.objectContaining({ command_name: "test", command: "cargo test", stage_index: 0 })]
  })));
  const planAuthority = await within(intelligence).findByLabelText("Authority for test · reviewed");
  expect(within(planAuthority).getByText("Plan identity")).toBeInTheDocument();
  expect(within(planAuthority).getByText("Definition SHA-256")).toBeInTheDocument();
  fireEvent.click(await within(intelligence).findByRole("button", { name: "Run reviewed plan" }));
  fireEvent.click(await within(intelligence).findByRole("button", { name: "Approve and run" }));
  await waitFor(() => expect(client.approveRequest).toHaveBeenCalledWith("00000000-0000-4000-8000-000000000534"));
  expect(await within(intelligence).findByText("succeeded")).toBeInTheDocument();
});

test("simple mode reviews and transfers one durable fact to an explicitly chosen project", async () => {
  const source = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000520",
    project_id: "00000000-0000-4000-8000-000000000521",
    root: "C:\\Projects\\source",
    fingerprint: "a".repeat(64),
    observed_at: "2026-08-20T12:00:00Z",
    repository: { is_git: true, branch: "main", head: "abc", dirty: false, recent_commits: [] },
    languages: ["TypeScript"], build_systems: ["npm"], test_systems: ["vitest"],
    package_managers: ["pnpm"], services: [], ci_systems: [], important_directories: ["src"],
    commands: [], instructions: { conflicts: [] }, world: { nodes: [], relationships: [] }, watched_inputs: {}
  };
  const target = { ...source, id: "00000000-0000-4000-8000-000000000522", project_id: "00000000-0000-4000-8000-000000000523", root: "C:\\Projects\\target", fingerprint: "b".repeat(64) };
  const claim = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000524",
    project_id: source.project_id,
    namespace: "project",
    key: "package_manager",
    value: "pnpm",
    state: "durable",
    confidence: 0.95,
    provenance: { source_kind: "user", source: "Desktop owner" },
    observed_at: "2026-08-20T12:01:00Z",
    fresh_until: "2026-08-27T12:01:00Z",
    correction_count: 0,
    usefulness_count: 2
  };
  const digest = "d".repeat(64);
  client.listProjects.mockResolvedValue({ projects: [source, target] });
  client.listProjectMemory.mockResolvedValue({ claims: [claim] });
  client.reviewProjectMemoryTransfer.mockResolvedValue({ claim, source_claim_sha256: digest });
  client.transferProjectMemory.mockResolvedValue({
    transfer: {
      schema_version: 1,
      id: "00000000-0000-4000-8000-000000000525",
      source_project_id: source.project_id,
      source_claim_id: claim.id,
      source_claim_sha256: digest,
      target_project_id: target.project_id,
      target_claim_id: "00000000-0000-4000-8000-000000000526",
      reason: "Both projects use the same package manager",
      created_at: "2026-08-20T12:02:00Z"
    },
    claim: { ...claim, id: "00000000-0000-4000-8000-000000000526", project_id: target.project_id, state: "new" },
    replayed: false,
    events: [{ sequence: 1, kind: "memory_claim_transferred", occurred_at: "2026-08-20T12:02:00Z" }]
  });

  render(<App />);
  fireEvent.click((await screen.findAllByRole("button", { name: "Use project" }))[0]!);
  const intelligence = await screen.findByLabelText("Project intelligence and memory");
  fireEvent.click(await within(intelligence).findByRole("button", { name: "Share with another project" }));
  await waitFor(() => expect(client.reviewProjectMemoryTransfer).toHaveBeenCalledWith(source.project_id, claim.id));
  fireEvent.change(within(intelligence).getByLabelText("Memory transfer target project"), { target: { value: target.project_id } });
  fireEvent.change(within(intelligence).getByLabelText("Memory transfer reason"), { target: { value: "Both projects use the same package manager" } });
  fireEvent.click(within(intelligence).getByRole("button", { name: "Copy reviewed fact" }));
  await waitFor(() => expect(client.transferProjectMemory).toHaveBeenCalledWith(source.project_id, {
    transfer_id: expect.any(String),
    source_claim_id: claim.id,
    source_claim_sha256: digest,
    target_project_id: target.project_id,
    reason: "Both projects use the same package manager"
  }));
});

test("project discovery errors remain visible on the Projects screen", async () => {
  const root = "C:\\Projects\\broken";
  client.pickProject.mockResolvedValue(root);
  client.discoverProject.mockRejectedValue(new Error("Project discovery failed (405 Method Not Allowed)"));
  render(<App />);

  fireEvent.click(screen.getByRole("link", { name: "Projects" }));
  fireEvent.click(screen.getByRole("button", { name: "Add project folder" }));

  expect(await screen.findByRole("alert")).toHaveTextContent("Project discovery failed (405 Method Not Allowed)");
});

test("task evidence uses the durable blocked state when no evaluation exists", async () => {
  const blockedTask = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000099",
    project_id: null,
    goal: "Blocked fixture",
    constraints: [],
    success_conditions: [],
    risk: "medium",
    budget: {},
    state: "blocked",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:01Z"
  };
  client.listTasks.mockResolvedValue({ tasks: [blockedTask] });
  client.taskEvents.mockResolvedValue({
    events: [{ sequence: 1, kind: "task_created", occurred_at: blockedTask.created_at }]
  });
  client.taskAudit.mockResolvedValue({ records: [] });
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Evidence" }));

  expect(within(await screen.findByRole("region", { name: "Task evidence" })).getByText("Blocked")).toBeInTheDocument();
});

test("plugin access is scoped before the owner can enable an extension", async () => {
  const installation = {
    schema_version: 1,
    plugin_id: "example.weather",
    active_revision_id: "00000000-0000-4000-8000-000000000081",
    enabled: false,
    capability_grants: [],
    revision: 1,
    updated_at: "2026-08-15T00:00:00Z"
  };
  const descriptor = {
    installation,
    active_revision: {
      schema_version: 1,
      id: installation.active_revision_id,
      manifest: {
        schema_version: 1,
        id: installation.plugin_id,
        display_name: "Weather helper",
        version: "1.0.0",
        class: "sandboxed_wasi",
        requested_capabilities: ["network"]
      },
      module_sha256: "a".repeat(64),
      module_size_bytes: 128,
      source: { kind: "local_import" },
      signature: { state: "unsigned" },
      installed_at: installation.updated_at
    }
  };
  const grant = {
    schema_version: 1,
    capability: "network",
    scopes: [{ kind: "network_origin", origin: "https://weather.example" }]
  };
  const granted = { ...descriptor, installation: { ...installation, revision: 2, capability_grants: [grant] } };
  client.listPlugins.mockResolvedValueOnce({ plugins: [descriptor] }).mockResolvedValue({ plugins: [granted] });
  client.setPluginGrants.mockResolvedValue(granted.installation);
  client.setPluginEnabled.mockResolvedValue({ ...granted.installation, enabled: true, revision: 3 });
  render(<App />);

  expect(await screen.findByRole("heading", { name: "Weather helper" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Enable" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Weather helper network origin"), { target: { value: "https://weather.example" } });
  fireEvent.click(screen.getByRole("button", { name: "Grant requested access" }));

  await waitFor(() => expect(client.setPluginGrants).toHaveBeenCalledWith("example.weather", {
    expected_revision: 1,
    grants: [grant]
  }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Enable" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Enable" }));
  await waitFor(() => expect(client.setPluginEnabled).toHaveBeenCalledWith("example.weather", 2, true));
});
