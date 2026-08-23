import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const client = vi.hoisted(() => ({
  start: vi.fn(),
  complete: vi.fn(),
  cancel: vi.fn(),
  open: vi.fn(),
  openInApp: vi.fn(),
  updateInApp: vi.fn(),
  closeInApp: vi.fn(),
  controlInApp: vi.fn(),
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
  listConversations: vi.fn(),
  createConversation: vi.fn(),
  updateConversation: vi.fn(),
  archiveConversation: vi.fn(),
  getConversation: vi.fn(),
  sendConversationMessage: vi.fn(),
  steerConversationTurn: vi.fn(),
  getCodexModels: vi.fn(),
  getCodexAccount: vi.fn(),
  getCodexRateLimits: vi.fn(),
  getCodexHarnessStatus: vi.fn(),
  getCodexConfigurationStatus: vi.fn(),
  getCodexPendingRequests: vi.fn(),
  getCodexStableDiscovery: vi.fn(),
  reconcileOwnedCodexThread: vi.fn(),
  forkOwnedCodexThread: vi.fn(),
  startOwnedCodexReview: vi.fn(),
  watchConversationProject: vi.fn(),
  unwatchConversationProject: vi.fn(),
  startCodexLogin: vi.fn(),
  cancelCodexLogin: vi.fn(),
  logoutCodexAccount: vi.fn(),
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
  getGovernorOverview: vi.fn(),
  listAgentRuns: vi.fn(),
  setGlobalOrganizationAssignment: vi.fn(),
  setProjectOrganizationAssignment: vi.fn(),
  previewOrganizationRevision: vi.fn(),
  publishOrganizationRevision: vi.fn(),
  cancelAgentRun: vi.fn(),
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
  listCodexSites: vi.fn(),
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
  ,listOpenInApplications: vi.fn()
  ,openWorkspaceIn: vi.fn()
  ,runTerminalCommand: vi.fn()
}));

vi.mock("./desktop-client", () => ({
  startOpenAiOAuth: client.start,
  completeOpenAiOAuth: client.complete,
  cancelOpenAiOAuth: client.cancel,
  openExternalUrl: client.open,
  openSiteInAppBrowser: client.openInApp,
  updateSiteInAppBrowser: client.updateInApp,
  closeSiteInAppBrowser: client.closeInApp,
  controlSiteInAppBrowser: client.controlInApp,
  openSiteExternalUrl: client.open,
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
  listConversations: client.listConversations,
  createConversation: client.createConversation,
  updateConversation: client.updateConversation,
  archiveConversation: client.archiveConversation,
  getConversation: client.getConversation,
  sendConversationMessage: client.sendConversationMessage,
  steerConversationTurn: client.steerConversationTurn,
  getCodexModels: client.getCodexModels,
  getCodexAccount: client.getCodexAccount,
  getCodexRateLimits: client.getCodexRateLimits,
  getCodexHarnessStatus: client.getCodexHarnessStatus,
  getCodexConfigurationStatus: client.getCodexConfigurationStatus,
  getCodexPendingRequests: client.getCodexPendingRequests,
  getCodexStableDiscovery: client.getCodexStableDiscovery,
  reconcileOwnedCodexThread: client.reconcileOwnedCodexThread,
  forkOwnedCodexThread: client.forkOwnedCodexThread,
  startOwnedCodexReview: client.startOwnedCodexReview,
  watchConversationProject: client.watchConversationProject,
  unwatchConversationProject: client.unwatchConversationProject,
  startCodexLogin: client.startCodexLogin,
  cancelCodexLogin: client.cancelCodexLogin,
  logoutCodexAccount: client.logoutCodexAccount,
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
  getGovernorOverview: client.getGovernorOverview,
  listAgentRuns: client.listAgentRuns,
  setGlobalOrganizationAssignment: client.setGlobalOrganizationAssignment,
  setProjectOrganizationAssignment: client.setProjectOrganizationAssignment,
  previewOrganizationRevision: client.previewOrganizationRevision,
  publishOrganizationRevision: client.publishOrganizationRevision,
  cancelAgentRun: client.cancelAgentRun,
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
  listCodexSites: client.listCodexSites,
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
  getLocalServiceStatus: client.localServiceStatus,
  listOpenInApplications: client.listOpenInApplications,
  openWorkspaceIn: client.openWorkspaceIn,
  runTerminalCommand: client.runTerminalCommand
}));

import { App, isChatViewportNearBottom } from "./App";

beforeEach(() => {
  window.history.replaceState(null, "", "/");
  window.localStorage.clear();
  client.localServiceStatus.mockReset();
  client.start.mockReset();
  client.complete.mockReset();
  client.cancel.mockReset();
  client.open.mockReset();
  client.openInApp.mockReset();
  client.updateInApp.mockReset();
  client.closeInApp.mockReset();
  client.controlInApp.mockReset();
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
  client.listConversations.mockReset();
  client.createConversation.mockReset();
  client.updateConversation.mockReset();
  client.archiveConversation.mockReset();
  client.getConversation.mockReset();
  client.sendConversationMessage.mockReset();
  client.steerConversationTurn.mockReset();
  client.getCodexModels.mockReset();
  client.getCodexAccount.mockReset();
  client.getCodexRateLimits.mockReset();
  client.getCodexHarnessStatus.mockReset();
  client.getCodexConfigurationStatus.mockReset();
  client.getCodexPendingRequests.mockReset();
  client.getCodexStableDiscovery.mockReset();
  client.reconcileOwnedCodexThread.mockReset();
  client.forkOwnedCodexThread.mockReset();
  client.startOwnedCodexReview.mockReset();
  client.watchConversationProject.mockReset();
  client.unwatchConversationProject.mockReset();
  client.startCodexLogin.mockReset();
  client.cancelCodexLogin.mockReset();
  client.logoutCodexAccount.mockReset();
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
  client.getGovernorOverview.mockReset();
  client.listAgentRuns.mockReset();
  client.setGlobalOrganizationAssignment.mockReset();
  client.setProjectOrganizationAssignment.mockReset();
  client.previewOrganizationRevision.mockReset();
  client.publishOrganizationRevision.mockReset();
  client.cancelAgentRun.mockReset();
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
  client.listCodexSites.mockReset();
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
  client.listOpenInApplications.mockReset();
  client.openWorkspaceIn.mockReset();
  client.runTerminalCommand.mockReset();
  client.open.mockResolvedValue(undefined);
  client.openInApp.mockResolvedValue(undefined);
  client.updateInApp.mockResolvedValue(undefined);
  client.closeInApp.mockResolvedValue(undefined);
  client.controlInApp.mockResolvedValue(undefined);
  client.listProviders.mockResolvedValue({ providers: [] });
  client.discoverProvider.mockImplementation(async (connectionId: string) => ({
    connection_id: connectionId,
    models: [],
    discovered_at: new Date().toISOString()
  }));
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
  client.listConversations.mockResolvedValue({ conversations: [] });
  client.createConversation.mockImplementation(async (request: { title?: string; project_id?: string; execution_preferences: Record<string, unknown> }) => ({
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000c01",
    title: request.title ?? "New chat",
    project_id: request.project_id,
    state: "active",
    execution_preferences: request.execution_preferences,
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:00Z"
  }));
  client.updateConversation.mockImplementation(async (conversationId: string, request: { title: string; project_id?: string }) => ({
    schema_version: 1,
    id: conversationId,
    title: request.title,
    project_id: request.project_id,
    state: "active",
    execution_preferences: {},
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:01:00Z"
  }));
  client.archiveConversation.mockImplementation(async (conversationId: string, archived: boolean) => ({
    schema_version: 1,
    id: conversationId,
    title: "Archived chat",
    state: archived ? "archived" : "active",
    execution_preferences: {},
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:01:00Z"
  }));
  client.getConversation.mockResolvedValue({
    conversation: {
      schema_version: 1,
      id: "00000000-0000-4000-8000-000000000c01",
      title: "New chat",
      state: "active",
      execution_preferences: {},
      created_at: "2026-08-22T00:00:00Z",
      updated_at: "2026-08-22T00:00:00Z"
    },
    turns: [],
    messages: [],
    activities: []
  });
  client.sendConversationMessage.mockImplementation(async (conversationId: string, request: { content: string; execution_preferences?: Record<string, unknown> }) => {
    const conversation = await client.createConversation({ title: request.content.slice(0, 96), execution_preferences: request.execution_preferences ?? {} });
    return {
      conversation: { ...conversation, id: conversationId },
      turns: [{ schema_version: 1, id: "00000000-0000-4000-8000-000000000c02", conversation_id: conversationId, state: "completed", execution_preferences: request.execution_preferences ?? {}, created_at: "2026-08-22T00:00:00Z", updated_at: "2026-08-22T00:00:01Z" }],
      messages: [
        { schema_version: 1, id: "00000000-0000-4000-8000-000000000c03", conversation_id: conversationId, turn_id: "00000000-0000-4000-8000-000000000c02", role: "user", content: request.content, created_at: "2026-08-22T00:00:00Z" },
        { schema_version: 1, id: "00000000-0000-4000-8000-000000000c04", conversation_id: conversationId, turn_id: "00000000-0000-4000-8000-000000000c02", role: "assistant", content: "Fixture response", created_at: "2026-08-22T00:00:01Z" }
      ],
      activities: []
    };
  });
  client.steerConversationTurn.mockImplementation(async () => client.getConversation());
  client.getCodexModels.mockResolvedValue([]);
  client.getCodexAccount.mockResolvedValue({ authenticated: false, requires_openai_auth: true });
  client.getCodexRateLimits.mockResolvedValue({ refreshed_at: "2026-08-22T00:00:00Z" });
  client.getCodexHarnessStatus.mockResolvedValue({
    bundled_version: "0.144.6",
    protocol_version: "0.144.6",
    schema_sha256: "e710974af2ef773a512e039f7b38b6040f533035465ad899929f4cc52ce7a2a4",
    process_state: "ready",
    initialized: true,
    account_state: "signed_out",
    active_threads: 0,
    pending_approvals: 0,
    pending_requests: 0,
    unsupported_requests: 0,
    restart_count: 0
    ,configuration_state: "ready"
    ,windows_sandbox_state: "ready"
    ,mcp_bridge_state: "ready"
    ,recovery_state: "healthy"
  });
  client.getCodexConfigurationStatus.mockResolvedValue({ requirements: {}, provider_capabilities: {}, windows_sandbox: {} });
  client.getCodexPendingRequests.mockResolvedValue([]);
  client.getCodexStableDiscovery.mockResolvedValue({
    skills: [],
    hooks: [],
    appsAvailable: false,
    appsUnavailableReason: "Installed-app discovery is experimental in bundled Codex 0.144.6",
    refreshedAt: "2026-08-22T00:00:00Z"
  });
  client.reconcileOwnedCodexThread.mockResolvedValue({});
  client.forkOwnedCodexThread.mockResolvedValue({});
  client.startOwnedCodexReview.mockResolvedValue({ reviewThreadId: "review-thread", turn: {}, delivery: "inline" });
  client.watchConversationProject.mockResolvedValue({ watchId: "mcg-conversation-test", path: "C:\\workspace" });
  client.unwatchConversationProject.mockResolvedValue({});
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
  const defaultOrganization = {
    schema_version: 1,
    organization_id: "00000000-0000-4000-8000-000000000901",
    revision_id: "00000000-0000-4000-8000-000000000902",
    revision: 1,
    name: "MCG Consumer Default",
    roles: [],
    relationships: [],
    policy: { prefer_minimal_organization: true, max_parallel_agents: 2, max_total_agents: 8, max_organizational_overhead_ratio: 0.25, default_lease_seconds: 900 },
    evaluation: { require_independent_evaluation: true, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 },
    created_at: "2026-08-21T00:00:00Z"
  };
  const assignments = { global_organization_id: defaultOrganization.organization_id, project_overrides: {}, updated_at: "2026-08-21T00:00:00Z" };
  client.getGovernorOverview.mockResolvedValue({
    status: { service: "mcg-daemon", version: "0.1.2", state: "ready", advanced_mode: false, autonomy: "autonomous", shadow_experiments: true, recovered_events: 0, recovered_tasks: 0, recovered_tool_leases: 0, recovered_agent_runs: 0, recovered_agent_instances: 0, recovered_agent_leases: 0, recovered_schedule_occurrences: 0, recovered_platform_sessions: 0, expired_platform_sessions: 0, recovered_computer_action_batches: 0 },
    autonomy: "autonomous",
    assignments,
    active_organization: defaultOrganization,
    active_runs: 0,
    recent_runs: 0,
    subsystems: []
  });
  client.listAgentRuns.mockResolvedValue({ runs: [] });
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
  client.listCodexSites.mockResolvedValue({ sites: [] });
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
  client.listOpenInApplications.mockResolvedValue([]);
  client.openWorkspaceIn.mockResolvedValue(undefined);
  client.runTerminalCommand.mockResolvedValue({ working_directory: "C:\\Projects\\sample", output: "ok", exit_code: 0 });
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
  const button = await screen.findByRole("button", { name: "Use advanced mode", pressed: false });
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
  await waitFor(() => expect(document.querySelector(".app-shell.advanced-mode")).toBeInTheDocument());
}

async function enableSimpleMode() {
  const button = await screen.findByRole("button", { name: "Use simple mode", pressed: true });
  fireEvent.click(button);
  await waitFor(() => expect(document.querySelector(".app-shell.simple-mode")).toBeInTheDocument());
}

test("workspace toolbar toggles the docked side panel and project terminal", async () => {
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: "Toggle side panel" }));
  expect(screen.getByRole("complementary", { name: "Workspace side panel" })).toBeInTheDocument();
  expect(screen.getByRole("tab", { name: "Browser", selected: true })).toBeInTheDocument();
  expect(window.localStorage.getItem("mcg.workspace-side-panel")).toBe("true");
  const sidePanelResizer = screen.getByRole("separator", { name: "Resize side panel" });
  expect(sidePanelResizer).toHaveAttribute("aria-valuenow", "520");
  fireEvent.pointerDown(sidePanelResizer, { clientX: 500, pointerId: 1 });
  fireEvent.pointerMove(sidePanelResizer, { clientX: 600, pointerId: 1 });
  fireEvent.pointerUp(sidePanelResizer, { pointerId: 1 });
  expect(sidePanelResizer).toHaveAttribute("aria-valuenow", "420");

  fireEvent.keyDown(window, { key: "b", ctrlKey: true, altKey: true });
  expect(screen.queryByRole("complementary", { name: "Workspace side panel" })).not.toBeInTheDocument();

  fireEvent.keyDown(window, { key: "j", ctrlKey: true });
  expect(screen.getByRole("region", { name: "Terminal" })).toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Terminal command" })).toBeInTheDocument();
  expect(window.localStorage.getItem("mcg.workspace-bottom-panel")).toBe("true");
});

test("active governed browser sessions render a live page preview in the side panel", async () => {
  const active = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000099",
    definition_hash: "c".repeat(64),
    kind: "browser",
    state: "active",
    owner: "mcg",
    display_name: "Browser · localhost",
    allowed_origins: ["http://localhost:3000"],
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:00Z",
    expires_at: "2026-08-22T01:00:00Z",
    recovery_count: 0
  };
  client.listPlatformSessions.mockResolvedValue({ sessions: [active] });
  client.navigateBrowserSession.mockResolvedValue({
    session_id: active.id,
    mode: "headless",
    safe_url: "http://localhost:3000/about",
    title: "PrairieOps",
    handed_off: false,
    denied_requests: 0
  });

  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Toggle side panel" }));
  const panel = screen.getByRole("complementary", { name: "Workspace side panel" });
  const preview = await within(panel).findByTitle("Browser preview · Browser · localhost");
  expect(preview).toHaveAttribute("data-preview-url", "http://localhost:3000");
  expect(within(panel).getByRole("status")).toHaveTextContent("Live preview");

  const url = within(panel).getByLabelText("URL for Browser · localhost");
  fireEvent.change(url, { target: { value: "http://localhost:3000/about?private=value" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Go" }));
  expect(await within(panel).findByTitle("Browser preview · PrairieOps")).toHaveAttribute("data-preview-url", "http://localhost:3000/about");
  expect(client.navigateBrowserSession).toHaveBeenCalledWith(active.id, "http://localhost:3000/about?private=value");
});

test("Agents distinguishes the deterministic runtime from live model agents", async () => {
  const organization = {
    schema_version: 1,
    organization_id: "00000000-0000-4000-8000-000000000921",
    revision_id: "00000000-0000-4000-8000-000000000922",
    revision: 3,
    name: "Product team",
    roles: [
      { id: "00000000-0000-4000-8000-000000000923", kind: "executive", name: "Executive", purpose: "Coordinates work", prompt_policy: "Stay bounded", model_policy: { capability_class: "general", preferred_models: [], allow_fallback: true, max_escalation_tier: 2 }, tool_capabilities: [], memory_namespace: "team" },
      { id: "00000000-0000-4000-8000-000000000924", kind: "coder", name: "Builder", purpose: "Implements changes", prompt_policy: "Verify changes", model_policy: { capability_class: "coding", preferred_models: [], allow_fallback: true, max_escalation_tier: 2 }, tool_capabilities: ["filesystem"], memory_namespace: "project" }
    ],
    relationships: [{ from: "00000000-0000-4000-8000-000000000923", to: "00000000-0000-4000-8000-000000000924", kind: "delegates_to" }],
    policy: { prefer_minimal_organization: true, max_parallel_agents: 2, max_total_agents: 4, max_organizational_overhead_ratio: 0.25, default_lease_seconds: 900 },
    evaluation: { require_independent_evaluation: true, require_objective_evidence: true, minimum_confidence: 0.8, calibration_window: 100 },
    created_at: "2026-08-21T00:00:00Z"
  };
  const assignments = { global_organization_id: organization.organization_id, project_overrides: {}, updated_at: "2026-08-21T00:00:00Z" };
  client.listOrganizations.mockResolvedValue({ organizations: [organization] });
  client.getGovernorOverview.mockResolvedValue({ status: { service: "mcg-daemon", version: "0.1.2", state: "ready", advanced_mode: false, autonomy: "reviewed", shadow_experiments: true, recovered_events: 1, recovered_tasks: 0, recovered_tool_leases: 0, recovered_agent_runs: 0, recovered_agent_instances: 0, recovered_agent_leases: 0, recovered_schedule_occurrences: 0, recovered_platform_sessions: 0, expired_platform_sessions: 0, recovered_computer_action_batches: 0 }, autonomy: "reviewed", assignments, active_organization: organization, active_runs: 0, recent_runs: 0, subsystems: [{ id: "policy", label: "Policy enforcement", state: "ready", detail: "Owner policy active" }] });

  render(<App />);
  fireEvent.click(await screen.findByRole("link", { name: /Agents/ }));
  expect(await screen.findByRole("heading", { name: "MCG runtime" })).toBeInTheDocument();
  expect(screen.getByText(/not a model agent/i)).toBeInTheDocument();
  expect(screen.getByText("Product team · revision 3")).toBeInTheDocument();
  expect(screen.getByText("Policy enforcement")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Teams" }));
  expect(screen.getByRole("combobox", { name: "Global default team" })).toHaveValue(organization.organization_id);
  expect(screen.queryByRole("button", { name: "Improvement" })).not.toBeInTheDocument();
});

test("Codex-style sidebar groups project chats, searches them, and persists pins", async () => {
  const project = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000610",
    project_id: "00000000-0000-4000-8000-000000000611",
    root: "C:\\Projects\\consumer-app",
    observed_at: "2026-08-21T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: [],
    build_systems: [],
    test_systems: [],
    instructions: { conflicts: [] }
  };
  const task = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000612",
    project_id: project.project_id,
    goal: "Polish the consumer dashboard",
    constraints: [],
    success_conditions: [],
    risk: "low",
    budget: {},
    state: "active",
    created_at: "2026-08-21T00:00:00Z",
    updated_at: "2026-08-21T00:00:00Z"
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.listTasks.mockResolvedValue({ tasks: [task] });
  render(<App />);

  const projectTree = screen.getByLabelText("Projects");
  expect(await within(projectTree).findByRole("button", { name: "consumer-app" })).toBeInTheDocument();
  expect(within(projectTree).getByRole("button", { name: "New chat in consumer-app" })).toBeInTheDocument();
  expect(within(projectTree).getByRole("button", { name: task.goal })).toBeInTheDocument();

  fireEvent.click(within(projectTree).getByRole("button", { name: task.goal }));
  fireEvent.click(await screen.findByRole("button", { name: "Pin" }));
  const pinned = screen.getByLabelText("Pinned chats");
  expect(within(pinned).getByRole("button", { name: task.goal })).toBeInTheDocument();
  expect(JSON.parse(window.localStorage.getItem("mcg.pinned-chat-ids") ?? "[]")).toEqual([task.id]);

  fireEvent.change(screen.getByRole("textbox", { name: "Search chats" }), { target: { value: "missing" } });
  expect(within(projectTree).queryByRole("button", { name: task.goal })).not.toBeInTheDocument();
  expect(within(pinned).queryByRole("button", { name: task.goal })).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "Search chats" }), { target: { value: "dashboard" } });
  fireEvent.click(within(pinned).getByRole("button", { name: task.goal }));
  expect(screen.getByRole("heading", { name: task.goal, level: 3 })).toBeInTheDocument();
});

test("project and pinned chat menus pin, archive, and expose the expected distinct actions", async () => {
  const project = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000710",
    project_id: "00000000-0000-4000-8000-000000000711",
    root: "C:\\Projects\\menu-app",
    observed_at: "2026-08-22T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: [], build_systems: [], test_systems: [], instructions: { conflicts: [] }
  };
  const conversation = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000712",
    title: "Plan the launch",
    project_id: project.project_id,
    state: "active" as const,
    execution_preferences: {},
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:00Z"
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.listConversations.mockResolvedValue({ conversations: [conversation] });
  client.getConversation.mockResolvedValue({ conversation, turns: [], messages: [], activities: [] });
  client.archiveConversation.mockResolvedValue({ ...conversation, state: "archived", updated_at: "2026-08-22T00:01:00Z" });

  render(<App />);
  const projectTree = screen.getByLabelText("Projects");
  const projectChat = await within(projectTree).findByRole("button", { name: conversation.title });
  fireEvent.contextMenu(projectChat, { clientX: 220, clientY: 200 });

  const projectMenu = screen.getByRole("menu", { name: "Project chat menu" });
  expect(within(projectMenu).getByRole("menuitem", { name: "Pin" })).toBeInTheDocument();
  expect(within(projectMenu).queryByRole("menuitem", { name: "Move up" })).not.toBeInTheDocument();
  fireEvent.click(within(projectMenu).getByRole("menuitem", { name: "Pin" }));

  const pinned = screen.getByLabelText("Pinned chats");
  const pinnedChat = within(pinned).getByRole("button", { name: conversation.title });
  expect(within(pinned).getByRole("button", { name: `Unpin ${conversation.title}` })).toBeInTheDocument();
  fireEvent.contextMenu(pinnedChat, { clientX: 230, clientY: 180 });

  const pinnedMenu = screen.getByRole("menu", { name: "Pinned chat menu" });
  expect(within(pinnedMenu).getByRole("menuitem", { name: "Move up" })).toBeDisabled();
  expect(within(pinnedMenu).getByRole("menuitem", { name: "Unpin" })).toBeInTheDocument();
  fireEvent.click(within(pinnedMenu).getByRole("menuitem", { name: "Archive" }));

  await waitFor(() => expect(client.archiveConversation).toHaveBeenCalledWith(conversation.id, true));
  await waitFor(() => expect(within(projectTree).queryByRole("button", { name: conversation.title })).not.toBeInTheDocument());
  expect(within(pinned).queryByRole("button", { name: conversation.title })).not.toBeInTheDocument();
});

test("sidebar shows recent chats newest first and restores chats from a collapsible archive", async () => {
  const older = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000721",
    title: "Older active chat",
    state: "active" as const,
    execution_preferences: {},
    created_at: "2026-08-20T00:00:00Z",
    updated_at: "2026-08-20T00:00:00Z"
  };
  const newer = {
    ...older,
    id: "00000000-0000-4000-8000-000000000722",
    title: "Newest active chat",
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:00Z"
  };
  const archived = {
    ...older,
    id: "00000000-0000-4000-8000-000000000723",
    title: "Archived launch notes",
    state: "archived" as const,
    updated_at: "2026-08-21T00:00:00Z"
  };
  client.listConversations.mockResolvedValue({ conversations: [older, archived, newer] });
  client.archiveConversation.mockResolvedValue({ ...archived, state: "active", updated_at: "2026-08-22T00:01:00Z" });

  render(<App />);

  const recents = screen.getByLabelText("Recent chats");
  const recentLinks = await within(recents).findAllByRole("button", { name: /active chat \(recent\)/ });
  expect(recentLinks.map((link) => link.textContent).filter(Boolean)).toEqual(["Newest active chat", "Older active chat"]);

  const archive = screen.getByLabelText("Archived chats");
  expect(within(archive).getByRole("button", { name: "Archived" })).toHaveAttribute("aria-expanded", "false");
  expect(within(archive).queryByRole("button", { name: archived.title })).not.toBeInTheDocument();

  fireEvent.click(within(archive).getByRole("button", { name: "Archived" }));
  expect(within(archive).getByRole("button", { name: archived.title })).toBeInTheDocument();
  fireEvent.click(within(archive).getByRole("button", { name: `Restore ${archived.title}` }));

  await waitFor(() => expect(client.archiveConversation).toHaveBeenCalledWith(archived.id, false));
  await waitFor(() => expect(within(archive).queryByRole("button", { name: archived.title })).not.toBeInTheDocument());
});

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

test("simple Sites starts a project-aware website chat without exposing deployment administration", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000111",
    project_id: "00000000-0000-4000-8000-000000000112",
    root: "C:\\Projects\\site",
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  render(<App />);
  const panel = await screen.findByRole("region", { name: "Sites" });
  expect(within(panel).getByRole("heading", { name: "Sites" })).toBeVisible();
  expect(within(panel).getByText("No sites yet")).toBeVisible();
  expect(within(panel).queryByLabelText("Public URL")).not.toBeInTheDocument();
  expect(within(panel).queryByLabelText("Deployment executable")).not.toBeInTheDocument();
  fireEvent.change(within(panel).getByLabelText("Starting point"), { target: { value: project.project_id } });
  fireEvent.click(within(panel).getByRole("button", { name: "New site" }));
  expect(screen.getByLabelText("Project")).toHaveValue(project.project_id);
  expect((screen.getByLabelText("Message") as HTMLTextAreaElement).value).toContain("Build a website");
});

test("opening Sites refreshes AI-published sites from the durable library", async () => {
  const publishedSite = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000119",
    definition_hash: "a".repeat(64),
    revision: 1,
    project_id: "00000000-0000-4000-8000-000000000112",
    display_name: "Springfield IT Support",
    environment: "production" as const,
    public_url: "https://springfield-it.hendo420.chatgpt.site/",
    health_url: "https://springfield-it.hendo420.chatgpt.site/",
    hosting_provider: "OpenAI Sites",
    state: "active" as const,
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:00Z"
  };
  client.listSites
    .mockResolvedValueOnce({ sites: [] })
    .mockResolvedValue({ sites: [publishedSite] });

  render(<App />);
  await waitFor(() => expect(client.listSites).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("link", { name: "Sites" }));

  const panel = await screen.findByRole("region", { name: "Sites" });
  expect(await within(panel).findByRole("heading", { name: "Springfield IT Support" })).toBeVisible();
  expect(within(panel).getByRole("button", { name: "Open in MCG browser" })).toBeVisible();
  fireEvent.click(within(panel).getByRole("button", { name: "More options for Springfield IT Support" }));
  expect(screen.getByRole("menuitem", { name: "Open in MCG browser" })).toBeVisible();
  expect(client.listSites).toHaveBeenCalledTimes(2);
});

test("simple Sites merges the account inventory, renders thumbnails, and offers browser actions", async () => {
  const accountSites = [
    {
      id: "site-1",
      display_name: "Springfield's Best IT",
      description: "Local IT support",
      public_url: "https://springfields-best-it.hendo420.chatgpt.site/",
      screenshot_url: "https://example.test/screens/site-1.png",
      status: "live",
      created_at: "2026-08-22T00:00:00Z",
      updated_at: "2026-08-22T00:00:00Z"
    },
    {
      id: "site-2",
      display_name: "Springfield IT Support",
      description: null,
      public_url: "https://springfield-it-support.hendo420.chatgpt.site/",
      screenshot_url: "https://example.test/screens/site-2.png",
      status: "live",
      created_at: "2026-08-22T00:00:00Z",
      updated_at: "2026-08-22T00:00:00Z"
    },
    {
      id: "site-3",
      display_name: "PrairieOps ITSM",
      description: null,
      public_url: "https://prairieops-itsm.hendo420.chatgpt.site/",
      screenshot_url: null,
      status: "live",
      created_at: "2026-08-22T00:00:00Z",
      updated_at: "2026-08-22T00:00:00Z"
    }
  ];
  client.listCodexSites.mockResolvedValue({ sites: accountSites });

  render(<App />);
  fireEvent.click(screen.getByRole("link", { name: "Sites" }));

  const panel = await screen.findByRole("region", { name: "Sites" });
  expect(await within(panel).findByText("3", { selector: ".count-badge" })).toBeVisible();
  expect(within(panel).getByRole("heading", { name: "Springfield's Best IT" })).toBeVisible();
  expect(within(panel).getByRole("heading", { name: "Springfield IT Support" })).toBeVisible();
  expect(within(panel).getByRole("heading", { name: "PrairieOps ITSM" })).toBeVisible();
  expect(within(panel).getByRole("img", { name: "Preview of Springfield's Best IT" })).toHaveAttribute(
    "src",
    accountSites[0].screenshot_url
  );

  const card = within(panel).getByRole("heading", { name: "Springfield's Best IT" }).closest("article");
  expect(card).not.toBeNull();
  fireEvent.contextMenu(card!);
  fireEvent.click(screen.getByRole("menuitem", { name: "Open in MCG browser" }));
  const browserPanel = await screen.findByRole("region", { name: "Browser" });
  expect(within(browserPanel).getByRole("tab", { name: /Springfield's Best IT/ })).toBeVisible();
  expect(within(browserPanel).getByRole("textbox", { name: "Address" })).toHaveValue(accountSites[0].public_url);
  await waitFor(() => expect(client.openInApp).toHaveBeenCalledWith(
    expect.any(String),
    accountSites[0].public_url,
    expect.objectContaining({ width: expect.any(Number), height: expect.any(Number) })
  ));
  fireEvent.click(within(browserPanel).getByRole("button", { name: "Close Springfield's Best IT" }));
  await waitFor(() => expect(client.closeInApp).toHaveBeenCalledWith(expect.any(String)));
  fireEvent.contextMenu(card!);
  fireEvent.click(screen.getByRole("menuitem", { name: "Open in external browser" }));
  expect(client.open).toHaveBeenCalledWith(accountSites[0].public_url);
});

test("simple Sites makes an account inventory failure visible while preserving local Sites", async () => {
  client.listCodexSites.mockRejectedValue(new Error("Account inventory unavailable"));

  render(<App />);
  fireEvent.click(screen.getByRole("link", { name: "Sites" }));

  const panel = await screen.findByRole("region", { name: "Sites" });
  expect(await within(panel).findByRole("alert")).toHaveTextContent(
    "The OpenAI Sites library could not refresh: Account inventory unavailable"
  );
});

test("simple Pull requests starts a read-only repository review instead of exposing runtime approvals", async () => {
  const project = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000701",
    project_id: "00000000-0000-4000-8000-000000000702",
    root: "C:\\Projects\\review-me",
    fingerprint: "fixture",
    observed_at: "2026-08-21T00:00:00Z",
    repository: { is_git: true, branch: "feature/review", head: "abc123", dirty: true, recent_commits: [] },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    package_managers: ["npm"],
    services: [],
    ci_systems: [],
    important_directories: [],
    commands: [],
    instructions: { conflicts: [] },
    world: { nodes: [], relationships: [] },
    watched_inputs: {}
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  render(<App />);

  const panel = await screen.findByRole("region", { name: "Pull requests" });
  expect(within(panel).getByRole("tab", { name: "All" })).toHaveAttribute("aria-selected", "true");
  expect(within(panel).getByRole("tab", { name: "Reviewing" })).toBeVisible();
  expect(within(panel).getByRole("tab", { name: "Authored" })).toBeVisible();
  expect(within(panel).getByRole("searchbox", { name: "Search pull requests" })).toBeVisible();
  expect(within(panel).getByRole("button", { name: "Filter pull requests" })).toBeVisible();
  expect(within(panel).getByRole("region", { name: "Pull request details" })).toBeVisible();
  expect(within(panel).getByText("Select a review to view")).toBeVisible();
  fireEvent.click(within(panel).getByRole("button", { name: "Start local review" }));
  expect(within(panel).getByText("Review is read-only. It will not modify files.")).toBeVisible();
  expect(within(panel).queryByRole("heading", { name: "Governed approvals" })).not.toBeInTheDocument();
  fireEvent.change(within(panel).getByLabelText("Repository"), { target: { value: project.project_id } });
  fireEvent.change(within(panel).getByLabelText("Review scope"), { target: { value: "uncommitted" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Start review" }));

  expect(screen.getByLabelText("Project")).toHaveValue(project.project_id);
  const goal = (screen.getByLabelText("Message") as HTMLTextAreaElement).value;
  expect(goal).toContain("Review code changes in review-me");
  expect(goal).toContain("all staged, unstaged, and untracked changes");
  expect(goal).toContain("Do not modify the working tree during the review");
});

test("advanced Pull requests separates governed approvals from code review", async () => {
  const waitingTask = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000711",
    goal: "Install a requested dependency",
    constraints: [],
    success_conditions: [],
    risk: "medium",
    budget: {},
    state: "waiting_for_approval",
    created_at: "2026-08-21T00:00:00Z",
    updated_at: "2026-08-21T00:00:00Z"
  };
  client.listTasks.mockResolvedValue({ tasks: [waitingTask] });
  render(<App />);
  await enableAdvancedMode();

  const panel = await screen.findByRole("region", { name: "Pull requests" });
  expect(within(panel).getByRole("heading", { name: "Governed approvals" })).toBeVisible();
  expect(within(panel).getByText("Runtime permission requests are separate from pull-request reviews.")).toBeVisible();
  expect(within(panel).getByText("Install a requested dependency")).toBeVisible();
});

test("advanced Sites registers a selected project and records governed health", async () => {
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
  await enableAdvancedMode();
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

test("simple Scheduled starts a project-aware setup chat and Advanced retains exact scheduling", async () => {
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
    trigger: { kind: "cron", expression: "0 2 * * *" },
    state: "enabled",
    next_run_at: "2026-08-14T01:00:00Z",
    created_at: "2026-08-14T00:00:00Z",
    updated_at: "2026-08-14T00:00:00Z"
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.createSchedule.mockResolvedValue(schedule);
  render(<App />);

  const scheduler = await screen.findByRole("region", { name: "Scheduler" });
  expect(within(scheduler).getByRole("heading", { name: "Scheduled tasks" })).toBeVisible();
  expect(within(scheduler).queryByLabelText("Trigger")).not.toBeInTheDocument();
  fireEvent.change(within(scheduler).getByLabelText("Scheduled project"), { target: { value: project.project_id } });
  fireEvent.change(within(scheduler).getByLabelText("Scheduled work"), { target: { value: "Summarize important changes every morning" } });
  fireEvent.change(within(scheduler).getByLabelText("Each run"), { target: { value: "chat" } });
  fireEvent.click(within(scheduler).getByRole("button", { name: "Set up in chat" }));
  expect(screen.getByLabelText("Project")).toHaveValue(project.project_id);
  expect((screen.getByLabelText("Message") as HTMLTextAreaElement).value).toContain("Summarize important changes every morning");
  expect((screen.getByLabelText("Message") as HTMLTextAreaElement).value).toContain("Return to this setup chat on each run");

  await enableAdvancedMode();
  expect(within(scheduler).getByRole("heading", { name: "Scheduler administration" })).toBeVisible();
  fireEvent.change(within(scheduler).getByLabelText("Trigger"), { target: { value: "cron" } });
  expect(within(scheduler).getByLabelText("Five-field cron")).toHaveValue("0 2 * * *");
  fireEvent.change(within(scheduler).getByLabelText("Name"), { target: { value: schedule.name } });
  fireEvent.change(within(scheduler).getByLabelText("Scheduled goal"), { target: { value: schedule.goal } });
  fireEvent.click(within(scheduler).getByRole("button", { name: "Create schedule" }));

  expect(await within(scheduler).findByRole("heading", { name: schedule.name })).toBeInTheDocument();
  expect(client.createSchedule).toHaveBeenCalledWith(expect.objectContaining({
    schedule_id: expect.any(String),
    project_id: project.project_id,
    trigger: { kind: "cron", expression: "0 2 * * *" },
    budget: {
      max_tokens: 32768,
      max_cost_usd: undefined,
      max_duration_seconds: 3600
    },
    max_model_calls: 16
  }));
});

test("simple Scheduled is an inbox for filtered tasks and unread findings", async () => {
  const base = {
    schema_version: 1,
    definition_hash: "fixture-definition",
    project_id: "00000000-0000-4000-8000-000000000722",
    goal: "Inspect the project and report important findings",
    trigger: { kind: "interval", seconds: 86400 },
    created_at: "2026-08-21T00:00:00Z",
    updated_at: "2026-08-21T00:00:00Z"
  };
  const schedules = [
    { ...base, id: "00000000-0000-4000-8000-000000000723", name: "Daily project brief", state: "enabled", next_run_at: "2026-08-22T08:00:00Z" },
    { ...base, id: "00000000-0000-4000-8000-000000000724", name: "Paused dependency check", state: "paused" },
    { ...base, id: "00000000-0000-4000-8000-000000000725", name: "Finished migration watch", state: "completed" }
  ];
  const notifications = [
    { id: "00000000-0000-4000-8000-000000000726", schedule_id: schedules[0].id, severity: "warning", title: "Review new findings", detail: "The latest run found a regression.", read: false, created_at: "2026-08-21T01:00:00Z" },
    { id: "00000000-0000-4000-8000-000000000727", schedule_id: schedules[1].id, severity: "info", title: "Schedule paused", detail: "The schedule needs input.", read: false, created_at: "2026-08-21T02:00:00Z" }
  ];
  client.listSchedules.mockResolvedValue({ schedules });
  client.listSchedulerNotifications.mockResolvedValue({ notifications });
  client.acknowledgeSchedulerNotification.mockImplementation(async (id) => ({ ...notifications.find((item) => item.id === id)!, read: true }));
  render(<App />);

  const scheduler = await screen.findByRole("region", { name: "Scheduler" });
  expect(within(scheduler).getByRole("heading", { name: "Daily project brief" })).toBeVisible();
  expect(within(scheduler).getByRole("heading", { name: "Needs attention" })).toBeVisible();
  fireEvent.click(within(scheduler).getByRole("button", { name: "Paused" }));
  expect(within(scheduler).queryByRole("heading", { name: "Daily project brief" })).not.toBeInTheDocument();
  expect(within(scheduler).getByRole("heading", { name: "Paused dependency check" })).toBeVisible();
  fireEvent.change(within(scheduler).getByLabelText("Search scheduled tasks"), { target: { value: "missing" } });
  expect(within(scheduler).getByText("No scheduled tasks found")).toBeVisible();
  fireEvent.click(within(scheduler).getByRole("button", { name: "Mark all as read" }));
  await waitFor(() => expect(client.acknowledgeSchedulerNotification).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(within(scheduler).queryByRole("heading", { name: "Needs attention" })).not.toBeInTheDocument());
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
  fireEvent.click(within(panel).getByRole("button", { name: "Reserve session" }));
  expect(await within(panel).findByRole("alert")).toHaveTextContent("Add at least one allowed site");
  expect(client.requestPlatformSession).not.toHaveBeenCalled();
  fireEvent.change(within(panel).getByLabelText("Allowed sites (required)"), { target: { value: "https://example.com" } });
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

  fireEvent.click(await screen.findByRole("button", { name: "Start chat" }));
  const scheduler = screen.getByRole("region", { name: "Scheduler" });
  await enableAdvancedMode();
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

  const advanced = await screen.findByRole("button", { name: "Use advanced mode", pressed: false });
  await waitFor(() => expect(advanced).toBeEnabled());
  advanced.focus();
  expect(advanced).toHaveFocus();
  fireEvent.click(advanced);

  const simple = await screen.findByRole("button", { name: "Use simple mode", pressed: true });
  await waitFor(() => expect(simple).toBeEnabled());
  await waitFor(() => expect(simple).toHaveFocus());
});

test("OpenAI subscription browser login completes automatically after the callback", async () => {
  client.startCodexLogin.mockResolvedValue({ loginId: "fixture-session", authUrl: "https://auth.openai.com/oauth/authorize" });
  render(<App />);

  const providerPanel = document.querySelector("#providers") as HTMLElement;
  expect(within(providerPanel).getByText(/No CLI pairing code\./)).toBeInTheDocument();
  client.getCodexAccount.mockResolvedValue({ authenticated: true, email: "hendo@example.com", plan_type: "plus", requires_openai_auth: true });
  fireEvent.click(within(providerPanel).getByRole("button", { name: "Continue with OpenAI" }));
  await waitFor(() => expect(client.startCodexLogin).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(client.open).toHaveBeenCalledWith("https://auth.openai.com/oauth/authorize"));
  expect(await within(providerPanel).findByText("Using shared Codex account")).toBeInTheDocument();
  expect(client.complete).not.toHaveBeenCalled();
  expect(within(providerPanel).queryByRole("button", { name: "Finish connection" })).not.toBeInTheDocument();
});

test("empty provider catalogs are discovered automatically and populate the chat model selector", async () => {
  const connection = {
    contract_version: 1,
    id: "openai-subscription",
    display_name: "OpenAI Subscription",
    kind: "open_ai",
    base_url: "https://chatgpt.com/backend-api/codex",
    auth_mode: "chat_gpt_browser_oauth"
  };
  client.listProviders.mockResolvedValue({ providers: [{
    connection,
    catalog: { connection_id: connection.id, models: [], discovered_at: "2026-08-22T00:00:00Z" }
  }] });
  client.discoverProvider.mockResolvedValue({
    connection_id: connection.id,
    discovered_at: "2026-08-22T12:00:00Z",
    models: [{ id: "gpt-5.6-sol", display_name: "5.6 Sol", capabilities: { reasoning_control: true } }]
  });
  render(<App />);

  await waitFor(() => expect(client.discoverProvider).toHaveBeenCalledWith(connection.id));
  const selector = document.querySelector('summary[aria-label="New chat model, effort, and speed"]') as HTMLElement;
  fireEvent.click(selector);
  expect(await screen.findByRole("option", { name: "5.6 Sol · OpenAI Subscription" })).toBeInTheDocument();
  expect(screen.getByText("1 available model")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Model"), { target: { value: `${connection.id}\u0000gpt-5.6-sol` } });
  expect(selector).toHaveTextContent("5.6 Sol");
});

test("OpenAI subscription browser login resets after its callback timeout", async () => {
  client.startCodexLogin.mockRejectedValue(
    new Error("Sign-in timed out after 3 minutes. Start sign-in again when you’re ready.")
  );
  render(<App />);

  const providerPanel = document.querySelector("#providers") as HTMLElement;
  fireEvent.click(within(providerPanel).getByRole("button", { name: "Continue with OpenAI" }));
  expect(await within(providerPanel).findByRole("alert")).toHaveTextContent("Sign-in timed out after 3 minutes");
  await waitFor(() => expect(within(providerPanel).getByRole("button", { name: "Continue with OpenAI" })).toBeEnabled());
});

test("account menu stays compact and logs out only ChatGPT OAuth connections", async () => {
  const oauth = {
    contract_version: 1,
    id: "openai-subscription",
    display_name: "OpenAI Subscription",
    kind: "open_ai",
    base_url: "https://chatgpt.com/backend-api/codex",
    auth_mode: "chat_gpt_browser_oauth"
  };
  const api = {
    contract_version: 1,
    id: "openai-api",
    display_name: "OpenAI API",
    kind: "open_ai",
    base_url: "https://api.openai.com/v1",
    auth_mode: "api_key"
  };
  client.listProviders.mockResolvedValue({ providers: [
    { connection: oauth, profile: {
      display_name: "Hendo",
      username: "hendo420",
      email: "hendo@example.com",
      avatar_url: "https://example.com/avatar.png",
      plan_type: "plus",
      account_structure: "personal",
      account_role: "owner",
      usage: {
        primary_window: { used_percent: 24, remaining_percent: 76, window_seconds: 18000, reset_at: 1800000000 },
        secondary_window: { used_percent: 80, remaining_percent: 20, window_seconds: 604800, reset_at: 1800500000 },
        additional_rate_limits: [{
          name: "Codex Spark",
          primary_window: { used_percent: 1, remaining_percent: 99, window_seconds: 18000, reset_at: 1800100000 }
        }]
      }
    }, catalog: { connection_id: oauth.id, models: [], discovered_at: "2026-08-21T00:00:00Z" } },
    { connection: api, catalog: { connection_id: api.id, models: [], discovered_at: "2026-08-21T00:00:00Z" } }
  ] });
  client.disconnectProvider.mockResolvedValue(undefined);
  render(<App />);

  fireEvent.click(await screen.findByRole("button", { name: /Hendo/ }));
  const menu = screen.getByLabelText("Owner menu");
  expect(within(menu).getByText("hendo420")).toBeInTheDocument();
  expect(document.querySelector('img[src="https://example.com/avatar.png"]')).toBeInTheDocument();
  expect(within(menu).getByRole("button", { name: /Usage/ })).toBeInTheDocument();
  expect(within(menu).getByText("76% left")).toBeInTheDocument();
  const expectedResetDate = new Date(1_800_000_000 * 1000).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric"
  });
  expect(within(menu).getByText(`Next Reset: ${expectedResetDate}`)).toBeInTheDocument();
  expect(within(menu).queryByText(/Non-Spark models|Resets/)).not.toBeInTheDocument();
  expect(within(menu).getByRole("progressbar", { name: "Usage remaining" })).toHaveAttribute("aria-valuenow", "76");
  expect(within(menu).queryByText("99% left")).not.toBeInTheDocument();
  expect(within(menu).getByRole("button", { name: /Settings/ })).toBeInTheDocument();
  expect(within(menu).queryByText("Show pet")).not.toBeInTheDocument();
  expect(within(menu).queryByText("Invite a friend")).not.toBeInTheDocument();
  fireEvent.click(within(menu).getByRole("button", { name: /Usage/ }));
  expect((await screen.findAllByText("ChatGPT Plus")).length).toBeGreaterThan(0);
  expect(screen.getByText("76%")).toBeInTheDocument();
  expect(screen.getByText("20%")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /Hendo/ }));
  fireEvent.click(within(menu).getByRole("button", { name: "Log out" }));

  await waitFor(() => expect(client.disconnectProvider).toHaveBeenCalledWith(oauth.id));
  expect(client.disconnectProvider).not.toHaveBeenCalledWith(api.id);
  expect(await screen.findByRole("button", { name: /Local owner/ })).toBeInTheDocument();
});

test("a locally chosen account photo persists for the matching ChatGPT profile", async () => {
  const oauth = {
    contract_version: 1,
    id: "openai-subscription",
    display_name: "OpenAI Subscription",
    kind: "open_ai",
    base_url: "https://chatgpt.com/backend-api/codex",
    auth_mode: "chat_gpt_browser_oauth"
  };
  const dataUrl = "data:image/png;base64,iVBORw0KGgo=";
  window.localStorage.setItem("mcg.account-avatar", JSON.stringify({ accountKey: "person@example.com", dataUrl }));
  client.listProviders.mockResolvedValue({ providers: [
    { connection: oauth, profile: { display_name: "Person", email: "person@example.com" }, catalog: { connection_id: oauth.id, models: [], discovered_at: "2026-08-21T00:00:00Z" } }
  ] });
  render(<App />);

  await screen.findAllByText("person@example.com");
  const identityButton = document.querySelector(".owner-menu-button");
  expect(identityButton).not.toBeNull();
  fireEvent.click(identityButton as HTMLButtonElement);
  expect(document.querySelector(`img[src="${dataUrl}"]`)).toBeInTheDocument();
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(screen.getByRole("button", { name: "Profile" }));
  expect(screen.getByRole("button", { name: "Change photo" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Use OpenAI photo" })).toBeInTheDocument();
});

test("Settings follows the grouped Codex navigation and filters sections", async () => {
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: /Local owner/ }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));

  const navigation = screen.getByRole("navigation", { name: "Settings sections" });
  expect(within(navigation).getByText("Personal")).toBeInTheDocument();
  expect(within(navigation).getByText("Integrations")).toBeInTheDocument();
  expect(within(navigation).getByText("Coding")).toBeInTheDocument();
  const settingsResizer = screen.getByRole("separator", { name: "Resize settings navigation" });
  expect(settingsResizer).toHaveAttribute("aria-valuenow", "244");
  fireEvent.pointerDown(settingsResizer, { clientX: 200, pointerId: 2 });
  fireEvent.pointerMove(settingsResizer, { clientX: 300, pointerId: 2 });
  fireEvent.pointerUp(settingsResizer, { pointerId: 2 });
  expect(settingsResizer).toHaveAttribute("aria-valuenow", "344");
  expect(within(navigation).queryByText("Pets")).not.toBeInTheDocument();
  fireEvent.change(screen.getByRole("textbox", { name: "Search settings" }), { target: { value: "archived" } });
  expect(within(navigation).getByRole("button", { name: "Archived chats" })).toBeInTheDocument();
  expect(within(navigation).queryByRole("button", { name: "General" })).not.toBeInTheDocument();
});

test("integration and coding settings expose sectioned Codex-style controls", async () => {
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: /Local owner/ }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  const settingsNavigation = screen.getByRole("navigation", { name: "Settings sections" });

  fireEvent.click(within(settingsNavigation).getByRole("button", { name: "Plugins" }));
  expect(screen.getByRole("tablist", { name: "Plugin categories" })).toBeInTheDocument();
  expect(screen.getByText("No plugins installed")).toBeInTheDocument();

  fireEvent.click(within(settingsNavigation).getByRole("button", { name: "Browser" }));
  expect(screen.getByText("Browser automation")).toBeInTheDocument();
  expect(screen.getByText("Site permissions", { exact: true })).toBeInTheDocument();

  fireEvent.click(within(settingsNavigation).getByRole("button", { name: "Git" }));
  expect(screen.getByText("Branch prefix")).toBeInTheDocument();
  expect(screen.getByText("codex/")).toBeInTheDocument();
});

test("Appearance applies a durable theme and reduced-motion preference", async () => {
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Appearance" }));

  fireEvent.click(screen.getByRole("button", { name: "Light" }));
  expect(document.documentElement).toHaveAttribute("data-theme", "light");
  expect(window.localStorage.getItem("mcg.theme")).toBe("light");

  fireEvent.click(screen.getByRole("checkbox", { name: /Reduce motion/ }));
  expect(document.documentElement).toHaveAttribute("data-reduce-motion", "true");
  expect(window.localStorage.getItem("mcg.reduce-motion")).toBe("true");
});

test("Keyboard shortcuts is searchable and lists only implemented bindings", async () => {
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Keyboard shortcuts" }));
  const search = screen.getByRole("textbox", { name: "Search shortcuts" });
  expect(screen.getByText("Open global folder")).toBeInTheDocument();

  fireEvent.change(search, { target: { value: "full screen" } });
  expect(screen.getByText("Toggle full screen")).toBeInTheDocument();
  expect(screen.queryByText("Open global folder")).not.toBeInTheDocument();

  fireEvent.change(search, { target: { value: "temporary chat" } });
  expect(screen.getByText("No shortcuts found.")).toBeInTheDocument();
});

test("advanced mode keeps device-code login an explicit headless fallback", async () => {
  client.start.mockResolvedValue({
    session_id: "device-session",
    connection_id: "openai-subscription",
    auth_mode: "chat_gpt_device_code",
    verification_url: "https://auth.openai.com/codex/device",
    user_code: "ABCD-1234"
  });
  client.complete.mockReturnValue(new Promise(() => undefined));
  render(<App />);

  await enableAdvancedMode();
  fireEvent.click(screen.getByText("Can't open a browser?"));
  fireEvent.click(screen.getByRole("button", { name: "Use a device code instead" }));

  expect(await screen.findByText("Code:", { exact: false })).toHaveTextContent("ABCD-1234");
  expect(screen.getByRole("button", { name: "Open device authorization page" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "I've entered the code" })).not.toBeInTheDocument();
  expect(screen.getByText(/connect automatically/i)).toBeInTheDocument();
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
    discovered_at: new Date().toISOString()
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
  expect(within(document.querySelector("#providers") as HTMLElement).getByText("OpenAI Subscription disconnected")).toBeInTheDocument();
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
  expect(within(document.querySelector("#providers") as HTMLElement).getByText(/2 conversations/)).toHaveTextContent("$0.012345 known cost");
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

test("new chat opens immediately as a conversation without creating a project or durable task", async () => {
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
  client.listProjects.mockResolvedValue({ projects: [project] });
  render(<App />);

  await waitFor(() => expect(within(screen.getByLabelText("Project")).getByRole("option", { name: "sample" })).toBeInTheDocument());
  fireEvent.change(screen.getByLabelText("Project"), {
    target: { value: project.project_id }
  });
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Explain this repository" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect(await screen.findByRole("heading", { name: "Explain this repository" })).toBeInTheDocument();
  expect(screen.getByLabelText("Chat conversation")).toBeInTheDocument();
  expect(client.createConversation).toHaveBeenCalledWith({
    title: "Explain this repository",
    project_id: project.project_id,
    execution_preferences: { model_id: undefined, reasoning_effort: "medium", service_tier: undefined, approval_mode: "ask" }
  });
  expect(client.sendConversationMessage).toHaveBeenCalledWith("00000000-0000-4000-8000-000000000c01", {
    content: "Explain this repository",
    execution_preferences: { model_id: undefined, reasoning_effort: "medium", service_tier: undefined, approval_mode: "ask" }
  });
  expect(client.createTask).not.toHaveBeenCalled();
  expect(client.launchTask).not.toHaveBeenCalled();
  expect(screen.getByText("Fixture response")).toBeInTheDocument();
});

test("active chats queue Enter messages in a drawer and allow immediate steering", async () => {
  const conversation = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000c01",
    title: "Active chat",
    state: "active" as const,
    execution_preferences: {},
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:01Z"
  };
  const turn = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000c02",
    conversation_id: conversation.id,
    state: "streaming" as const,
    execution_preferences: {},
    codex_thread_id: "codex-thread-1",
    codex_turn_id: "codex-turn-1",
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:01Z"
  };
  const activeView = {
    conversation,
    turns: [turn],
    messages: [{
      schema_version: 1,
      id: "00000000-0000-4000-8000-000000000c03",
      conversation_id: conversation.id,
      turn_id: turn.id,
      role: "user" as const,
      content: "Start working",
      created_at: turn.created_at
    }],
    activities: []
  };
  client.getConversation.mockResolvedValue(activeView);
  client.sendConversationMessage.mockReturnValue(new Promise(() => undefined));
  client.steerConversationTurn.mockResolvedValue(activeView);
  render(<App />);

  const composer = screen.getByLabelText("Message");
  fireEvent.change(composer, { target: { value: "Start working" } });
  fireEvent.keyDown(composer, { key: "Enter", code: "Enter" });
  expect(await screen.findByRole("heading", { name: "Start working" })).toBeInTheDocument();

  const activeComposer = screen.getByLabelText("Message");
  fireEvent.change(activeComposer, { target: { value: "Use the blue design" } });
  fireEvent.keyDown(activeComposer, { key: "Enter", code: "Enter" });
  expect(screen.getByLabelText("Queued messages")).toHaveTextContent("Use the blue design");
  expect(screen.getByRole("button", { name: "Queue" })).toBeInTheDocument();

  await waitFor(() => expect(client.getConversation).toHaveBeenCalledWith(conversation.id), { timeout: 1500 });
  fireEvent.click(screen.getByRole("button", { name: /Steer/ }));
  await waitFor(() => expect(client.steerConversationTurn).toHaveBeenCalledWith(
    conversation.id,
    turn.id,
    "Use the blue design"
  ));
  expect(screen.queryByLabelText("Queued messages")).not.toBeInTheDocument();

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Keep the current structure" } });
  fireEvent.keyDown(screen.getByLabelText("Message"), { key: "Enter", code: "Enter", ctrlKey: true });
  await waitFor(() => expect(client.steerConversationTurn).toHaveBeenLastCalledWith(
    conversation.id,
    turn.id,
    "Keep the current structure"
  ));
  expect(screen.getByLabelText("Message")).toHaveValue("");
});

test("permission mode is selected before prompting and sent with the conversation turn", async () => {
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
  client.listProjects.mockResolvedValue({ projects: [project] });
  render(<App />);

  await waitFor(() => expect(within(screen.getByLabelText("Project")).getByRole("option", { name: "sample" })).toBeInTheDocument());
  fireEvent.change(screen.getByLabelText("Project"), { target: { value: project.project_id } });
  fireEvent.click(screen.getByLabelText("Change permissions"));
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Approve for me/ }));
  expect(screen.getByLabelText("Change permissions")).toHaveTextContent("Approve for me");

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Review this repository" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ approval_mode: "auto_review" }) })
  ));
});

test("project-free chats allow elevated permission selection while explaining the read-only boundary", async () => {
  render(<App />);
  fireEvent.click(screen.getByLabelText("Change permissions"));
  const approveForMe = screen.getByRole("menuitemradio", { name: /Approve for me/ });
  expect(approveForMe).toBeEnabled();
  fireEvent.click(approveForMe);
  expect(screen.getByLabelText("Change permissions")).toHaveTextContent("Approve for me");

  fireEvent.click(screen.getByLabelText("Change permissions"));
  const fullAccess = screen.getByRole("menuitemradio", { name: /Full access/ });
  expect(fullAccess).toBeEnabled();
  expect(screen.getByText("Your selection is saved. Project-free turns stay read-only until you attach a project.")).toBeInTheDocument();
  fireEvent.click(fullAccess);
  expect(screen.getByLabelText("Change permissions")).toHaveTextContent("Full access");

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Explain this safely" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ approval_mode: "full_access" }) })
  ));
});

test("chat follow threshold treats a few lines from the bottom as live", () => {
  expect(isChatViewportNearBottom({ scrollHeight: 1_000, scrollTop: 410, clientHeight: 500 } as HTMLElement)).toBe(true);
  expect(isChatViewportNearBottom({ scrollHeight: 1_000, scrollTop: 300, clientHeight: 500 } as HTMLElement)).toBe(false);
});

test("conversation streaming follows near the bottom and pauses after the user scrolls up", async () => {
  const conversation = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000c01",
    title: "Explain streaming",
    state: "active" as const,
    execution_preferences: {},
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString()
  };
  const turn = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000c02",
    conversation_id: conversation.id,
    state: "streaming" as "streaming" | "completed",
    execution_preferences: {},
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString()
  };
  const streamedView = {
    conversation,
    turns: [turn],
    messages: [
      { schema_version: 1, id: "00000000-0000-4000-8000-000000000c03", conversation_id: conversation.id, turn_id: turn.id, role: "user" as const, content: "Explain streaming", created_at: turn.created_at },
      { schema_version: 1, id: "00000000-0000-4000-8000-000000000c04", conversation_id: conversation.id, turn_id: turn.id, role: "assistant" as const, content: "Here is a partial", created_at: turn.created_at }
    ],
    activities: []
  };
  let finishSend: ((view: typeof streamedView) => void) | undefined;
  client.getConversation.mockResolvedValue(streamedView);
  client.sendConversationMessage.mockReturnValue(new Promise((resolve) => { finishSend = resolve; }));
  const { container } = render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Toggle side panel" }));
  const workspace = container.querySelector<HTMLElement>(".workspace");
  expect(workspace).not.toBeNull();
  let scrollTop = 0;
  Object.defineProperties(workspace!, {
    scrollHeight: { configurable: true, get: () => 1_000 },
    clientHeight: { configurable: true, get: () => 500 },
    scrollTop: {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => { scrollTop = Math.min(value, 500); }
    }
  });

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Explain streaming" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  expect(await screen.findByText("Here is a partial", {}, { timeout: 5000 })).toBeInTheDocument();
  expect(screen.getByText(/^Responding ·/)).toBeInTheDocument();
  await waitFor(() => expect(scrollTop).toBe(500));

  scrollTop = 250;
  fireEvent.scroll(workspace!);
  client.getConversation.mockResolvedValue({
    ...streamedView,
    messages: [{ ...streamedView.messages[0] }, { ...streamedView.messages[1], content: "A second partial response" }]
  });
  expect(await screen.findByText("A second partial response", {}, { timeout: 5000 })).toBeInTheDocument();
  expect(scrollTop).toBe(250);

  scrollTop = 450;
  fireEvent.scroll(workspace!);
  client.getConversation.mockResolvedValue({
    ...streamedView,
    messages: [{ ...streamedView.messages[0] }, { ...streamedView.messages[1], content: "The latest partial response" }]
  });
  expect(await screen.findByText("The latest partial response", {}, { timeout: 5000 })).toBeInTheDocument();
  await waitFor(() => expect(scrollTop).toBe(500));

  finishSend?.({
    ...streamedView,
    turns: [{ ...turn, state: "completed" as const }],
    messages: [{ ...streamedView.messages[0] }, { ...streamedView.messages[1], content: "Here is the complete response." }]
  });
  expect(await screen.findByText("Here is the complete response.")).toBeInTheDocument();
  expect(screen.queryByText(/^Responding ·/)).not.toBeInTheDocument();
  expect(scrollTop).toBe(500);
});

test("conversation activity shows command lifecycle, details, and context compaction", async () => {
  const conversation = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000e01",
    title: "Inspect activity",
    state: "active" as const,
    execution_preferences: {},
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:02Z"
  };
  const turn = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000e02",
    conversation_id: conversation.id,
    state: "completed" as const,
    execution_preferences: {},
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:02Z"
  };
  const activityView = {
    conversation,
    turns: [turn],
    messages: [
      { schema_version: 1, id: "00000000-0000-4000-8000-000000000e03", conversation_id: conversation.id, turn_id: turn.id, role: "user" as const, content: "Inspect activity", created_at: turn.created_at },
      { schema_version: 1, id: "00000000-0000-4000-8000-000000000e04", conversation_id: conversation.id, turn_id: turn.id, role: "assistant" as const, content: "I will inspect the workspace.", codex_item_id: "message-1", created_at: "2026-08-22T00:00:01Z" },
      { schema_version: 1, id: "00000000-0000-4000-8000-000000000e05", conversation_id: conversation.id, turn_id: turn.id, role: "assistant" as const, content: "The first checks passed.", codex_item_id: "message-2", created_at: "2026-08-22T00:00:04Z" },
      { schema_version: 1, id: "00000000-0000-4000-8000-000000000e06", conversation_id: conversation.id, turn_id: turn.id, role: "assistant" as const, content: "Finished inspecting.", codex_item_id: "message-3", created_at: "2026-08-22T00:00:07Z" }
    ],
    activities: [
      {
        schema_version: 1,
        conversation_id: conversation.id,
        turn_id: turn.id,
        codex_item_id: "command-1",
        kind: "command" as const,
        state: "completed" as const,
        command: "Get-Content README.md",
        cwd: "C:\\workspace",
        output: "# MCG",
        exit_code: 0,
        duration_ms: 1250,
        started_at: "2026-08-22T00:00:02Z",
        updated_at: "2026-08-22T00:00:02Z"
      },
      {
        schema_version: 1,
        conversation_id: conversation.id,
        turn_id: turn.id,
        codex_item_id: "command-2",
        kind: "command" as const,
        state: "completed" as const,
        command: "npm test",
        started_at: "2026-08-22T00:00:03Z",
        updated_at: "2026-08-22T00:00:03Z"
      },
      {
        schema_version: 1,
        conversation_id: conversation.id,
        turn_id: turn.id,
        codex_item_id: "usage-1",
        kind: "usage" as const,
        state: "completed" as const,
        command: "Usage updated: 1,240 tokens",
        output: "1,240 tokens used",
        started_at: "2026-08-22T00:00:04.500Z",
        updated_at: "2026-08-22T00:00:04.500Z"
      },
      {
        schema_version: 1,
        conversation_id: conversation.id,
        turn_id: turn.id,
        codex_item_id: "compact-1",
        kind: "context_compaction" as const,
        state: "completed" as const,
        started_at: "2026-08-22T00:00:05Z",
        updated_at: "2026-08-22T00:00:05Z"
      },
      {
        schema_version: 1,
        conversation_id: conversation.id,
        turn_id: turn.id,
        codex_item_id: "command-3",
        kind: "command" as const,
        state: "running" as const,
        command: "cargo check --workspace",
        started_at: "2026-08-22T00:00:06Z",
        updated_at: "2026-08-22T00:00:06Z"
      }
    ]
  };
  client.getConversation.mockResolvedValue(activityView);
  client.sendConversationMessage.mockReturnValue(new Promise(() => undefined));
  render(<App />);

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Inspect activity" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  const completedGroup = await screen.findByText("Ran commands", {}, { timeout: 1500 });
  const runningGroup = screen.getByText("Running commands");
  expect(completedGroup.closest("details")).not.toHaveAttribute("open");
  expect(runningGroup.closest("details")).not.toHaveAttribute("open");
  expect(screen.getByTitle("npm test")).toBeInTheDocument();
  expect(screen.getByTitle("cargo check --workspace")).toBeInTheDocument();
  expect(screen.getByText("2 commands")).toBeInTheDocument();
  expect(screen.getByText("1 command")).toBeInTheDocument();
  expect(screen.getByText("Context automatically compacted")).toBeInTheDocument();
  expect(screen.queryByText(/Usage updated/)).not.toBeInTheDocument();
  expect(screen.queryByText("1,240 tokens used")).not.toBeInTheDocument();
  const timelineText = Array.from(screen.getByLabelText("Turn activity").children).map((element) => element.textContent);
  expect(timelineText).toEqual([
    "Chat AgentI will inspect the workspace.",
    expect.stringContaining("Ran commands2 commandsnpm test"),
    "Chat AgentThe first checks passed.",
    "≡Context automatically compacted",
    expect.stringContaining("Running commands1 commandcargo check --workspace"),
    "Chat AgentFinished inspecting."
  ]);
  fireEvent.click(completedGroup.closest("summary")!);
  fireEvent.click(screen.getByText("Ran Get-Content README.md").closest("summary")!);
  expect(screen.getByText("C:\\workspace")).toBeInTheDocument();
  expect(screen.getByText("Exit code 0")).toBeInTheDocument();
  expect(screen.getByText("# MCG")).toBeInTheDocument();
});

test("conversation approval is shown in chat and resolves without restarting the task", async () => {
  const conversation = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000d01",
    project_id: "00000000-0000-4000-8000-000000000d02",
    title: "Install the Sites starter",
    state: "active" as const,
    execution_preferences: {},
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString()
  };
  const turn = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000d03",
    conversation_id: conversation.id,
    task_id: "00000000-0000-4000-8000-000000000d04",
    state: "waiting_for_approval" as const,
    execution_preferences: {},
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString()
  };
  const approval = {
    id: "00000000-0000-4000-8000-000000000d05",
    task_id: turn.task_id,
    action: "item/commandExecution/requestApproval",
    resource: "npm package download for the Sites starter",
    reason: "The starter package requires network access.",
    state: "pending"
  };
  const waitingView = {
    conversation,
    turns: [turn],
    messages: [
      { schema_version: 1, id: "00000000-0000-4000-8000-000000000d06", conversation_id: conversation.id, turn_id: turn.id, role: "user" as const, content: "Build the site", created_at: turn.created_at }
    ],
    activities: []
  };
  let finishSend: ((view: typeof waitingView) => void) | undefined;
  client.getConversation.mockResolvedValue(waitingView);
  client.listApprovals.mockResolvedValue({ approvals: [approval] });
  client.approveRequest.mockResolvedValue({ ...approval, state: "approved" });
  client.sendConversationMessage.mockReturnValue(new Promise((resolve) => { finishSend = resolve; }));
  render(<App />);

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Build the site" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  expect(await screen.findByText("Permission needed to continue", {}, { timeout: 1500 })).toBeInTheDocument();
  expect(screen.getByText(approval.reason)).toBeInTheDocument();
  expect(screen.getByText(approval.resource)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Allow once" }));

  await waitFor(() => expect(client.approveRequest).toHaveBeenCalledWith(approval.id));
  expect(client.resumeTask).not.toHaveBeenCalled();
  finishSend?.(waitingView);
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

  fireEvent.click(await screen.findByRole("button", { name: blockedTask.goal }));
  fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
  expect(client.retryTask).toHaveBeenCalledWith(blockedTask.id, {
    approval_mode: "ask",
    reasoning_effort: "medium",
    speed: "standard",
  });
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

  fireEvent.click(await screen.findByRole("button", { name: waitingTask.goal }));
  expect(await screen.findByText(approval.reason)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Approve and continue" }));
  expect(client.approveRequest).toHaveBeenCalledWith(approval.id);
  await waitFor(() =>
    expect(client.resumeTask).toHaveBeenCalledWith(waitingTask.id, {
      approval_mode: "ask",
      reasoning_effort: "medium",
      speed: "standard",
    }),
  );
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

  fireEvent.click(await screen.findByRole("button", { name: waitingTask.goal }));
  fireEvent.click(await screen.findByRole("button", { name: "Deny" }));
  expect(client.denyRequest).toHaveBeenCalledWith(approval.id);
  expect(await screen.findByText("blocked")).toBeInTheDocument();
  expect(screen.getByText("Request denied; task is blocked and can be retried.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
});

test("native project selection discovers the folder and associates the new conversation", async () => {
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
  render(<App />);

  const folderButton = screen.getByRole("button", { name: "Open folder" });
  expect(folderButton).not.toHaveTextContent("Open folder");
  fireEvent.click(folderButton);
  expect((await screen.findAllByText("sample")).length).toBeGreaterThan(0);
  expect(screen.getByLabelText("Project")).toHaveValue(project.project_id);
  expect(document.querySelector(".selected-project")).toBeNull();
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Improve the sample" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  expect(await screen.findByRole("heading", { name: "Improve the sample" })).toBeInTheDocument();
  expect(client.discoverProject).toHaveBeenCalledWith(root);
  expect(client.createConversation).toHaveBeenCalledWith(expect.objectContaining({
    project_id: project.project_id
  }));
  expect(client.createTask).not.toHaveBeenCalled();
});

test("Ctrl+O registers a global folder without creating a project", async () => {
  const root = "C:\\Projects\\global-folder";
  const project = {
    id: "00000000-0000-4000-8000-000000000014",
    project_id: "00000000-0000-4000-8000-000000000015",
    root,
    observed_at: "2026-08-21T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: [], build_systems: [], test_systems: [], instructions: { conflicts: [] }
  };
  client.pickProject.mockResolvedValue(root);
  client.discoverProject.mockResolvedValue({ project });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "File" }));
  expect(screen.getByRole("button", { name: /Open Folder.*Ctrl\+O/ })).toBeInTheDocument();
  expect(within(screen.getByLabelText("Global folders")).getByRole("button", { name: "Add global folder" })).toBeInTheDocument();
  fireEvent.keyDown(window, { key: "o", ctrlKey: true });

  expect(await within(screen.getByLabelText("Global folders")).findByText("global-folder")).toBeInTheDocument();
  expect(screen.queryByRole("option", { name: "global-folder" })).not.toBeInTheDocument();
  expect(within(screen.getByLabelText("Projects")).queryByRole("button", { name: /^global-folder$/ })).not.toBeInTheDocument();
  expect(client.discoverProject).not.toHaveBeenCalled();
  expect(JSON.parse(window.localStorage.getItem("mcg.global-folders") ?? "[]")).toEqual([root]);

  fireEvent.click(within(screen.getByLabelText("Global folders")).getByRole("button", { name: "Remove" }));
  expect(within(screen.getByLabelText("Global folders")).getByText("No global folders added.")).toBeInTheDocument();
});

test("desktop menus follow the Codex command structure with MCG branding", async () => {
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "File" }));
  let menu = within(document.querySelector(".desktop-menu-popover.open") as HTMLElement);
  expect(menu.getByRole("button", { name: "New Window" })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /New Chat.*Ctrl\+N/ })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /New Temporary Chat/ })).toBeDisabled();
  expect(menu.getByRole("button", { name: /Open Folder.*Ctrl\+O/ })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /Quit MCG.*Ctrl\+Q/ })).toBeInTheDocument();
  expect(menu.queryByText(/ChatGPT/)).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  menu = within(document.querySelector(".desktop-menu-popover.open") as HTMLElement);
  expect(menu.getByRole("button", { name: /Undo.*Ctrl\+Z/ })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /Settings.*Ctrl\+,/ })).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "View" }));
  menu = within(document.querySelector(".desktop-menu-popover.open") as HTMLElement);
  expect(menu.getByRole("button", { name: /Toggle Sidebar.*Ctrl\+B/ })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /Open Terminal/ })).toBeEnabled();
  expect(menu.getByRole("button", { name: /Toggle Full Screen.*F11/ })).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Help" }));
  menu = within(document.querySelector(".desktop-menu-popover.open") as HTMLElement);
  expect(menu.getByRole("button", { name: /Keyboard Shortcuts.*Ctrl\+\// })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: "About MCG" })).toBeInTheDocument();
  expect(menu.queryByText(/About ChatGPT/)).not.toBeInTheDocument();
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

  fireEvent.click(await screen.findByRole("button", { name: "Open folder" }));

  await waitFor(() => expect(client.discoverProject).toHaveBeenCalledWith(selectedRoot, project.project_id));
});

test("project intelligence stays advanced while project chat remains the simple default", async () => {
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
  fireEvent.click(await screen.findByRole("button", { name: "Start chat" }));
  expect(screen.getByRole("heading", { name: "New chat", level: 1 })).toBeInTheDocument();
  expect(screen.getByLabelText("Project")).toHaveValue(project.project_id);
  expect(document.querySelector(".selected-project")).toBeNull();
  expect(screen.queryByLabelText("Project intelligence and memory")).not.toBeInTheDocument();
  await enableAdvancedMode();
  const intelligence = await screen.findByLabelText("Project intelligence and memory");
  expect(within(intelligence).getByRole("list", { name: "Project topology plan" })).toHaveTextContent("Stage 1");
  expect(within(intelligence).getByRole("list", { name: "Project topology plan" })).toHaveTextContent("main database");
  expect(within(intelligence).getByRole("list", { name: "Project topology plan" })).toHaveTextContent("database · compose.yaml");
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
  fireEvent.click(await screen.findByRole("button", { name: "Start chat" }));
  expect(screen.queryByLabelText("Project intelligence and memory")).not.toBeInTheDocument();
  await enableAdvancedMode();
  const intelligence = await screen.findByLabelText("Project intelligence and memory");
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
  fireEvent.click((await screen.findAllByRole("button", { name: "Start chat" }))[0]!);
  expect(screen.queryByLabelText("Project intelligence and memory")).not.toBeInTheDocument();
  await enableAdvancedMode();
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

  fireEvent.click(screen.getByRole("button", { name: "Open folder" }));

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

  fireEvent.click(await screen.findByRole("button", { name: blockedTask.goal }));
  fireEvent.click(await screen.findByRole("button", { name: "Evidence" }));

  expect(within(await screen.findByRole("region", { name: "Task evidence" })).getByText("Blocked")).toBeInTheDocument();
});

test("simple Plugins reviews access, enables an extension, and starts a new chat", async () => {
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
  const enabled = { ...descriptor, installation: { ...granted.installation, revision: 3, enabled: true } };
  client.listPlugins
    .mockResolvedValueOnce({ plugins: [descriptor] })
    .mockResolvedValueOnce({ plugins: [granted] })
    .mockResolvedValue({ plugins: [enabled] });
  client.setPluginGrants.mockResolvedValue(granted.installation);
  client.setPluginEnabled.mockResolvedValue(enabled.installation);
  render(<App />);

  expect(await screen.findByRole("heading", { name: "Plugin directory" })).toBeInTheDocument();
  expect(await screen.findByRole("heading", { name: "Weather helper" })).toBeInTheDocument();
  expect(screen.queryByLabelText("Plugin input (JSON)")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Install plugin" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "OpenAI" }));
  expect(screen.getByText("OpenAI catalog not connected")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Personal" }));
  expect(screen.getByRole("heading", { name: "Weather helper" })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Search plugins"), { target: { value: "missing" } });
  expect(screen.getByText("No matching plugins")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Search plugins"), { target: { value: "weather" } });
  expect(screen.getByRole("button", { name: "Turn on" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Review access" }));
  fireEvent.change(screen.getByLabelText("Weather helper network origin"), { target: { value: "https://weather.example" } });
  fireEvent.click(screen.getByRole("button", { name: "Approve requested access" }));

  await waitFor(() => expect(client.setPluginGrants).toHaveBeenCalledWith("example.weather", {
    expected_revision: 1,
    grants: [grant]
  }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Turn on" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Turn on" }));
  await waitFor(() => expect(client.setPluginEnabled).toHaveBeenCalledWith("example.weather", 2, true));
  fireEvent.click(await screen.findByRole("button", { name: "Start new chat" }));
  expect(screen.getByLabelText("Message")).toHaveValue("Use @Weather helper to ");
});
