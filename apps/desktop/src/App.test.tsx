import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const nativeWindow = vi.hoisted(() => ({
  close: vi.fn(),
  destroy: vi.fn(),
  minimize: vi.fn(),
  toggleMaximize: vi.fn(),
  isFullscreen: vi.fn(),
  setFullscreen: vi.fn()
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => nativeWindow
}));

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
  githubStatus: vi.fn(),
  startGitHubLogin: vi.fn(),
  completeGitHubLogin: vi.fn(),
  disconnectGitHub: vi.fn(),
  listGitHubRepositories: vi.fn(),
  createGitHubRepository: vi.fn(),
  linkGitHubRepository: vi.fn(),
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
  updateConversationMode: vi.fn(),
  updateConversationGoal: vi.fn(),
  clearConversationGoal: vi.fn(),
  answerConversationInput: vi.fn(),
  sendConversationMessage: vi.fn(),
  steerConversationTurn: vi.fn(),
  searchProjectMentions: vi.fn(),
  pickMentionFiles: vi.fn(),
  pickMentionFolder: vi.fn(),
  pickWritableFolder: vi.fn(),
  pickSandboxReadFolder: vi.fn(),
  getCodexModels: vi.fn(),
  getCodexPermissionProfiles: vi.fn(),
  getCodexFeatures: vi.fn(),
  setCodexFeature: vi.fn(),
  getCodexAccount: vi.fn(),
  getCodexRateLimits: vi.fn(),
  getCodexHarnessStatus: vi.fn(),
  getCodexConfigurationStatus: vi.fn(),
  setupWindowsSandbox: vi.fn(),
  grantWindowsSandboxReadDirectory: vi.fn(),
  getCodexPendingRequests: vi.fn(),
  getCodexStableDiscovery: vi.fn(),
  updateCodexHookState: vi.fn(),
  reconcileOwnedCodexThread: vi.fn(),
  forkOwnedCodexThread: vi.fn(),
  compactOwnedCodexThread: vi.fn(),
  startOwnedCodexReview: vi.fn(),
  watchConversationProject: vi.fn(),
  unwatchConversationProject: vi.fn(),
  startCodexLogin: vi.fn(),
  updateCodexTransport: vi.fn(),
  cancelCodexLogin: vi.fn(),
  logoutCodexAccount: vi.fn(),
  createTask: vi.fn(),
  launchTask: vi.fn(),
  retryTask: vi.fn(),
  resumeTask: vi.fn(),
  listApprovals: vi.fn(),
  getApproval: vi.fn(),
  approveRequest: vi.fn(),
  denyRequest: vi.fn(),
  transitionTask: vi.fn(),
  taskEvents: vi.fn(),
  taskAudit: vi.fn(),
  replayRun: vi.fn(),
  pickProject: vi.fn(),
  discoverProject: vi.fn(),
  initializeProjectGit: vi.fn(),
  listProjects: vi.fn(),
  archiveProject: vi.fn(),
  removeProject: vi.fn(),
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
  getGovernorControlPolicy: vi.fn(),
  listGovernorProposals: vi.fn(),
  getGovernorProposal: vi.fn(),
  decideGovernorProposal: vi.fn(),
  requestGovernorImplementation: vi.fn(),
  listManagedWorktrees: vi.fn(),
  listCloudDiffApplies: vi.fn(),
  prepareCloudDiffApply: vi.fn(),
  applyCloudDiff: vi.fn(),
  discardCloudDiffApply: vi.fn(),
  listAgentRuns: vi.fn(),
  listGlobalAgentSessions: vi.fn(),
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
  ,startConversationPreview: vi.fn()
  ,stopConversationPreview: vi.fn()
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
  ,protectedRootStatus: vi.fn()
  ,protectedRootWorkflows: vi.fn()
  ,protectedRootWorkflow: vi.fn()
  ,protectedRootWorkflowSourceLinks: vi.fn()
  ,protectedRootWorkflowActivity: vi.fn()
  ,protectedRootActions: vi.fn()
  ,protectedRootAction: vi.fn()
  ,compensateProtectedRootAction: vi.fn()
  ,protectedRootInvocations: vi.fn()
  ,protectedRootInvocation: vi.fn()
  ,protectedRootCapabilities: vi.fn()
  ,protectedRootCapability: vi.fn()
  ,cancelProtectedRootWorkflow: vi.fn()
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
  githubStatus: client.githubStatus,
  startGitHubLogin: client.startGitHubLogin,
  completeGitHubLogin: client.completeGitHubLogin,
  disconnectGitHub: client.disconnectGitHub,
  listGitHubRepositories: client.listGitHubRepositories,
  createGitHubRepository: client.createGitHubRepository,
  linkGitHubRepository: client.linkGitHubRepository,
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
  updateConversationMode: client.updateConversationMode,
  updateConversationGoal: client.updateConversationGoal,
  clearConversationGoal: client.clearConversationGoal,
  answerConversationInput: client.answerConversationInput,
  sendConversationMessage: client.sendConversationMessage,
  steerConversationTurn: client.steerConversationTurn,
  searchProjectMentions: client.searchProjectMentions,
  pickMentionFiles: client.pickMentionFiles,
  pickMentionFolder: client.pickMentionFolder,
  pickWritableFolder: client.pickWritableFolder,
  pickSandboxReadFolder: client.pickSandboxReadFolder,
  getCodexModels: client.getCodexModels,
  getCodexPermissionProfiles: client.getCodexPermissionProfiles,
  getCodexFeatures: client.getCodexFeatures,
  setCodexFeature: client.setCodexFeature,
  getCodexAccount: client.getCodexAccount,
  getCodexRateLimits: client.getCodexRateLimits,
  getCodexHarnessStatus: client.getCodexHarnessStatus,
  getCodexConfigurationStatus: client.getCodexConfigurationStatus,
  setupWindowsSandbox: client.setupWindowsSandbox,
  grantWindowsSandboxReadDirectory: client.grantWindowsSandboxReadDirectory,
  getCodexPendingRequests: client.getCodexPendingRequests,
  getCodexStableDiscovery: client.getCodexStableDiscovery,
  updateCodexHookState: client.updateCodexHookState,
  reconcileOwnedCodexThread: client.reconcileOwnedCodexThread,
  forkOwnedCodexThread: client.forkOwnedCodexThread,
  compactOwnedCodexThread: client.compactOwnedCodexThread,
  startOwnedCodexReview: client.startOwnedCodexReview,
  watchConversationProject: client.watchConversationProject,
  unwatchConversationProject: client.unwatchConversationProject,
  startCodexLogin: client.startCodexLogin,
  updateCodexTransport: client.updateCodexTransport,
  cancelCodexLogin: client.cancelCodexLogin,
  logoutCodexAccount: client.logoutCodexAccount,
  createTask: client.createTask,
  launchTask: client.launchTask,
  retryTask: client.retryTask,
  resumeTask: client.resumeTask,
  listApprovals: client.listApprovals,
  getApproval: client.getApproval,
  approveRequest: client.approveRequest,
  denyRequest: client.denyRequest,
  transitionTask: client.transitionTask,
  loadTaskEvents: client.taskEvents,
  loadTaskAudit: client.taskAudit,
  replayAgentRun: client.replayRun,
  pickProjectDirectory: client.pickProject,
  discoverProject: client.discoverProject,
  initializeProjectGit: client.initializeProjectGit,
  listProjects: client.listProjects,
  archiveProject: client.archiveProject,
  removeProject: client.removeProject,
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
  getGovernorControlPolicy: client.getGovernorControlPolicy,
  listGovernorProposals: client.listGovernorProposals,
  getGovernorProposal: client.getGovernorProposal,
  decideGovernorProposal: client.decideGovernorProposal,
  requestGovernorImplementation: client.requestGovernorImplementation,
  listManagedWorktrees: client.listManagedWorktrees,
  listCloudDiffApplies: client.listCloudDiffApplies,
  prepareCloudDiffApply: client.prepareCloudDiffApply,
  applyCloudDiff: client.applyCloudDiff,
  discardCloudDiffApply: client.discardCloudDiffApply,
  listAgentRuns: client.listAgentRuns,
  listGlobalAgentSessions: client.listGlobalAgentSessions,
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
  startConversationPreview: client.startConversationPreview,
  stopConversationPreview: client.stopConversationPreview,
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
  getProtectedRootStatus: client.protectedRootStatus,
  listProtectedRootWorkflows: client.protectedRootWorkflows,
  getProtectedRootWorkflow: client.protectedRootWorkflow,
  getProtectedRootWorkflowSourceLinks: client.protectedRootWorkflowSourceLinks,
  getProtectedRootWorkflowActivity: client.protectedRootWorkflowActivity,
  listProtectedRootActions: client.protectedRootActions,
  getProtectedRootAction: client.protectedRootAction,
  compensateProtectedRootAction: client.compensateProtectedRootAction,
  listProtectedRootInvocations: client.protectedRootInvocations,
  getProtectedRootInvocation: client.protectedRootInvocation,
  listProtectedRootCapabilities: client.protectedRootCapabilities,
  getProtectedRootCapability: client.protectedRootCapability,
  cancelProtectedRootWorkflow: client.cancelProtectedRootWorkflow,
  listOpenInApplications: client.listOpenInApplications,
  openWorkspaceIn: client.openWorkspaceIn,
  runTerminalCommand: client.runTerminalCommand
}));

import { App, conversationTeamHasPendingDelivery, effectiveDelegationAttempts, isChatViewportNearBottom } from "./App";

test("retry lineage counts only the latest effective coding attempt", () => {
  const attempts = [
    { id: "failed-attempt", state: "failed" },
    { id: "successful-retry", state: "completed", retry_of_id: "failed-attempt" },
    { id: "independent-agent", state: "completed" }
  ];
  expect(effectiveDelegationAttempts(attempts as never)).toEqual([
    attempts[1],
    attempts[2]
  ]);
});

beforeEach(() => {
  delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  for (const method of Object.values(nativeWindow)) method.mockReset();
  window.history.replaceState(null, "", "/");
  window.localStorage.clear();
  client.localServiceStatus.mockReset();
  client.protectedRootStatus.mockReset();
  client.protectedRootWorkflows.mockReset();
  client.protectedRootWorkflow.mockReset();
  client.protectedRootActions.mockReset();
  client.protectedRootAction.mockReset();
  client.compensateProtectedRootAction.mockReset();
  client.protectedRootInvocations.mockReset();
  client.protectedRootInvocation.mockReset();
  client.protectedRootCapabilities.mockReset();
  client.protectedRootCapability.mockReset();
  client.cancelProtectedRootWorkflow.mockReset();
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
  client.githubStatus.mockReset();
  client.startGitHubLogin.mockReset();
  client.completeGitHubLogin.mockReset();
  client.disconnectGitHub.mockReset();
  client.listGitHubRepositories.mockReset();
  client.createGitHubRepository.mockReset();
  client.linkGitHubRepository.mockReset();
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
  client.searchProjectMentions.mockReset();
  client.pickMentionFiles.mockReset();
  client.pickMentionFolder.mockReset();
  client.pickWritableFolder.mockReset();
  client.pickSandboxReadFolder.mockReset();
  client.getCodexModels.mockReset();
  client.getCodexPermissionProfiles.mockReset();
  client.getCodexFeatures.mockReset();
  client.setCodexFeature.mockReset();
  client.getCodexAccount.mockReset();
  client.getCodexRateLimits.mockReset();
  client.getCodexHarnessStatus.mockReset();
  client.getCodexConfigurationStatus.mockReset();
  client.setupWindowsSandbox.mockReset();
  client.grantWindowsSandboxReadDirectory.mockReset();
  client.getCodexPendingRequests.mockReset();
  client.getCodexStableDiscovery.mockReset();
  client.updateCodexHookState.mockReset();
  client.reconcileOwnedCodexThread.mockReset();
  client.forkOwnedCodexThread.mockReset();
  client.compactOwnedCodexThread.mockReset();
  client.startOwnedCodexReview.mockReset();
  client.watchConversationProject.mockReset();
  client.unwatchConversationProject.mockReset();
  client.startCodexLogin.mockReset();
  client.updateCodexTransport.mockReset();
  client.cancelCodexLogin.mockReset();
  client.logoutCodexAccount.mockReset();
  client.createTask.mockReset();
  client.launchTask.mockReset();
  client.retryTask.mockReset();
  client.resumeTask.mockReset();
  client.listApprovals.mockReset();
  client.getApproval.mockReset();
  client.approveRequest.mockReset();
  client.denyRequest.mockReset();
  client.transitionTask.mockReset();
  client.taskEvents.mockReset();
  client.taskAudit.mockReset();
  client.replayRun.mockReset();
  client.pickProject.mockReset();
  client.discoverProject.mockReset();
  client.initializeProjectGit.mockReset();
  client.listProjects.mockReset();
  client.archiveProject.mockReset();
  client.removeProject.mockReset();
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
  client.getGovernorControlPolicy.mockReset();
  client.listGovernorProposals.mockReset();
  client.getGovernorProposal.mockReset();
  client.decideGovernorProposal.mockReset();
  client.requestGovernorImplementation.mockReset();
  client.listManagedWorktrees.mockReset();
  client.listCloudDiffApplies.mockReset();
  client.prepareCloudDiffApply.mockReset();
  client.applyCloudDiff.mockReset();
  client.discardCloudDiffApply.mockReset();
  client.listAgentRuns.mockReset();
  client.listGlobalAgentSessions.mockReset();
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
  client.startConversationPreview.mockReset();
  client.stopConversationPreview.mockReset();
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
  client.githubStatus.mockResolvedValue({ configured: false, connected: false });
  client.listGitHubRepositories.mockResolvedValue({ repositories: [] });
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
    schema_version: 2,
    id: "00000000-0000-4000-8000-000000000c01",
    title: request.title ?? "New chat",
    project_id: request.project_id,
    state: "active",
    execution_preferences: request.execution_preferences,
    created_at: "2026-08-22T00:00:00Z",
    updated_at: "2026-08-22T00:00:00Z"
  }));
  client.updateConversation.mockImplementation(async (conversationId: string, request: { title: string; project_id?: string }) => ({
    schema_version: 2,
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
  client.searchProjectMentions.mockResolvedValue({ candidates: [] });
  client.pickMentionFiles.mockResolvedValue([]);
  client.pickMentionFolder.mockResolvedValue(null);
  client.pickWritableFolder.mockResolvedValue(null);
  client.pickSandboxReadFolder.mockResolvedValue(null);
  client.getCodexModels.mockResolvedValue([]);
  client.getCodexPermissionProfiles.mockResolvedValue([]);
  client.getCodexFeatures.mockResolvedValue([]);
  client.setCodexFeature.mockResolvedValue([]);
  client.getCodexAccount.mockResolvedValue({ authenticated: false, requires_openai_auth: true });
  client.getCodexRateLimits.mockResolvedValue({ refreshed_at: "2026-08-22T00:00:00Z" });
  client.getCodexHarnessStatus.mockResolvedValue({
    bundled_version: "0.150.1",
    protocol_version: "0.150.1",
    schema_sha256: "844b52d4a5a8cda58794e28b3b119c3a3d20a588b7db83209c298bec62704092",
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
    ,transport: { kind: "local_stdio" }
  });
  client.getCodexConfigurationStatus.mockResolvedValue({ requirements: {}, provider_capabilities: {}, windows_sandbox: { status: "ready" } });
  client.setupWindowsSandbox.mockResolvedValue({ mode: "elevated", readiness: "ready", config_write_status: "ok" });
  client.grantWindowsSandboxReadDirectory.mockImplementation(async (_projectId: string, path: string, permissionProfile: string) => ({ path, permission_profile: permissionProfile }));
  client.getCodexPendingRequests.mockResolvedValue([]);
  client.getCodexStableDiscovery.mockResolvedValue({
    skills: [],
    hooks: [],
    apps: [{ id: "calendar", name: "Calendar", description: "Work with calendar events", isAccessible: true, isEnabled: true }],
    appsAvailable: true,
    refreshedAt: "2026-08-22T00:00:00Z"
  });
  client.updateCodexHookState.mockImplementation(async () => client.getCodexStableDiscovery());
  client.reconcileOwnedCodexThread.mockResolvedValue({});
  client.forkOwnedCodexThread.mockResolvedValue({});
  client.compactOwnedCodexThread.mockResolvedValue({});
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
    name: "MetaCog Consumer Default",
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
  client.getGovernorControlPolicy.mockResolvedValue({
    schema_version: 1,
    governor_model_enabled: false,
    requires_owner_approval: true,
    allowed_actions: ["propose_prompt_policy_change", "request_governed_implementation"],
    denied_actions: ["mutate_source_directly"],
    protected_surfaces: ["authority_boundary"],
    implementation_path: "Chat Agent to Coordinator to Coding Agent"
  });
  client.listGovernorProposals.mockResolvedValue({ proposals: [] });
  client.listManagedWorktrees.mockResolvedValue({ worktrees: [] });
  client.listCloudDiffApplies.mockResolvedValue({ applies: [] });
  client.prepareCloudDiffApply.mockResolvedValue({
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000ca1",
    project_id: "00000000-0000-4000-8000-000000000702",
    task_id: "cloud-task-123",
    state: "ready",
    root: "C:\\MetaCog\\cloud-apply",
    branch: "mcg/cloud/fixture",
    base_head: "abc123",
    baseline_head: "abc123",
    baseline_sha256: "a".repeat(64),
    baseline_entries: [],
    baseline_exclusions: [],
    changed_paths: ["src/app.ts", "src/app.test.ts"],
    touched_paths: [],
    patch_sha256: "b".repeat(64),
    output: "Applied cloud task in isolated worktree",
    revision: 3,
    created_at: "2026-08-29T00:00:00Z",
    updated_at: "2026-08-29T00:00:01Z"
  });
  client.applyCloudDiff.mockImplementation(async (_applyId: string, expectedRevision: number) => ({
    ...(await client.prepareCloudDiffApply()),
    state: "applied",
    revision: expectedRevision + 1,
    updated_at: "2026-08-29T00:00:02Z"
  }));
  client.discardCloudDiffApply.mockResolvedValue({ state: "removed" });
  client.listAgentRuns.mockResolvedValue({ runs: [] });
  client.listGlobalAgentSessions.mockResolvedValue({ sessions: [] });
  client.listOrganizationPublishers.mockResolvedValue([]);
  client.listOrganizationShareReviews.mockResolvedValue([]);
  client.listApprovals.mockResolvedValue({ approvals: [] });
  client.getApproval.mockRejectedValue(new Error("No approval fixture configured"));
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
    interface_complexity: "simple",
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
      { code: "provider_dispatch_recovery", label: "Provider dispatch recovery", severity: "warning", detail: "1 physical provider request(s) are active or indeterminate; 4 validated request(s) completed; 2 definite request(s) failed.", remediation: "Review the affected task and provider billing/logs, then retry with a new owner-authorized call identity; MetaCog will not resend an indeterminate request automatically." },
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
  client.protectedRootStatus.mockResolvedValue({
    schema_version: 1,
    service: "protected-root",
    process_id: 4242,
    protocol_version: 1,
    session_id: "00000000-0000-4000-8000-000000000401",
    runtime_identity: {
      instance_id: "00000000-0000-4000-8000-000000000402",
      build_hash: { algorithm: "sha256", domain: "RootExecutable", digest_hex: "a".repeat(64) },
      protocol_version: 1,
      started_at: "2026-08-29T00:00:00Z"
    },
    storage_schema_version: 51,
    writer_queue_capacity: 64,
    migration_receipt: {
      schema_version: 1,
      from_storage_schema: 50,
      to_storage_schema: 51,
      backup_path: "backups/pre-root.mcgb",
      backup_sha256: "b".repeat(64),
      migrated_at: "2026-08-29T00:00:00Z"
    }
  });
  client.protectedRootWorkflows.mockResolvedValue([]);
  client.protectedRootWorkflow.mockResolvedValue(null);
  client.protectedRootWorkflowSourceLinks.mockResolvedValue([]);
  client.protectedRootWorkflowActivity.mockResolvedValue(null);
  client.protectedRootActions.mockResolvedValue([]);
  client.protectedRootAction.mockResolvedValue(null);
  client.compensateProtectedRootAction.mockRejectedValue(new Error("No action compensation fixture configured."));
  client.protectedRootInvocations.mockResolvedValue([]);
  client.protectedRootInvocation.mockResolvedValue(null);
  client.protectedRootCapabilities.mockResolvedValue([]);
  client.protectedRootCapability.mockResolvedValue(null);
  client.cancelProtectedRootWorkflow.mockRejectedValue(new Error("No workflow fixture configured."));
  client.updateSettings.mockImplementation(async (request) => ({
    schema_version: 1,
    revision: request.expected_revision + 1,
    interface_complexity: request.interface_complexity,
    autonomy: request.autonomy,
    shadow_experiments: request.shadow_experiments,
    updated_at: "2026-08-15T00:01:00Z"
  }));
  client.updateCodexTransport.mockImplementation(async (expectedRevision: number, endpoint?: string, authTokenEnv?: string) => ({
    ...settings,
    revision: expectedRevision + 1,
    codex_remote_endpoint: endpoint,
    codex_remote_auth_token_env: authTokenEnv,
    updated_at: "2026-08-15T00:02:00Z"
  }));
});

async function enableAdvancedMode() {
  const button = await screen.findByRole("button", { name: "Cycle interface complexity" });
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
  await waitFor(() => expect(document.querySelector(".app-shell.advanced-mode")).toBeInTheDocument());
}

async function enableSimpleMode() {
  const button = await screen.findByRole("button", { name: "Cycle interface complexity" });
  fireEvent.click(button);
  await waitFor(() => expect(button).toHaveTextContent("Developer"));
  fireEvent.click(button);
  await waitFor(() => expect(document.querySelector(".app-shell.simple-mode")).toBeInTheDocument());
}

async function selectNewChatProject(name: string) {
  fireEvent.click(screen.getByLabelText("Choose project"));
  const menu = screen.getByRole("menu", { name: "Project choices" });
  fireEvent.click(await within(menu).findByRole("menuitemradio", { name: new RegExp(name, "i") }));
}

function expectNewChatProject(name: string) {
  expect(screen.getByLabelText("Choose project")).toHaveTextContent(name);
}

function openNewChatRunSettings() {
  const selector = document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement;
  expect(selector).toBeInTheDocument();
  fireEvent.click(selector);
  return {
    selector,
    dialog: screen.getByRole("dialog", { name: "New chat model, effort, speed, and team" })
  };
}

test("custom desktop chrome invokes native minimize, maximize, and close controls", async () => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "Minimize" }));
  fireEvent.click(screen.getByRole("button", { name: "Maximize or restore" }));
  fireEvent.click(screen.getByRole("button", { name: "Close window" }));

  await waitFor(() => {
    expect(nativeWindow.minimize).toHaveBeenCalledOnce();
    expect(nativeWindow.toggleMaximize).toHaveBeenCalledOnce();
    expect(nativeWindow.close).toHaveBeenCalledOnce();
  });
});

test("workspace toolbar toggles the docked side panel and project terminal", async () => {
  render(<App />);

  const sidePanelToggle = await screen.findByRole("button", { name: "Toggle side panel" });
  const bottomPanelToggle = screen.getByRole("button", { name: "Toggle bottom panel" });
  expect(sidePanelToggle.querySelector("svg")).toBeInTheDocument();
  expect(bottomPanelToggle.querySelector("svg")).toBeInTheDocument();
  fireEvent.click(sidePanelToggle);
  expect(screen.getByRole("complementary", { name: "Workspace side panel" })).toBeInTheDocument();
  expect(screen.getByRole("tab", { name: "Browser", selected: true }).querySelector("svg")).toBeInTheDocument();
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
  const terminal = screen.getByRole("region", { name: "Terminal" });
  expect(terminal).toBeInTheDocument();
  expect(within(terminal).getByRole("button", { name: "Terminal" }).querySelector("svg")).toBeInTheDocument();
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
  expect(await screen.findByRole("heading", { name: "MetaCog runtime" })).toBeInTheDocument();
  expect(screen.getByText(/not a model agent/i)).toBeInTheDocument();
  expect(screen.getByText("Product team · revision 3")).toBeInTheDocument();
  expect(screen.getByText("Policy enforcement")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Teams" }));
  expect(screen.getByRole("combobox", { name: "Global default team" })).toHaveValue(organization.organization_id);
  expect(screen.queryByRole("button", { name: "Improvement" })).not.toBeInTheDocument();
});

test("durable workflows progressively disclose Root evidence and fence cancellation", async () => {
  const attempt = {
    workflow_id: "00000000-0000-4000-8000-000000000531",
    attempt_id: "00000000-0000-4000-8000-000000000532",
    task_contract: { id: "00000000-0000-4000-8000-000000000533", version: 1, content_hash: { algorithm: "blake3-256", domain: "MetaCog.TaskContract", digest_hex: "a".repeat(64) } },
    parent_attempt_id: null,
    attempt_number: 1,
    state: "running",
    revision: 2,
    lease_owner: "00000000-0000-4000-8000-000000000534",
    lease_generation: 1,
    lease_expires_at: "2026-08-30T13:00:00Z",
    heartbeat_at: "2026-08-30T12:00:00Z",
    checkpoint: null,
    environment_snapshot: { algorithm: "blake3-256", domain: "Environment", digest_hex: "b".repeat(64) },
    configuration: { id: "00000000-0000-4000-8000-000000000535", version: 1, content_hash: { algorithm: "blake3-256", domain: "Configuration", digest_hex: "c".repeat(64) } },
    idempotency_key: "create-workflow",
    retry_policy: { max_attempts: 3, initial_backoff_ms: 100, maximum_backoff_ms: 1000 },
    compensation_policy: {},
    created_at: "2026-08-30T12:00:00Z",
    updated_at: "2026-08-30T12:00:00Z"
  };
  const inspection = {
    attempt,
    task_contract: { id: attempt.task_contract.id, version: 1, objective: "Recover a durable report", non_goals: [], requirements: [], constraints: [], assumptions: [], acceptance_criteria: [{ clause_id: "durable", statement: "Survives restart" }], status: "active", created_at: attempt.created_at },
    events: [{ id: "00000000-0000-4000-8000-000000000536", type_id: "mcg.workflow_claimed.v1", schema_version: 1, sequence: 2, workflow_id: attempt.workflow_id, attempt_id: attempt.attempt_id, attempt_revision: 2, lease_generation: 1, correlation_id: "00000000-0000-4000-8000-000000000537", causation_id: null, idempotency_key: "claim", occurred_at: attempt.updated_at, payload: {} }]
  };
  client.getSettings.mockResolvedValue({ schema_version: 1, revision: 1, interface_complexity: "developer", autonomy: "autonomous", shadow_experiments: true, updated_at: attempt.updated_at });
  client.protectedRootWorkflows.mockResolvedValue([attempt]);
  client.protectedRootWorkflow.mockResolvedValue(inspection);
  client.protectedRootWorkflowActivity.mockResolvedValue({
    workflow: inspection,
    invocations: [{
      invocation_id: "00000000-0000-4000-8000-000000000538",
      workflow_id: attempt.workflow_id,
      attempt_id: attempt.attempt_id,
      role: "verify",
      task_contract: attempt.task_contract,
      model_run_id: null,
      model_run_state: "planned",
      model_run_revision: 1,
      persisted_at: attempt.updated_at,
      updated_at: attempt.updated_at
    }],
    invocations_truncated: false
  });
  client.protectedRootWorkflowSourceLinks.mockResolvedValue([{
    schema_version: 1,
    source: { source_type: "mcg.compatibility_task_source.v1", locator: "00000000-0000-4000-8000-000000000539", content_hash: { algorithm: "blake3-256", domain: "MetaCog.WorkflowSourceSnapshot", digest_hex: "d".repeat(64) } },
    source_snapshot: { type_id: "mcg.compatibility_task_source.v1", schema_version: 1, canonical_encoding: "cbor-v1", normalization: "nfc-v1", bytes: [1, 2, 3] },
    primary_source: { source_type: "mcg.compatibility_agent_turn_lease_source.v1", locator: "00000000-0000-4000-8000-000000000540", content_hash: { algorithm: "blake3-256", domain: "MetaCog.WorkflowSourceSnapshot", digest_hex: "e".repeat(64) } },
    workflow_id: attempt.workflow_id,
    attempt_id: attempt.attempt_id,
    task_contract: attempt.task_contract,
    linked_at: attempt.updated_at
  }]);
  client.cancelProtectedRootWorkflow.mockResolvedValue({ ...attempt, state: "cancel_requested", revision: 3, lease_generation: 2, lease_owner: null, lease_expires_at: null });
  const confirmation = vi.spyOn(window, "confirm").mockReturnValue(true);

  render(<App />);
  fireEvent.click(await screen.findByRole("link", { name: /Agents/ }));
  const workflows = await screen.findByRole("article", { name: "Governed workflow activity" });
  expect(within(workflows).getByText(/Working · attempt 1 · revision 2 · generation 1/)).toBeInTheDocument();
  fireEvent.change(within(workflows).getByLabelText("Exact source ID"), { target: { value: "00000000-0000-4000-8000-000000000539" } });
  fireEvent.click(within(workflows).getByRole("button", { name: "Resolve exact source" }));
  await waitFor(() => expect(client.protectedRootWorkflowSourceLinks).toHaveBeenCalledWith("mcg.compatibility_task_source.v1", "00000000-0000-4000-8000-000000000539"));
  expect(await within(workflows).findByRole("status", { name: "Source-linked workflows" })).toHaveTextContent("1 workflow linked to task");
  expect(within(workflows).getByRole("button", { name: "Exact workflow source evidence" })).toBeInTheDocument();
  fireEvent.click(within(workflows).getByRole("button", { name: "Inspect activity" }));
  expect(await within(workflows).findByText("Recover a durable report")).toBeInTheDocument();
  expect(within(workflows).getByText("Verification")).toBeInTheDocument();
  expect(within(workflows).getByRole("button", { name: "Canonical workflow activity evidence" })).toBeInTheDocument();
  fireEvent.click(within(workflows).getByRole("button", { name: "Request cancellation" }));
  await waitFor(() => expect(client.cancelProtectedRootWorkflow).toHaveBeenCalledWith(expect.objectContaining({
    attempt_id: attempt.attempt_id,
    expected_revision: 2,
    expected_generation: 1,
    idempotency_key: expect.stringMatching(/^desktop-cancel-/)
  })));
  expect(await within(workflows).findByText(/Stopping safely · attempt 1 · revision 3 · generation 2/)).toBeInTheDocument();
  confirmation.mockRestore();
});

test("Tool capability inspection progressively discloses one Root projection", async () => {
  const updatedAt = "2026-09-01T12:00:00Z";
  const summary = {
    action_id: "00000000-0000-4000-8000-000000000571",
    permit_id: "00000000-0000-4000-8000-000000000572",
    attempt_id: "00000000-0000-4000-8000-000000000573",
    tool_manifest: { id: "00000000-0000-4000-8000-000000000574", version: 1, content_hash: { algorithm: "blake3-256", domain: "MetaCog.ToolManifest", digest_hex: "a".repeat(64) } },
    tool_name: "Governed filesystem",
    effect_class: "reversible",
    filesystem_scope_count: 1,
    network_scope_count: 0,
    secret_slot_count: 0,
    process_spawn: false,
    state: "active",
    issued_at: updatedAt,
    expires_at: "2026-09-01T12:05:00Z"
  };
  const hash = (domain: string, digest = "b") => ({ algorithm: "blake3-256", domain, digest_hex: digest.repeat(64) });
  const source = (type: string, locator: string) => ({ source_type: type, locator, content_hash: hash("MetaCog.Source") });
  const inspection = {
    summary,
    runner: source("mcg.executable.v1", "C:/Program Files/MetaCog/mcg-action-runner.exe"),
    input_schema: source("mcg.schema.v1", "governed-filesystem-input"),
    output_schema: source("mcg.schema.v1", "governed-filesystem-output"),
    receipt_semantics: "mcg.filesystem_receipt.v1",
    reconciler: source("mcg.executable.v1", "C:/Program Files/MetaCog/mcg-action-runner.exe"),
    platform_support: ["windows"],
    runtime_identity: { instance_id: "00000000-0000-4000-8000-000000000575", build_hash: hash("MetaCog.RuntimeBuild"), protocol_version: 1, started_at: updatedAt },
    runtime_actor: "00000000-0000-4000-8000-000000000576",
    task_contract: { id: "00000000-0000-4000-8000-000000000577", version: 1, content_hash: hash("MetaCog.TaskContract") },
    attempt_revision: 3,
    lease_generation: 2,
    argument_hash: hash("MetaCog.ActionOperation"),
    proposal_hash: hash("MetaCog.ToolInvocationProposal"),
    permit_hash: hash("MetaCog.ToolInvocationPermit"),
    policy: { content_hash: hash("MetaCog.ToolPolicy"), payload: { type_id: "mcg.tool_policy.v1", schema_version: 1, canonical_encoding: "canonical-cbor-v1", normalization: "canonical-json-v1", bytes: [1, 2, 3] } },
    capability_scope_hash: hash("MetaCog.ToolCapabilitySet"),
    scopes: { filesystem: [{ root: "C:/workspace", access: "write" }], network: [], secret_slots: [], process_spawn: false, truncated: { filesystem: false, network: false, secret_slots: false } },
    limits: { timeout_ms: 30000, max_output_bytes: 1048576, max_memory_bytes: 268435456, max_child_processes: 0 },
    sandbox: { filesystem_scope: "hard", network: "hard", process_tree: "hard", secrets: "hard", environment: "hard", resources: "best_effort", protected_paths: "hard" },
    platform: "windows"
  };
  client.protectedRootCapabilities.mockResolvedValue([summary]);
  client.protectedRootCapability.mockResolvedValue(inspection);

  for (const complexity of ["simple", "advanced", "developer"] as const) {
    client.getSettings.mockResolvedValue({ schema_version: 1, revision: 1, interface_complexity: complexity, autonomy: "autonomous", shadow_experiments: true, updated_at: updatedAt });
    const view = render(<App />);
    fireEvent.click(await screen.findByRole("link", { name: /Agents/ }));
    const capabilities = await screen.findByRole("article", { name: "Governed capabilities" });
    expect(within(capabilities).getByText("Governed filesystem")).toBeInTheDocument();
    if (complexity === "simple") {
      expect(within(capabilities).getByText(/Bounded tool access · active/)).toBeInTheDocument();
      expect(within(capabilities).queryByRole("button", { name: "Inspect access" })).not.toBeInTheDocument();
    } else {
      expect(within(capabilities).getByText(/1 filesystem · 0 network/)).toBeInTheDocument();
      fireEvent.click(within(capabilities).getByRole("button", { name: "Inspect access" }));
      expect(await within(capabilities).findByText("C:/workspace")).toBeInTheDocument();
      if (complexity === "developer") {
        expect(within(capabilities).getByRole("button", { name: "Exact capability evidence" })).toBeInTheDocument();
      } else {
        expect(within(capabilities).queryByRole("button", { name: "Exact capability evidence" })).not.toBeInTheDocument();
      }
    }
    view.unmount();
  }
});

test("Developer progressively inspects canonical governed-action evidence", async () => {
  const conversationId = "00000000-0000-4000-8000-000000000546";
  const turnId = "00000000-0000-4000-8000-000000000554";
  const agentId = "00000000-0000-4000-8000-000000000555";
  const approvalId = "00000000-0000-4000-8000-000000000549";
  const invocationId = "00000000-0000-4000-8000-000000000547";
  const invocationHash = { algorithm: "blake3-256", domain: "MetaCog.InvocationManifest", digest_hex: "2".repeat(64) };
  const action = {
    action_id: "00000000-0000-4000-8000-000000000541",
    execution_id: "00000000-0000-4000-8000-000000000542",
    workflow_id: "00000000-0000-4000-8000-000000000531",
    attempt_id: "00000000-0000-4000-8000-000000000532",
    actor: "root-action-runner",
    effect_class: "reversible",
    state: "outcome_unknown",
    outcome: { outcome: "outcome_unknown" },
    revision: 3,
    lease_generation: 2,
    updated_at: "2026-08-30T12:00:00Z"
  };
  const payload = { type_id: "mcg.governed_filesystem_operation.v1", schema_version: 1, canonical_encoding: "canonical-cbor-v1", normalization: "canonical-json-v1", bytes: [1, 2, 3] };
  const inspection = {
    envelope: { id: action.action_id, workflow_id: action.workflow_id, attempt_id: action.attempt_id, actor: action.actor, origin: { conversation: { source_type: "mcg.conversation.v1", locator: conversationId, content_hash: { algorithm: "blake3-256", domain: "MetaCog.Conversation", digest_hex: "1".repeat(64) } }, invocation: { source_type: "mcg.invocation_manifest.v1", locator: invocationId, content_hash: invocationHash }, command: { source_type: "mcg.command_execution.v1", locator: "command-548", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Command", digest_hex: "3".repeat(64) } }, approval: { source_type: "mcg.approval_request.v1", locator: approvalId, content_hash: { algorithm: "blake3-256", domain: "MetaCog.Approval", digest_hex: "4".repeat(64) } }, agent: { source_type: "mcg.runtime_actor.v1", locator: action.actor, content_hash: { algorithm: "blake3-256", domain: "MetaCog.Actor", digest_hex: "5".repeat(64) } }, evidence: [{ source_type: "mcg.action_evidence.v1", locator: "artifact-550", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Evidence", digest_hex: "6".repeat(64) } }] }, effect_class: action.effect_class, operation: payload, operation_hash: { algorithm: "blake3-256", domain: "MetaCog.ActionOperation", digest_hex: "d".repeat(64) }, created_at: action.updated_at },
    compensation_actions: ["00000000-0000-4000-8000-000000000552"],
    executions: [{ id: action.execution_id, action_id: action.action_id, attempt_number: 1, state: action.state, revision: 3, lease_generation: 2, executor: action.actor, outcome: action.outcome, created_at: action.updated_at, updated_at: action.updated_at }],
    receipts: [],
    reconciliation_jobs: [{ id: "00000000-0000-4000-8000-000000000543", action_id: action.action_id, execution_id: action.execution_id, adapter: payload, state: "pending", revision: 1, lease_generation: 0, lease_owner: null, lease_expires_at: null, next_attempt_at: action.updated_at, attempts: 0, deadline: "2026-08-30T13:00:00Z", last_hint: null, created_at: action.updated_at, updated_at: action.updated_at }],
    events: [{ id: "00000000-0000-4000-8000-000000000544", type_id: "mcg.action_classified.v1", schema_version: 1, sequence: 4, action_id: action.action_id, execution_id: action.execution_id, correlation_id: "00000000-0000-4000-8000-000000000545", causation_id: null, idempotency_key: "classify", occurred_at: action.updated_at, payload }]
  };
  client.getSettings.mockResolvedValue({ schema_version: 1, revision: 1, interface_complexity: "developer", autonomy: "autonomous", shadow_experiments: true, updated_at: action.updated_at });
  client.protectedRootActions.mockResolvedValue([action]);
  client.protectedRootAction.mockImplementation(async (actionId: string) => actionId === action.action_id ? inspection : ({
    ...inspection,
    envelope: { ...inspection.envelope, id: actionId },
    compensation_actions: [],
    executions: inspection.executions.map((execution) => ({ ...execution, id: "00000000-0000-4000-8000-000000000553", action_id: actionId }))
  }));
  const conversation = { schema_version: 2, id: conversationId, title: "Exact approval conversation", state: "active", execution_preferences: {}, created_at: action.updated_at, updated_at: action.updated_at };
  client.listConversations.mockResolvedValue({ conversations: [conversation] });
  client.getConversation.mockResolvedValue({
    conversation,
    turns: [{ schema_version: 1, id: turnId, conversation_id: conversationId, state: "waiting_for_approval", execution_preferences: {}, task_id: action.workflow_id, created_at: action.updated_at, updated_at: action.updated_at }],
    messages: [],
    activities: [{ schema_version: 1, conversation_id: conversationId, turn_id: turnId, agent_session_id: agentId, codex_item_id: "command-548", kind: "command", state: "completed", command: "cargo test", started_at: action.updated_at, updated_at: action.updated_at }],
    team: { sessions: [{ schema_version: 8, id: agentId, conversation_id: conversationId, role_id: "role-coder", organization_revision_id: "org-revision", kind: "coder", display_name: "Coding Agent Atlas", state: "waiting", codex_thread_id: "thread-approval", prompt_revision: 1, created_at: action.updated_at, updated_at: action.updated_at }], delegations: [], messages: [], blackboard: [], worktrees: [] }
  });
  client.getApproval.mockResolvedValue({
    schema_version: 1,
    id: approvalId,
    tool_request_id: "00000000-0000-4000-8000-000000000556",
    task_id: action.workflow_id,
    action: "item/commandExecution/requestApproval",
    resource: "cargo test",
    reason: "Run the exact test command",
    origin: {
      conversation: { source_type: "mcg.conversation.v2", locator: conversationId, content_hash: { algorithm: "blake3-256", domain: "MetaCog.ApprovalConversation", digest_hex: "7".repeat(64) } },
      turn: { source_type: "mcg.conversation_turn.v1", locator: turnId, content_hash: { algorithm: "blake3-256", domain: "MetaCog.ApprovalTurn", digest_hex: "8".repeat(64) } },
      invocation: { source_type: "mcg.invocation_manifest.v1", locator: invocationId, content_hash: invocationHash },
      command: { source_type: "codex.command_approval_request.v1", locator: "command-548", content_hash: { algorithm: "blake3-256", domain: "MetaCog.ApprovalCommand", digest_hex: "9".repeat(64) } },
      agent: { source_type: "mcg.conversation_agent_session.v8", locator: agentId, content_hash: { algorithm: "blake3-256", domain: "MetaCog.ApprovalAgent", digest_hex: "a".repeat(64) } },
      evidence: [{ source_type: "codex.command_approval_evidence.v1", locator: "command-548:params", content_hash: { algorithm: "blake3-256", domain: "MetaCog.ApprovalEvidence", digest_hex: "b".repeat(64) } }]
    },
    state: "pending",
    requested_at: action.updated_at,
    expires_at: "2026-08-30T13:00:00Z"
  });
  const invocationSummary = {
    invocation_id: invocationId,
    workflow_id: action.workflow_id,
    attempt_id: action.attempt_id,
    role: "work",
    task_contract: { id: "00000000-0000-4000-8000-000000000557", version: 1, content_hash: { algorithm: "blake3-256", domain: "MetaCog.TaskContract", digest_hex: "c".repeat(64) } },
    model_run_id: null,
    model_run_state: "completed",
    model_run_revision: 1,
    persisted_at: action.updated_at,
    updated_at: action.updated_at
  };
  const invocationInspection = {
    manifest: {
      id: invocationId,
      role: "work",
      attempt_id: action.attempt_id,
      task_contract: invocationSummary.task_contract,
      runtime_identity: { instance_id: "runtime-approval", build_hash: { algorithm: "blake3-256", domain: "MetaCog.RuntimeExecutable", digest_hex: "d".repeat(64) }, protocol_version: 1, started_at: action.updated_at },
      resolved_provider: { source_type: "mcg.provider.v1", locator: "governed-provider", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Provider", digest_hex: "e".repeat(64) } },
      resolved_model: { source_type: "mcg.model.v1", locator: "approval-model", content_hash: { algorithm: "blake3-256", domain: "MetaCog.Model", digest_hex: "f".repeat(64) } },
      observed_model_fingerprint: null,
      runtime_build: { algorithm: "blake3-256", domain: "MetaCog.RuntimeBuild", digest_hex: "1".repeat(64) },
      request_hash: { algorithm: "blake3-256", domain: "MetaCog.ModelRequest", digest_hex: "2".repeat(64) },
      response_hash: null
    },
    manifest_hash: invocationHash,
    context_manifest: { id: "00000000-0000-4000-8000-000000000558", attempt_id: action.attempt_id, selected_sources: [], approximate_tokens: 0 },
    context_manifest_hash: { algorithm: "blake3-256", domain: "MetaCog.ContextManifest", digest_hex: "3".repeat(64) },
    model_runs: [],
    model_runs_truncated: false
  };
  client.protectedRootInvocations.mockResolvedValue([invocationSummary]);
  client.protectedRootInvocation.mockResolvedValue(invocationInspection);

  render(<App />);
  fireEvent.click(await screen.findByRole("link", { name: /Agents/ }));
  const actions = await screen.findByRole("article", { name: "Governed actions" });
  expect(within(actions).getByText(/Checking what happened · reversible · revision 3 · generation 2/)).toBeInTheDocument();
  fireEvent.click(within(actions).getByRole("button", { name: "Inspect" }));
  expect(await within(actions).findByText("mcg.governed_filesystem_operation.v1")).toBeInTheDocument();
  expect(within(actions).getByText("Reconciliation: pending")).toBeInTheDocument();
  expect(within(actions).getByRole("region", { name: "Bound action origin" })).toHaveTextContent("command-548");
  expect(within(actions).getByRole("region", { name: "Bound action origin" })).toHaveTextContent("artifact-550");
  expect(within(actions).getByRole("button", { name: /Open originating conversation/ })).toBeEnabled();
  expect(within(actions).getByRole("region", { name: "Compensation history" })).toHaveTextContent("00000000-0000-4000-8000-000000000552");
  expect(within(actions).getByRole("button", { name: "Canonical action evidence" })).toBeInTheDocument();
  fireEvent.click(within(actions).getByRole("button", { name: "Canonical action evidence" }));
  const evidence = within(actions).getByRole("textbox", { name: /Canonical action .* evidence/ });
  expect(evidence).toHaveAttribute("aria-readonly", "true");
  expect(evidence).toHaveTextContent("mcg.action_classified.v1");
  expect(evidence).toHaveTextContent("correlation_id");
  fireEvent.click(within(actions).getByRole("button", { name: /Inspect compensation/ }));
  await waitFor(() => expect(client.protectedRootAction).toHaveBeenLastCalledWith("00000000-0000-4000-8000-000000000552"));
  await waitFor(() => expect(actions.querySelectorAll(".canonical-action-row")).toHaveLength(2));
  fireEvent.click(within(actions).getAllByRole("button", { name: /Open approval, agent, command, and evidence/ })[0]);
  await waitFor(() => expect(client.getApproval).toHaveBeenCalledWith(approvalId));
  const source = await screen.findByRole("region", { name: "Approval source evidence" });
  expect(source).toHaveTextContent("Exact approval source");
  expect(source).toHaveTextContent("Coding Agent Atlas");
  expect(source).toHaveTextContent("cargo test");
  expect(source).toHaveTextContent("command-548:params");
  expect(source).toHaveTextContent("777777777777");
  fireEvent.click(within(source).getByRole("button", { name: /Open exact Runtime invocation/ }));
  await waitFor(() => expect(client.protectedRootInvocation).toHaveBeenCalledWith(invocationId));
  const invocationFocus = await screen.findByRole("status", { name: "Focused approval invocation" });
  expect(invocationFocus).toHaveTextContent(invocationId);
  expect(invocationFocus).toHaveTextContent("governed-provider / approval-model");
  client.protectedRootInvocation.mockResolvedValueOnce({
    ...invocationInspection,
    manifest_hash: { ...invocationHash, digest_hex: "0".repeat(64) }
  });
  const actionsAfterFocus = screen.getByRole("article", { name: "Governed actions" });
  fireEvent.click(within(actionsAfterFocus).getAllByRole("button", { name: /Open exact Runtime invocation/ })[0]);
  const invocationsAfterMismatch = screen.getByRole("article", { name: "Runtime invocations" });
  expect(await within(invocationsAfterMismatch).findByRole("alert")).toHaveTextContent("no longer matches its immutable approval binding");
});

test("Advanced initiates only the exact owner-confirmed compensation through Protected Root", async () => {
  const actionId = "00000000-0000-4000-8000-000000000570";
  const executionId = "00000000-0000-4000-8000-000000000571";
  const workflowId = "00000000-0000-4000-8000-000000000572";
  const attemptId = "00000000-0000-4000-8000-000000000573";
  const compensationId = "00000000-0000-4000-8000-000000000574";
  const now = "2026-08-31T12:00:00Z";
  const payload = { type_id: "mcg.governed_filesystem_operation.v1", schema_version: 1, canonical_encoding: "canonical-cbor-v1", normalization: "canonical-json-v1", bytes: [1, 2, 3] };
  const execution = { id: executionId, action_id: actionId, attempt_number: 1, state: "committed", revision: 7, lease_generation: 6, executor: null, outcome: { outcome: "effect_confirmed", evidence: [] }, created_at: now, updated_at: now };
  const summary = { action_id: actionId, execution_id: executionId, workflow_id: workflowId, attempt_id: attemptId, actor: "00000000-0000-4000-8000-000000000575", effect_class: "reversible", state: "committed", outcome: execution.outcome, revision: 7, lease_generation: 6, updated_at: now };
  const inspection = {
    envelope: { id: actionId, workflow_id: workflowId, attempt_id: attemptId, actor: summary.actor, effect_class: "reversible", operation: payload, operation_hash: { algorithm: "blake3-256", domain: "MetaCog.ActionOperation", digest_hex: "a".repeat(64) }, compensation: payload, compensates_action: null, created_at: now },
    compensation_actions: [],
    executions: [execution],
    receipts: [],
    reconciliation_jobs: [],
    events: []
  };
  const compensation = {
    ...inspection,
    envelope: { ...inspection.envelope, id: compensationId, compensation: null, compensates_action: actionId },
    executions: [{ ...execution, id: "00000000-0000-4000-8000-000000000576", action_id: compensationId, state: "effect_confirmed", revision: 4, lease_generation: 3 }]
  };
  const workflow = { workflow_id: workflowId, attempt_id: attemptId, task_contract: { id: "00000000-0000-4000-8000-000000000577", version: 1, content_hash: { algorithm: "blake3-256", domain: "MetaCog.TaskContract", digest_hex: "b".repeat(64) } }, parent_attempt_id: null, attempt_number: 1, state: "running", revision: 11, lease_owner: summary.actor, lease_generation: 9, lease_expires_at: "2026-08-31T12:10:00Z", heartbeat_at: now, checkpoint: null, environment_snapshot: { algorithm: "blake3-256", domain: "MetaCog.Environment", digest_hex: "c".repeat(64) }, configuration: { id: "00000000-0000-4000-8000-000000000578", version: 1, content_hash: { algorithm: "blake3-256", domain: "MetaCog.Configuration", digest_hex: "d".repeat(64) } }, idempotency_key: "workflow", retry_policy: { max_attempts: 1, initial_backoff_ms: 100, maximum_backoff_ms: 100 }, compensation_policy: {}, created_at: now, updated_at: now };
  client.getSettings.mockResolvedValue({ schema_version: 1, revision: 1, interface_complexity: "advanced", autonomy: "autonomous", shadow_experiments: false, updated_at: now });
  client.protectedRootActions.mockResolvedValue([summary]);
  client.protectedRootAction.mockImplementation(async (id: string) => id === actionId ? inspection : null);
  client.protectedRootWorkflow.mockResolvedValue({ attempt: workflow, task_contract: { id: workflow.task_contract.id, version: 1, objective: "Create then compensate an exact file", requirements: [], constraints: [], assumptions: [], acceptance_criteria: [], status: "active", created_at: now }, events: [] });
  client.compensateProtectedRootAction.mockResolvedValue({ original_action_id: actionId, original_execution: execution, compensation, replayed: false });
  const confirmation = vi.spyOn(window, "confirm").mockReturnValue(true);

  render(<App />);
  fireEvent.click(await screen.findByRole("link", { name: /Agents/ }));
  const actions = await screen.findByRole("article", { name: "Governed actions" });
  fireEvent.click(within(actions).getByRole("button", { name: "Inspect" }));
  const reason = await within(actions).findByRole("textbox", { name: `Compensation reason for action ${actionId}` });
  fireEvent.change(reason, { target: { value: "Remove the exact file because the owner no longer needs it." } });
  fireEvent.click(within(actions).getByRole("button", { name: "Initiate exact compensation" }));
  await waitFor(() => expect(client.compensateProtectedRootAction).toHaveBeenCalledTimes(1));
  const request = client.compensateProtectedRootAction.mock.calls[0][0];
  expect(request).toMatchObject({
    action_id: actionId,
    execution_id: executionId,
    expected_action_revision: 7,
    expected_action_generation: 6,
    expected_workflow_revision: 11,
    expected_workflow_generation: 9,
    reason: "Remove the exact file because the owner no longer needs it.",
    confirmed: true,
    idempotency_key: expect.stringMatching(/^desktop-owner-compensation-/)
  });
  expect(request).not.toHaveProperty("operation");
  expect(request).not.toHaveProperty("authority");
  expect(request).not.toHaveProperty("capability_root");
  expect(await within(actions).findByRole("status")).toHaveTextContent(`Compensation ${compensationId} was initiated from the exact bound plan.`);
  confirmation.mockRestore();
});

test("Runtime invocations preserve one Root truth across Simple Advanced and Developer", async () => {
  const invocationId = "00000000-0000-4000-8000-000000000561";
  const workflowId = "00000000-0000-4000-8000-000000000562";
  const attemptId = "00000000-0000-4000-8000-000000000563";
  const updatedAt = "2026-08-30T14:00:00Z";
  const hash = (domain: string, digit: string) => ({ algorithm: "blake3-256", domain, digest_hex: digit.repeat(64) });
  const taskContract = { id: "00000000-0000-4000-8000-000000000564", version: 4, content_hash: hash("MetaCog.TaskContract", "a") };
  const summary = {
    invocation_id: invocationId,
    workflow_id: workflowId,
    attempt_id: attemptId,
    role: "work",
    task_contract: taskContract,
    model_run_id: "00000000-0000-4000-8000-000000000565",
    model_run_state: "completed",
    model_run_revision: 3,
    persisted_at: updatedAt,
    updated_at: updatedAt
  };
  const inspection = {
    manifest: {
      id: invocationId,
      role: "work",
      attempt_id: attemptId,
      task_contract: taskContract,
      runtime_identity: { instance_id: "runtime-generation-7", build_hash: hash("MetaCog.RuntimeExecutable", "b"), protocol_version: 1, started_at: updatedAt },
      resolved_provider: { source_type: "mcg.provider.v1", locator: "governed-provider", content_hash: hash("MetaCog.Provider", "c") },
      resolved_model: { source_type: "mcg.model.v1", locator: "deterministic-work-v1", content_hash: hash("MetaCog.Model", "d") },
      observed_model_fingerprint: "deterministic-fixture-1",
      runtime_build: hash("MetaCog.RuntimeBuild", "e"),
      request_hash: hash("MetaCog.ModelRequest", "f"),
      response_hash: hash("MetaCog.ModelResponse", "1")
    },
    manifest_hash: hash("MetaCog.InvocationManifest", "2"),
    context_manifest: {
      id: "00000000-0000-4000-8000-000000000566",
      attempt_id: attemptId,
      selected_sources: [{ source: { source_type: "mcg.task-contract.v1", locator: taskContract.id, content_hash: taskContract.content_hash }, perspective: null, placement: 0, approximate_tokens: 144 }],
      approximate_tokens: 144
    },
    context_manifest_hash: hash("MetaCog.ContextManifest", "3"),
    model_runs: [{
      revision: 3,
      content_hash: hash("MetaCog.ModelRun", "4"),
      run: {
        id: summary.model_run_id,
        invocation_id: invocationId,
        role: "work",
        state: "completed",
        provider_request_id: "provider-request-567",
        response_artifact: { content_hash: hash("MetaCog.RuntimeResponse", "5"), media_type: "application/vnd.metacog.runtime-response+json", byte_length: 512 },
        observed_model_fingerprint: "MetaCog.ObservedModelFingerprint:sha256:observed-fixture-567",
        outcome_evidence: { latency_milliseconds: 1250, actual_cost_microusd: 18, cost_provenance: { kind: "provider_catalog", pricing_source: { source_type: "mcg.model.v1", locator: "fixture-priced-model", content_hash: hash("MetaCog.Model", "6") } }, error_class: null },
        started_at: updatedAt,
        finished_at: "2026-08-30T14:00:01Z"
      },
      recorded_at: "2026-08-30T14:00:01Z"
    }],
    model_runs_truncated: false
  };
  client.protectedRootInvocations.mockResolvedValue([summary]);
  client.protectedRootInvocation.mockResolvedValue(inspection);

  render(<App />);
  fireEvent.click(await screen.findByRole("link", { name: /Agents/ }));
  let invocations = await screen.findByRole("article", { name: "Runtime invocations" });
  expect(within(invocations).getByText("Work run")).toBeInTheDocument();
  expect(within(invocations).getByText("Completed")).toBeInTheDocument();
  expect(within(invocations).queryByText(invocationId)).not.toBeInTheDocument();
  expect(within(invocations).queryByRole("button", { name: "Inspect" })).not.toBeInTheDocument();

  await enableAdvancedMode();
  invocations = screen.getByRole("article", { name: "Runtime invocations" });
  expect(within(invocations).getByText(/Completed · revision 3/)).toBeInTheDocument();
  expect(within(invocations).getByText(new RegExp(invocationId))).toBeInTheDocument();
  fireEvent.click(within(invocations).getByRole("button", { name: "Inspect" }));
  expect(await within(invocations).findByText(/governed-provider/)).toBeInTheDocument();
  expect(within(invocations).getByText(/deterministic-work-v1/)).toBeInTheDocument();
  expect(within(invocations).getByText(/observed model/)).toHaveTextContent(/MetaCog\.ObservedModel/);
  expect(within(invocations).getByText(/Provider latency/)).toHaveTextContent(/1,250 ms/);
  expect(within(invocations).getByText(/Provider latency/)).toHaveTextContent(/\$0\.000018/);
  expect(within(invocations).getByText(/1 governed context source · approximately 144 tokens/)).toBeInTheDocument();
  expect(within(invocations).queryByRole("button", { name: "Canonical invocation evidence" })).not.toBeInTheDocument();

  const complexity = screen.getByRole("button", { name: "Cycle interface complexity" });
  fireEvent.click(complexity);
  await waitFor(() => expect(complexity).toHaveTextContent("Developer"));
  invocations = screen.getByRole("article", { name: "Runtime invocations" });
  fireEvent.click(within(invocations).getByRole("button", { name: "Canonical invocation evidence" }));
  const evidence = within(invocations).getByRole("textbox", { name: `Canonical invocation ${invocationId} evidence` });
  expect(evidence).toHaveAttribute("aria-readonly", "true");
  expect(evidence).toHaveTextContent("provider-request-567");
  expect(evidence).toHaveTextContent("MetaCog.ModelRequest");
  expect(evidence).toHaveTextContent("response_artifact");
  expect(evidence).toHaveTextContent("MetaCog.ObservedModelFingerprint:sha256:observed-fixture-567");
  expect(evidence).toHaveTextContent("outcome_evidence");
  expect(evidence).toHaveTextContent("actual_cost_microusd");

  fireEvent.click(complexity);
  await waitFor(() => expect(complexity).toHaveTextContent("Simple"));
  await waitFor(() => expect(document.querySelector(".app-shell.simple-mode")).toBeInTheDocument());
  invocations = screen.getByRole("article", { name: "Runtime invocations" });
  expect(within(invocations).getByText("Completed")).toBeInTheDocument();
  expect(within(invocations).queryByRole("button", { name: "Inspect" })).not.toBeInTheDocument();
  expect(within(invocations).queryByRole("button", { name: "Canonical invocation evidence" })).not.toBeInTheDocument();
  expect(client.protectedRootInvocation).toHaveBeenCalledWith(invocationId);
});

test("Developer searches the global cross-conversation agent inventory without changing its truth", async () => {
  const updatedAt = "2026-08-30T15:00:00Z";
  const sessions = [
    {
      schema_version: 1,
      session_id: "00000000-0000-4000-8000-000000000571",
      conversation_id: "00000000-0000-4000-8000-000000000572",
      conversation_title: "Release qualification",
      conversation_state: "active",
      role_id: "00000000-0000-4000-8000-000000000573",
      organization_revision_id: "00000000-0000-4000-8000-000000000574",
      kind: "coder",
      display_name: "Coding Agent Atlas",
      state: "working",
      codex_thread_id: "thread-atlas",
      model: "gpt-5.6-sol",
      created_at: updatedAt,
      updated_at: updatedAt
    },
    {
      schema_version: 1,
      session_id: "00000000-0000-4000-8000-000000000575",
      conversation_id: "00000000-0000-4000-8000-000000000576",
      conversation_title: "Security verification",
      conversation_state: "active",
      role_id: "00000000-0000-4000-8000-000000000577",
      organization_revision_id: "00000000-0000-4000-8000-000000000578",
      kind: "tester",
      display_name: "Tester",
      state: "ready",
      created_at: updatedAt,
      updated_at: updatedAt
    }
  ];
  client.getSettings.mockResolvedValue({ schema_version: 1, revision: 1, interface_complexity: "developer", autonomy: "autonomous", shadow_experiments: true, updated_at: updatedAt });
  client.listGlobalAgentSessions.mockResolvedValue({ sessions });

  render(<App />);
  fireEvent.click(await screen.findByRole("link", { name: /Agents/ }));
  const browser = await screen.findByRole("article", { name: "Compatibility agent sessions" });
  expect(within(browser).getByText("Coding Agent Atlas")).toBeInTheDocument();
  expect(within(browser).getByText("Tester")).toBeInTheDocument();
  expect(within(browser).getByText(/Codex thread thread-atlas/)).toBeInTheDocument();
  expect(within(browser).getAllByRole("button", { name: "Open conversation" })).toHaveLength(2);

  fireEvent.change(within(browser).getByRole("searchbox", { name: "Search global agent sessions" }), { target: { value: "security" } });
  expect(within(browser).queryByText("Coding Agent Atlas")).not.toBeInTheDocument();
  expect(within(browser).getByText("Tester")).toBeInTheDocument();
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

test("removed-project task evidence is not resurrected as a General chat", async () => {
  client.listProjects.mockResolvedValue({ projects: [], archived_projects: [] });
  client.listTasks.mockResolvedValue({ tasks: [{
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000613",
    project_id: "00000000-0000-4000-8000-000000000614",
    goal: "Legacy task from a removed project",
    constraints: [],
    success_conditions: [],
    risk: "medium",
    budget: {},
    state: "blocked",
    created_at: "2026-08-21T00:00:00Z",
    updated_at: "2026-08-21T00:00:00Z"
  }] });

  render(<App />);

  const projectTree = screen.getByLabelText("Projects");
  await waitFor(() => expect(client.listTasks).toHaveBeenCalled());
  expect(within(projectTree).queryByRole("button", { name: "General" })).not.toBeInTheDocument();
  expect(within(projectTree).queryByRole("button", { name: "Legacy task from a removed project" })).not.toBeInTheDocument();
});

test("project sidebar icons archive, restore, and safely remove catalog entries", async () => {
  const active = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000620",
    project_id: "00000000-0000-4000-8000-000000000621",
    root: "C:\\Projects\\active-app",
    fingerprint: "a".repeat(64),
    observed_at: "2026-08-24T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false, recent_commits: [] },
    languages: [], build_systems: [], test_systems: [], package_managers: [], services: [], ci_systems: [], important_directories: [], commands: [],
    instructions: { conflicts: [] }, world: { nodes: [], relationships: [] }, watched_inputs: {}
  };
  const archived = { ...active, id: "00000000-0000-4000-8000-000000000622", project_id: "00000000-0000-4000-8000-000000000623", root: "C:\\Projects\\archived-app" };
  client.listProjects.mockResolvedValue({ projects: [active], archived_projects: [archived] });
  client.archiveProject.mockImplementation(async (projectId: string) => projectId === active.project_id ? active : archived);
  client.removeProject.mockResolvedValue(undefined);
  vi.spyOn(window, "confirm").mockReturnValue(true);

  render(<App />);
  const projectTree = screen.getByLabelText("Projects");
  await within(projectTree).findByRole("button", { name: "active-app" });

  fireEvent.click(within(projectTree).getByRole("button", { name: "Archive active-app" }));
  await waitFor(() => expect(client.archiveProject).toHaveBeenCalledWith(active.project_id, true));
  expect(within(projectTree).queryByRole("button", { name: "active-app" })).not.toBeInTheDocument();

  fireEvent.click(within(projectTree).getByRole("button", { name: "Archived projects" }));
  fireEvent.click(within(projectTree).getByRole("button", { name: "Restore archived-app" }));
  await waitFor(() => expect(client.archiveProject).toHaveBeenCalledWith(archived.project_id, false));

  fireEvent.click(within(projectTree).getByRole("button", { name: "Delete archived-app" }));
  await waitFor(() => expect(client.removeProject).toHaveBeenCalledWith(archived.project_id));
  expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining("folder, local Git history, and GitHub repository will not be deleted"));
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

  let projectMenu = screen.getByRole("menu", { name: "Project chat menu" });
  expect(within(projectMenu).getByRole("menuitem", { name: "Pin" })).toBeInTheDocument();
  expect(within(projectMenu).queryByRole("menuitem", { name: "Move up" })).not.toBeInTheDocument();
  fireEvent.pointerDown(screen.getByRole("button", { name: "New chat" }));
  expect(screen.queryByRole("menu", { name: "Project chat menu" })).not.toBeInTheDocument();
  fireEvent.contextMenu(projectChat, { clientX: 220, clientY: 200 });
  projectMenu = screen.getByRole("menu", { name: "Project chat menu" });
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
    detail: "The packaged service is missing. Repair or reinstall MetaCog."
  });
  render(<App />);
  const status = (await screen.findByText("Local runtime needs attention")).closest("[role='status']");
  expect(status).toHaveAttribute("title", "The packaged service is missing. Repair or reinstall MetaCog.");
});

test("owner settings persist autonomy with optimistic revision control", async () => {
  render(<App />);
  const panel = await screen.findByLabelText("Owner settings");
  expect(within(panel).getByLabelText("Autonomy")).toHaveValue("autonomous");
  fireEvent.change(within(panel).getByLabelText("Autonomy"), { target: { value: "guided" } });
  fireEvent.click(within(panel).getByRole("button", { name: "Save settings" }));
  await waitFor(() => expect(client.updateSettings).toHaveBeenCalledWith({
    expected_revision: 1,
    interface_complexity: "simple",
    autonomy: "guided",
    shadow_experiments: true,
    reason: "Updated owner settings from desktop"
  }));
  expect(await screen.findByText("Revision 2")).toBeInTheDocument();
  expect(within(panel).getByText("Ask before consequential local actions.")).toBeInTheDocument();
});

test("interface complexity exposes three presentation levels without changing runtime settings", async () => {
  render(<App />);
  const panel = await screen.findByLabelText("Owner settings");
  const advancedChoice = within(panel).getByRole("radio", { name: /Advanced/ });
  fireEvent.click(advancedChoice);
  await waitFor(() => expect(client.updateSettings).toHaveBeenLastCalledWith({
    expected_revision: 1,
    interface_complexity: "advanced",
    autonomy: "autonomous",
    shadow_experiments: true,
    reason: "Enabled advanced interface"
  }));

  const developerChoice = within(panel).getByRole("radio", { name: /Developer/ });
  await waitFor(() => expect(developerChoice).toBeEnabled());
  fireEvent.click(developerChoice);
  await waitFor(() => expect(client.updateSettings).toHaveBeenLastCalledWith({
    expected_revision: 2,
    interface_complexity: "developer",
    autonomy: "autonomous",
    shadow_experiments: true,
    reason: "Enabled developer interface"
  }));
  expect(within(panel).getByText(/Developer mode remains governed/)).toBeInTheDocument();
  expect(within(panel).getByRole("button", { name: "Runtime contract inspector" })).toBeInTheDocument();
  const runtimeContract = within(panel).getByRole("textbox", { name: "Current runtime settings contract" });
  expect(runtimeContract).toHaveAttribute("aria-readonly", "true");
  expect(runtimeContract).toHaveTextContent("protected-root");
  expect(runtimeContract).toHaveTextContent("pre-root.mcgb");
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
    detail: "MetaCog will accept only packages signed by this build's trusted update key.",
    recovery_notice: "The update did not complete. MetaCog restarted the previous installation and preserved its data."
  });
  client.checkForUpdate.mockResolvedValue({
    schema_version: 1,
    configured: true,
    current_version: "0.1.0",
    available_version: "0.2.0",
    notes: "Security and recovery improvements.",
    published_at: "2026-08-18T12:00:00Z",
    target: "windows-x86_64-nsis",
    detail: "A newer owner-signed MetaCog release is available."
  });
  render(<App />);
  const panel = await screen.findByLabelText("Application updates");
  expect(within(panel).getByRole("status")).toHaveTextContent(/restarted the previous installation/);
  fireEvent.click(within(panel).getByRole("button", { name: "Check for updates" }));
  expect(await within(panel).findByText("MetaCog 0.2.0")).toBeInTheDocument();
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
  expectNewChatProject("site");
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
  expect(within(panel).getByRole("button", { name: "Open in MetaCog browser" })).toBeVisible();
  fireEvent.click(within(panel).getByRole("button", { name: "More options for Springfield IT Support" }));
  expect(screen.getByRole("menuitem", { name: "Open in MetaCog browser" })).toBeVisible();
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

  await enableAdvancedMode();
  expect(within(panel).getByRole("heading", { name: "Springfield's Best IT" })).toBeVisible();
  expect(within(panel).queryByRole("heading", { name: "Web properties" })).not.toBeInTheDocument();
  expect(within(panel).queryByLabelText("Hosting provider")).not.toBeInTheDocument();

  const card = within(panel).getByRole("heading", { name: "Springfield's Best IT" }).closest("article");
  expect(card).not.toBeNull();
  fireEvent.contextMenu(card!);
  fireEvent.click(screen.getByRole("menuitem", { name: "Open in MetaCog browser" }));
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
    "The ChatGPT Sites library could not refresh: Account inventory unavailable"
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

  expectNewChatProject("review-me");
  const goal = (screen.getByLabelText("Message") as HTMLTextAreaElement).value;
  expect(goal).toContain("Review code changes in review-me");
  expect(goal).toContain("all staged, unstaged, and untracked changes");
  expect(goal).toContain("Do not modify the working tree during the review");
});

test("Pull requests prepares a Codex cloud task in isolation before explicit application", async () => {
  const project = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000701",
    project_id: "00000000-0000-4000-8000-000000000702",
    root: "C:\\Projects\\review-me",
    fingerprint: "fixture",
    observed_at: "2026-08-29T00:00:00Z",
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
  client.listProjects.mockResolvedValue({ projects: [project], archived_projects: [] });
  render(<App />);

  const panel = await screen.findByRole("region", { name: "Pull requests" });
  fireEvent.click(within(within(panel).getByLabelText("Code change actions")).getByRole("button", { name: "Apply cloud task" }));
  expect(within(panel).getByRole("heading", { name: "Apply a Codex cloud task" })).toBeVisible();
  expect(within(panel).getByText(/project is unchanged while MetaCog prepares/i)).toBeVisible();

  fireEvent.change(within(panel).getByLabelText("Repository"), { target: { value: project.project_id } });
  await waitFor(() => expect(within(panel).getByLabelText("Codex cloud task ID")).toBeEnabled());
  fireEvent.change(within(panel).getByLabelText("Codex cloud task ID"), { target: { value: "cloud-task-123" } });
  const checkChanges = within(panel).getByRole("button", { name: "Check changes" });
  await waitFor(() => expect(checkChanges).toBeEnabled());
  fireEvent.click(checkChanges);

  await waitFor(() => expect(client.prepareCloudDiffApply).toHaveBeenCalledWith(project.project_id, "cloud-task-123"));
  expect(await within(panel).findByText("2 changed files")).toBeVisible();
  expect(within(panel).getByText("src/app.ts")).toBeVisible();
  expect(within(panel).getByText("src/app.test.ts")).toBeVisible();
  expect(within(panel).getByText(/will apply this exact validated diff/i)).toBeVisible();

  fireEvent.click(within(panel).getByRole("button", { name: "Discard prepared changes" }));
  await waitFor(() => expect(client.discardCloudDiffApply).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000ca1",
    3
  ));
  expect(within(panel).queryByText("src/app.ts")).not.toBeInTheDocument();
  await waitFor(() => expect(within(panel).getByLabelText("Codex cloud task ID")).toBeEnabled());
  fireEvent.change(within(panel).getByLabelText("Codex cloud task ID"), { target: { value: "cloud-task-123" } });
  const checkAgain = within(panel).getByRole("button", { name: "Check changes" });
  await waitFor(() => expect(checkAgain).toBeEnabled());
  fireEvent.click(checkAgain);
  await waitFor(() => expect(client.prepareCloudDiffApply).toHaveBeenCalledTimes(2));

  fireEvent.click(within(panel).getByRole("button", { name: "Apply changes" }));
  await waitFor(() => expect(client.applyCloudDiff).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000ca1",
    3
  ));
  expect(await within(panel).findByText("Changes applied to the working tree.")).toBeVisible();
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
  fireEvent.click(within(panel).getByText("Technical review controls"));
  expect(within(panel).getByRole("heading", { name: "Governed approvals" })).toBeVisible();
  expect(within(panel).getByText("Runtime permission requests are separate from pull-request reviews.")).toBeVisible();
  expect(within(panel).getByText("Install a requested dependency")).toBeVisible();
});

test("advanced Sites remains the ChatGPT Sites library without generic provider controls", async () => {
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
  expect(within(panel).getByRole("heading", { name: "Sites" })).toBeVisible();
  expect(within(panel).getByLabelText("Starting point")).toBeVisible();
  expect(within(panel).queryByRole("heading", { name: "Web properties" })).not.toBeInTheDocument();
  expect(within(panel).queryByLabelText("Hosting provider")).not.toBeInTheDocument();
  expect(client.registerSite).not.toHaveBeenCalled();
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
  expectNewChatProject("scheduled");
  expect((screen.getByLabelText("Message") as HTMLTextAreaElement).value).toContain("Summarize important changes every morning");
  expect((screen.getByLabelText("Message") as HTMLTextAreaElement).value).toContain("Return to this setup chat on each run");

  await enableAdvancedMode();
  fireEvent.click(within(scheduler).getByText("Exact scheduler controls"));
  expect(within(scheduler).getByRole("heading", { name: "Scheduler administration" })).toBeVisible();
  fireEvent.change(within(scheduler).getByLabelText("Trigger"), { target: { value: "cron" } });
  expect(within(scheduler).getByLabelText("Five-field cron")).toHaveValue("0 2 * * *");
  fireEvent.change(within(scheduler).getByLabelText("Name"), { target: { value: schedule.name } });
  fireEvent.change(within(scheduler).getByLabelText("Scheduled goal"), { target: { value: schedule.goal } });
  fireEvent.click(within(scheduler).getByRole("button", { name: "Create schedule" }));

    expect(
      (await within(scheduler).findAllByRole("heading", { name: schedule.name })).length,
    ).toBeGreaterThan(0);
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
  fireEvent.click(within(scheduler).getByText("Exact scheduler controls"));
  fireEvent.change(within(scheduler).getByLabelText("Name"), { target: { value: completed.name } });
  fireEvent.change(within(scheduler).getByLabelText("Scheduled goal"), { target: { value: completed.goal } });
  const create = within(scheduler).getByRole("button", { name: "Create schedule" });
  fireEvent.click(create);
  expect(await within(scheduler).findByRole("alert")).toHaveTextContent("response lost");
  const firstIdentity = client.createSchedule.mock.calls[0][0].schedule_id;

  fireEvent.click(create);
    expect(
      (await within(scheduler).findAllByRole("heading", { name: completed.name })).length,
    ).toBeGreaterThan(0);
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

test("advanced mode upgrades legacy task preferences without crashing", async () => {
  const taskId = "00000000-0000-4000-8000-000000000623";
  window.localStorage.setItem("mcg.chat-run-preferences.tasks", JSON.stringify({
    [taskId]: { reasoning_effort: "medium", speed: "standard", approval_mode: "ask" }
  }));
  client.listTasks.mockResolvedValue({ tasks: [{
    schema_version: 1,
    id: taskId,
    goal: "Legacy task preferences",
    constraints: [],
    success_conditions: [],
    risk: "low",
    budget: {},
    state: "pending",
    created_at: "2026-08-21T00:00:00Z",
    updated_at: "2026-08-21T00:00:00Z"
  }] });
  render(<App />);

  await enableAdvancedMode();
  expect((await screen.findAllByText("Legacy task preferences")).length).toBeGreaterThan(0);
  expect(screen.getAllByText("Automatic").length).toBeGreaterThan(0);
  await waitFor(() => expect(JSON.parse(window.localStorage.getItem("mcg.chat-run-preferences.tasks") ?? "{}")[taskId].team_preset).toBe("automatic"));
});

test("advanced Agents exposes disabled Governor guardrails and owner-reviewed proposals", async () => {
  const proposal = {
    schema_version: 1,
    id: "governor-proposal-1",
    organization_id: "00000000-0000-4000-8000-000000000901",
    baseline_revision_id: "00000000-0000-4000-8000-000000000902",
    target: "routing_policy",
    risk: "moderate",
    state: "awaiting_owner_approval",
    title: "Reduce redundant Coordinator wakes",
    diagnosis: "Routine progress events wake the Coordinator too often.",
    proposed_change: "Deduplicate routine progress and retain terminal callbacks.",
    expected_benefits: ["Lower token usage"],
    expected_risks: ["A missed wake could delay delivery"],
    success_criteria: ["Exactly one final Chat callback"],
    observation_ids: [],
    validation_findings: ["Authority boundaries remain unchanged"],
    expires_at: "2026-09-25T00:00:00Z",
    created_at: "2026-08-25T00:00:00Z",
    updated_at: "2026-08-25T00:00:00Z"
  };
  client.listGovernorProposals.mockResolvedValue({ proposals: [proposal] });
  client.decideGovernorProposal.mockResolvedValue({ proposal: { ...proposal, state: "approved" }, replayed: false });
  render(<App />);

  await enableAdvancedMode();
  fireEvent.click(await screen.findByRole("button", { name: "Agents overview" }));
  fireEvent.click(await screen.findByRole("button", { name: "Governor" }));

  expect(screen.getByRole("heading", { name: "Governor guardrails" })).toBeInTheDocument();
  expect(screen.getByText("disabled")).toBeInTheDocument();
  expect(screen.getByText("Reduce redundant Coordinator wakes")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Approve" }));
  await waitFor(() => expect(client.decideGovernorProposal).toHaveBeenCalledWith("governor-proposal-1", true));
});

test("mode persistence restores keyboard focus after the temporarily disabled control returns", async () => {
  render(<App />);

  const draft = await screen.findByRole("textbox", { name: "Message" });
  fireEvent.change(draft, { target: { value: "Keep this ordinary draft while changing density" } });
  const advanced = await screen.findByRole("button", { name: "Cycle interface complexity" });
  await waitFor(() => expect(advanced).toBeEnabled());
  advanced.focus();
  expect(advanced).toHaveFocus();
  fireEvent.click(advanced);

  const simple = await screen.findByRole("button", { name: "Cycle interface complexity" });
  await waitFor(() => expect(simple).toBeEnabled());
  await waitFor(() => expect(simple).toHaveFocus());
  expect(screen.getByRole("textbox", { name: "Message" })).toHaveValue("Keep this ordinary draft while changing density");

  fireEvent.click(simple);
  await waitFor(() => expect(simple).toHaveTextContent("Developer"));
  fireEvent.click(simple);
  await waitFor(() => expect(screen.getByRole("button", { name: "Cycle interface complexity" })).toHaveTextContent("Simple"));
  expect(screen.getByRole("textbox", { name: "Message" })).toHaveValue("Keep this ordinary draft while changing density");
});

test("advanced mode keeps one routed workspace surface and exposes a persistent simple-mode escape", async () => {
  render(<App />);

  await enableAdvancedMode();
  const toolbar = screen.getByRole("button", { name: "Cycle interface complexity" });
  expect(toolbar).toHaveTextContent("Advanced");
  expect(screen.getByRole("heading", { name: "What should we build?" })).toBeInTheDocument();
  expect(screen.getAllByRole("textbox", { name: "Message" })).toHaveLength(1);
  expect(screen.queryByText("Constraints (one per line)")).not.toBeInTheDocument();
  expect(screen.queryByText("Success conditions (one per line)")).not.toBeInTheDocument();
  expect(screen.queryByText("Maximum cost (USD, optional)")).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("link", { name: "Pull requests" }));
  const pullRequests = await screen.findByRole("region", { name: "Pull requests" });
  const newChat = document.querySelector(".hero-card") as HTMLElement;
  const taskPanel = document.querySelector(".task-panel") as HTMLElement;
  expect(pullRequests).not.toHaveAttribute("hidden");
  expect(newChat).toHaveAttribute("hidden");
  expect(taskPanel).toHaveAttribute("hidden");

  fireEvent.click(toolbar);
  await waitFor(() => expect(toolbar).toHaveTextContent("Developer"));
  fireEvent.click(toolbar);
  await waitFor(() => expect(document.querySelector(".app-shell.simple-mode")).toBeInTheDocument());
  expect(client.updateSettings).toHaveBeenLastCalledWith(expect.objectContaining({ interface_complexity: "simple" }));
});

test("new chat presents project, workspace, branch, add, permission, and run settings as contextual controls", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000020",
    project_id: "00000000-0000-4000-8000-000000000021",
    root: "C:\\Projects\\Metacognitive Governor",
    observed_at: "2026-08-26T00:00:00Z",
    repository: { is_git: true, branch: "codex/new-chat-surface", head: "0123456789abcdef", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  render(<App />);

  await selectNewChatProject("Metacognitive Governor");
  expect(screen.getByRole("heading", { name: "What should we build in Metacognitive Governor?" })).toBeInTheDocument();

  fireEvent.click(screen.getByLabelText("Workspace: Local"));
  const workspaceMenu = screen.getByRole("menu", { name: "Work in" });
  expect(within(workspaceMenu).getByRole("menuitemradio", { name: /Local/ })).toHaveAttribute("aria-checked", "true");
  expect(within(workspaceMenu).getByRole("menuitem", { name: /New local worktree/ })).toBeDisabled();
  expect(within(workspaceMenu).getByRole("menuitem", { name: /Cloud/ })).toBeDisabled();

  fireEvent.click(screen.getByLabelText("Branch: codex/new-chat-surface"));
  expect(within(screen.getByRole("menu", { name: "Git branch" })).getByRole("menuitemradio", { name: /codex\/new-chat-surface/ })).toHaveAttribute("aria-checked", "true");

  fireEvent.click(screen.getByLabelText("Add files and more"));
  const addMenu = screen.getByRole("menu", { name: "Add" });
  expect(within(addMenu).getByRole("menuitem", { name: /Goal/ })).toBeEnabled();
  expect(within(addMenu).getByRole("menuitem", { name: /Plan mode/ })).toBeEnabled();
  expect(within(addMenu).getByRole("menuitem", { name: /Apps/ })).toBeEnabled();
  expect(within(addMenu).getByRole("menuitem", { name: /Slash commands/ })).toBeEnabled();
  expect(within(addMenu).getByRole("menuitem", { name: /Open project folder/ })).toBeEnabled();
  expect(within(addMenu).getByRole("menuitem", { name: /Remove project/ })).toBeEnabled();
  expect(screen.getByLabelText("Change permissions")).toBeInTheDocument();
  expect(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]')).toBeInTheDocument();
});

test("advanced run settings add and persist governed writable folders", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000020",
    project_id: "00000000-0000-4000-8000-000000000021",
    root: "C:\\Projects\\sample",
    observed_at: "2026-08-28T00:00:00Z",
    repository: { is_git: true, branch: "main", head: "0123456789abcdef", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.pickWritableFolder.mockResolvedValue("C:\\Shared\\assets");
  render(<App />);

  await selectNewChatProject("sample");
  const { dialog } = openNewChatRunSettings();
  fireEvent.click(within(dialog).getByText("Advanced"));
  const addFolder = within(dialog).getByRole("button", { name: "Add folder" });
  expect(addFolder).toBeEnabled();
  fireEvent.click(addFolder);
  expect(await within(dialog).findByText("C:\\Shared\\assets")).toBeVisible();
  fireEvent.click(within(dialog).getByRole("button", { name: "Remove writable folder C:\\Shared\\assets" }));
  expect(within(dialog).queryByText("C:\\Shared\\assets")).not.toBeInTheDocument();
  fireEvent.click(addFolder);
  expect(await within(dialog).findByText("C:\\Shared\\assets")).toBeVisible();

  fireEvent.click(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement);
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Update shared assets" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ additional_writable_roots: ["C:\\Shared\\assets"] }) })
  ));
});

test("project-free chats cannot add writable folders", async () => {
  render(<App />);
  const { dialog } = openNewChatRunSettings();
  fireEvent.click(within(dialog).getByText("Advanced"));
  expect(within(dialog).getByRole("button", { name: "Add folder" })).toBeDisabled();
  expect(within(dialog).getByText("Choose a project before adding writable folders.")).toBeVisible();
});

test("advanced run settings persist safe Codex feature overrides per conversation", async () => {
  client.getCodexFeatures.mockResolvedValue([
    { name: "memories", stage: "beta", displayName: "Memories", description: "Use durable memories.", enabled: false, defaultEnabled: false },
    { name: "shell_tool", stage: "stable", displayName: "Shell tool", enabled: true, defaultEnabled: true }
  ]);
  render(<App />);

  let settings = openNewChatRunSettings().dialog;
  expect(within(settings).queryByText("Session features")).not.toBeInTheDocument();
  expect(within(settings).queryByText("Web search")).not.toBeInTheDocument();
  fireEvent.click(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement);
  await enableAdvancedMode();
  settings = openNewChatRunSettings().dialog;
  fireEvent.click(await within(settings).findByText("Session features"));
  fireEvent.change(within(settings).getByRole("searchbox", { name: "Search session features" }), { target: { value: "memor" } });
  expect(within(settings).getByText("Memories")).toBeVisible();
  expect(within(settings).queryByText("Shell tool")).not.toBeInTheDocument();
  fireEvent.change(within(settings).getByRole("combobox", { name: "Memories session override" }), { target: { value: "enabled" } });
  fireEvent.click(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement);
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Use memories for this turn" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ codex_feature_overrides: { memories: true } }) })
  ));
});

test("advanced run settings persist live web search per conversation", async () => {
  render(<App />);
  await enableAdvancedMode();
  const settings = openNewChatRunSettings().dialog;
  fireEvent.click(within(settings).getByText("Web search"));
  fireEvent.click(within(settings).getByRole("menuitemradio", { name: /Live/ }));

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Find current information" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ codex_config_overrides: [{ key: "web_search", value: "live" }] }) })
  ));
});

test("advanced run settings discover and persist Codex local providers", async () => {
  client.getCodexConfigurationStatus.mockResolvedValue({
    requirements: {},
    provider_capabilities: {},
    windows_sandbox: { status: "ready" },
    local_providers: {
      configured_provider: "ollama",
      providers: [
        { id: "ollama", display_name: "Ollama", default_model: "gpt-oss:20b", endpoint: "http://localhost:11434/v1", running: true, responses_supported: true, version: "0.13.4", models: ["gpt-oss:20b", "gpt-oss:120b"], detail: "Ready for Codex Responses API work" },
        { id: "lmstudio", display_name: "LM Studio", default_model: "openai/gpt-oss-20b", endpoint: "http://localhost:1234/v1", running: false, responses_supported: false, models: [], detail: "LM Studio is not responding" }
      ]
    }
  });
  render(<App />);
  await enableAdvancedMode();

  let settings = openNewChatRunSettings().dialog;
  fireEvent.click(within(settings).getByRole("button", { name: /^Provider/ }));
  fireEvent.click(within(settings).getByRole("menuitemradio", { name: /Ollama.*Running.*0\.13\.4/ }));
  expect(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]')).toHaveTextContent("gpt-oss:20b");

  settings = openNewChatRunSettings().dialog;
  fireEvent.click(within(settings).getByRole("button", { name: /^Model/ }));
  fireEvent.click(within(settings).getByRole("menuitemradio", { name: /gpt-oss:120b.*Ollama/ }));
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Use my local model" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ model_provider: "ollama", model_id: "gpt-oss:120b", service_tier: undefined }) })
  ));
});

test("advanced run settings persist typed Codex config and block governed keys", async () => {
  render(<App />);
  await enableAdvancedMode();
  const settings = openNewChatRunSettings().dialog;
  fireEvent.click(within(settings).getByText("Codex config"));
  fireEvent.change(within(settings).getByRole("textbox", { name: "Codex profile" }), { target: { value: "team.fast" } });
  const strictConfig = within(settings).getByRole("checkbox", { name: "Require strict Codex configuration" });
  expect(strictConfig).not.toBeChecked();
  fireEvent.click(strictConfig);
  expect(strictConfig).toBeChecked();

  fireEvent.change(within(settings).getByRole("textbox", { name: "Codex configuration key" }), { target: { value: "sandbox_mode" } });
  fireEvent.change(within(settings).getByRole("textbox", { name: "Codex configuration value" }), { target: { value: '"danger-full-access"' } });
  fireEvent.click(within(settings).getByRole("button", { name: "Add override" }));
  expect(within(settings).getByRole("alert")).toHaveTextContent("governed by MetaCog");

  fireEvent.change(within(settings).getByRole("textbox", { name: "Codex configuration key" }), { target: { value: "web_search" } });
  fireEvent.change(within(settings).getByRole("textbox", { name: "Codex configuration value" }), { target: { value: "true" } });
  fireEvent.click(within(settings).getByRole("button", { name: "Add override" }));
  expect(within(settings).getByRole("alert")).toHaveTextContent("cached, live, or disabled");

  fireEvent.change(within(settings).getByRole("textbox", { name: "Codex configuration key" }), { target: { value: "model_verbosity" } });
  fireEvent.change(within(settings).getByRole("textbox", { name: "Codex configuration value" }), { target: { value: '"low"' } });
  fireEvent.click(within(settings).getByRole("button", { name: "Add override" }));
  expect(within(settings).getByText("model_verbosity")).toBeVisible();
  expect(within(settings).getByText('"low"')).toBeVisible();

  fireEvent.click(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement);
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Use concise output" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ codex_profile: "team.fast", codex_config_overrides: [{ key: "model_verbosity", value: "low" }], strict_config: true }) })
  ));
});

test("advanced run settings validate and persist structured output schemas", async () => {
  render(<App />);
  await enableAdvancedMode();
  const settings = openNewChatRunSettings().dialog;
  fireEvent.click(within(settings).getByText("Output format"));
  const editor = within(settings).getByRole("textbox", { name: "Output JSON Schema" });

  fireEvent.change(editor, { target: { value: "[]" } });
  fireEvent.click(within(settings).getByRole("button", { name: "Apply schema" }));
  expect(within(settings).getByRole("alert")).toHaveTextContent("must be a JSON object");

  const schema = {
    type: "object",
    properties: { summary: { type: "string" } },
    required: ["summary"],
    additionalProperties: false
  };
  fireEvent.change(editor, { target: { value: JSON.stringify(schema) } });
  fireEvent.click(within(settings).getByRole("button", { name: "Apply schema" }));
  expect(within(settings).queryByRole("alert")).not.toBeInTheDocument();

  fireEvent.click(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement);
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Return a structured summary" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ output_schema: schema }) })
  ));
});

test("simple run settings omit strict Codex configuration", () => {
  render(<App />);
  const settings = openNewChatRunSettings().dialog;
  expect(within(settings).queryByText("Codex config", { exact: true })).not.toBeInTheDocument();
  expect(within(settings).queryByText("Output format", { exact: true })).not.toBeInTheDocument();
  expect(within(settings).queryByText("Provider", { exact: true })).not.toBeInTheDocument();
  expect(within(settings).queryByRole("textbox", { name: "Codex profile" })).not.toBeInTheDocument();
  expect(within(settings).queryByRole("checkbox", { name: "Require strict Codex configuration" })).not.toBeInTheDocument();
  expect(within(settings).queryByRole("textbox", { name: "Output JSON Schema" })).not.toBeInTheDocument();
});

test("goal and plan commands are available from Add and prepare the composer", () => {
  render(<App />);

  fireEvent.click(screen.getByLabelText("Add files and more"));
  fireEvent.click(within(screen.getByRole("menu", { name: "Add" })).getByRole("menuitem", { name: /Goal/ }));
  expect(screen.getByRole("textbox", { name: "Message" })).toHaveValue("/goal ");

  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "" } });
  fireEvent.click(screen.getByLabelText("Add files and more"));
  fireEvent.click(within(screen.getByRole("menu", { name: "Add" })).getByRole("menuitem", { name: /Plan mode/ }));
  expect(screen.getByRole("textbox", { name: "Message" })).toHaveValue("/plan ");
});

test("Codex apps use the shared dollar picker from Add, slash commands, and direct typing", async () => {
  render(<App />);
  const composer = screen.getByRole("textbox", { name: "Message" });

  fireEvent.click(screen.getByLabelText("Add files and more"));
  fireEvent.click(within(screen.getByRole("menu", { name: "Add" })).getByRole("menuitem", { name: /Apps/ }));
  expect(composer).toHaveValue("$");
  fireEvent.click(await within(screen.getByRole("listbox", { name: "Enabled apps" })).findByRole("option", { name: /Calendar/ }));
  expect(composer).toHaveValue("$calendar ");

  fireEvent.change(composer, { target: { value: "/apps ", selectionStart: 6 } });
  fireEvent.click(within(screen.getByRole("listbox", { name: "Enabled apps" })).getByRole("option", { name: /Calendar/ }));
  expect(composer).toHaveValue("$calendar ");

  fireEvent.change(composer, { target: { value: "Use $cal", selectionStart: 8 } });
  expect(screen.getByRole("listbox", { name: "Enabled apps" })).toBeVisible();
  fireEvent.keyDown(composer, { key: "Tab" });
  expect(composer).toHaveValue("Use $calendar ");

  fireEvent.change(composer, { target: { value: "$", selectionStart: 1 } });
  expect(screen.getByRole("listbox", { name: "Enabled apps" })).toBeVisible();
  fireEvent.pointerDown(screen.getByRole("heading", { name: "What should we build?" }));
  expect(screen.queryByRole("listbox", { name: "Enabled apps" })).not.toBeInTheDocument();
});

test("slash command palette exposes the complete inventory and runs local command dialogs", () => {
  render(<App />);

  fireEvent.click(screen.getByLabelText("Add files and more"));
  fireEvent.click(within(screen.getByRole("menu", { name: "Add" })).getByRole("menuitem", { name: /Slash commands/ }));
  const palette = screen.getByRole("listbox", { name: "Slash commands" });
  expect(within(palette).getAllByRole("option")).toHaveLength(25);

  fireEvent.pointerDown(screen.getByRole("heading", { name: "What should we build?" }));
  expect(screen.queryByRole("listbox", { name: "Slash commands" })).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "/cl" } });
  expect(within(screen.getByRole("listbox", { name: "Slash commands" })).getAllByRole("option").map((option) => option.textContent)).toEqual(expect.arrayContaining([expect.stringContaining("/cloud"), expect.stringContaining("/cloud-environment")]));

  fireEvent.change(screen.getByRole("textbox", { name: "Message" }), { target: { value: "/status" } });
  fireEvent.keyDown(screen.getByRole("textbox", { name: "Message" }), { key: "Enter" });
  expect(screen.getByRole("dialog", { name: "Slash command status" })).toBeVisible();
  expect(screen.getByText("Not started")).toBeVisible();
});

test("personality slash command persists into new conversation execution preferences", async () => {
  render(<App />);
  const input = screen.getByRole("textbox", { name: "Message" });

  fireEvent.change(input, { target: { value: "/personality concise" } });
  fireEvent.keyDown(input, { key: "Enter" });
  expect(await screen.findByText("Response personality changed to concise.")).toBeVisible();

  fireEvent.change(input, { target: { value: "Explain the project" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.createConversation).toHaveBeenCalledWith(expect.objectContaining({
    execution_preferences: expect.objectContaining({ personality: "concise" })
  })));
});

test("new chat uses the shared icon system and stable run-setting choice columns", () => {
  render(<App />);

  expect(screen.getByLabelText("Choose project").querySelector("svg")).toBeInTheDocument();
  expect(screen.getByLabelText("Workspace: Local").querySelector("svg")).toBeInTheDocument();
  expect(screen.getByLabelText("Add files and more").querySelector("svg")).toBeInTheDocument();
  expect(screen.getByLabelText("Change permissions").querySelector("svg")).toBeInTheDocument();

  const { dialog } = openNewChatRunSettings();
  expect(dialog.querySelectorAll(".chat-run-menu-icon svg")).toHaveLength(4);
  fireEvent.click(within(dialog).getByRole("button", { name: /^Effort/ }));

  const choices = within(screen.getByRole("menu", { name: "Effort" })).getAllByRole("menuitemradio");
  expect(choices.length).toBeGreaterThanOrEqual(5);
  for (const choice of choices) {
    expect(choice.querySelector(":scope > .chat-run-choice-copy")).toBeInTheDocument();
    expect(choice.querySelector(":scope > .chat-run-choice-check")).toBeInTheDocument();
  }
  expect(choices.filter((choice) => choice.querySelector(".chat-run-choice-check svg"))).toHaveLength(1);
});

test("transient menus close on outside interaction, Escape, and app focus loss", async () => {
  render(<App />);

  const permissionTrigger = screen.getByLabelText("Change permissions");
  const permissionDetails = permissionTrigger.closest("details") as HTMLDetailsElement;
  fireEvent.click(permissionTrigger);
  expect(permissionDetails.open).toBe(true);

  fireEvent.pointerDown(screen.getByRole("heading", { name: "What should we build?" }));
  expect(permissionDetails.open).toBe(false);

  fireEvent.click(permissionTrigger);
  const runTrigger = document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement;
  fireEvent.pointerDown(runTrigger);
  fireEvent.click(runTrigger);
  const runDetails = runTrigger.closest("details") as HTMLDetailsElement;
  expect(permissionDetails.open).toBe(false);
  expect(runDetails.open).toBe(true);

  fireEvent.keyDown(window, { key: "Escape" });
  expect(runDetails.open).toBe(false);
  await waitFor(() => expect(runTrigger).toHaveFocus());

  const ownerTrigger = screen.getByRole("button", { name: "Open owner menu" });
  fireEvent.click(ownerTrigger);
  expect(screen.getByLabelText("Owner menu")).toHaveClass("open");
  fireEvent.pointerDown(screen.getByRole("textbox", { name: "Message" }));
  expect(screen.getByLabelText("Owner menu")).not.toHaveClass("open");

  const shareTrigger = screen.getByRole("button", { name: "Share" });
  fireEvent.click(shareTrigger);
  expect(screen.getByRole("menu", { name: "Share workspace" })).toBeInTheDocument();
  fireEvent.pointerDown(screen.getByRole("heading", { name: "What should we build?" }));
  expect(screen.queryByRole("menu", { name: "Share workspace" })).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("link", { name: "Pull requests" }));
  const reviewPanel = screen.getByRole("region", { name: "Pull requests" });
  fireEvent.click(within(reviewPanel).getByRole("button", { name: "Filter pull requests" }));
  expect(within(reviewPanel).getByRole("menu", { name: "Pull request filters" })).toBeInTheDocument();
  fireEvent.pointerDown(within(reviewPanel).getByRole("searchbox", { name: "Search pull requests" }));
  expect(within(reviewPanel).queryByRole("menu", { name: "Pull request filters" })).not.toBeInTheDocument();

  const fileMenuTrigger = within(document.querySelector(".desktop-menubar") as HTMLElement).getByRole("button", { name: "File" });
  fireEvent.click(fileMenuTrigger);
  expect(screen.getByLabelText("File menu")).toHaveClass("open");
  fireEvent(window, new Event("blur"));
  expect(screen.getByLabelText("File menu")).not.toHaveClass("open");
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
  const { selector, dialog } = openNewChatRunSettings();
  fireEvent.click(within(dialog).getByRole("button", { name: /^Model/ }));
  const modelMenu = screen.getByRole("menu", { name: "Model" });
  const modelOption = await within(modelMenu).findByRole("menuitemradio", { name: /5\.6 Sol.*OpenAI Subscription/ });
  expect(modelOption).toBeInTheDocument();
  fireEvent.click(modelOption);
  expect(selector).toHaveTextContent("5.6 Sol");
  expect(screen.getByText("1 available model")).toBeInTheDocument();
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

  fireEvent.click(await screen.findByRole("button", { name: "Open owner menu" }));
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
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(menu).getByRole("button", { name: "Log out" }));

  await waitFor(() => expect(client.disconnectProvider).toHaveBeenCalledWith(oauth.id));
  expect(client.disconnectProvider).not.toHaveBeenCalledWith(api.id);
  expect(await screen.findByRole("button", { name: "Open owner menu" })).toBeInTheDocument();
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
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
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
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  const settingsNavigation = screen.getByRole("navigation", { name: "Settings sections" });
  expect(within(settingsNavigation).queryByRole("button", { name: "Hooks" })).not.toBeInTheDocument();

  fireEvent.click(within(settingsNavigation).getByRole("button", { name: "Plugins" }));
  expect(screen.getByRole("tablist", { name: "Plugin categories" })).toBeInTheDocument();
  expect(screen.getByText("No plugins installed")).toBeInTheDocument();

  fireEvent.click(within(settingsNavigation).getByRole("button", { name: "Browser" }));
  expect(screen.getByText("Browser automation")).toBeInTheDocument();
  expect(screen.getByText("Site permissions", { exact: true })).toBeInTheDocument();

  fireEvent.click(within(settingsNavigation).getByRole("button", { name: "Git" }));
  expect(screen.getByText("Branch prefix")).toBeInTheDocument();
  expect(screen.getByText("codex/")).toBeInTheDocument();
  expect(await screen.findByText("Local Git by default")).toBeInTheDocument();
});

test("advanced Hooks reviews exact definitions and trusts through the shared contract", async () => {
  const untrustedDiscovery = {
    skills: [],
    hooks: [{
      cwd: "C:\\workspace",
      warnings: [],
      errors: [],
      hooks: [{
        key: "project:preToolUse:0",
        eventName: "preToolUse",
        handlerType: "command",
        command: "cargo check",
        async: false,
        matcher: "Bash",
        timeoutSec: 30,
        sourcePath: "C:\\workspace\\.codex\\hooks.json",
        source: "project",
        displayOrder: 0,
        enabled: true,
        isManaged: false,
        currentHash: `sha256:${"a".repeat(64)}`,
        trustStatus: "untrusted"
      }]
    }],
    apps: [],
    appsAvailable: true,
    refreshedAt: "2026-08-29T00:00:00Z"
  };
  const trustedDiscovery = {
    ...untrustedDiscovery,
    hooks: [{ ...untrustedDiscovery.hooks[0], hooks: [{ ...untrustedDiscovery.hooks[0].hooks[0], trustStatus: "trusted" }] }]
  };
  client.getCodexStableDiscovery.mockResolvedValue(untrustedDiscovery);
  client.updateCodexHookState.mockResolvedValue(trustedDiscovery);
  vi.spyOn(window, "confirm").mockReturnValue(true);
  render(<App />);
  await enableAdvancedMode();
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(within(screen.getByRole("navigation", { name: "Settings sections" })).getByRole("button", { name: "Hooks" }));

  expect(await screen.findByText("preToolUse")).toBeInTheDocument();
  expect(screen.getByText("cargo check")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Review and trust" }));
  await waitFor(() => expect(client.updateCodexHookState).toHaveBeenCalledWith({
    cwd: "C:\\workspace",
    key: "project:preToolUse:0",
    expectedCurrentHash: `sha256:${"a".repeat(64)}`,
    enabled: undefined,
    trustCurrentDefinition: true
  }));
  expect(await screen.findByRole("button", { name: "Disable" })).toBeInTheDocument();
});

test("advanced configuration exposes searchable Codex feature flags and persists toggles", async () => {
  const features = [
    { name: "memories", stage: "beta", displayName: "Memories", description: "Use durable memories.", enabled: false, defaultEnabled: false },
    { name: "shell_tool", stage: "stable", enabled: true, defaultEnabled: true }
  ];
  client.getCodexFeatures.mockResolvedValue(features);
  client.setCodexFeature.mockResolvedValue([{ ...features[0], enabled: true }, features[1]]);
  render(<App />);

  expect(screen.queryByText("Codex feature flags")).not.toBeInTheDocument();
  await enableAdvancedMode();
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(within(screen.getByRole("navigation", { name: "Settings sections" })).getByRole("button", { name: "Configuration" }));
  fireEvent.click(await screen.findByText("Codex feature flags"));

  fireEvent.change(screen.getByRole("textbox", { name: "Search Codex feature flags" }), { target: { value: "memor" } });
  expect(screen.getByText("Memories")).toBeInTheDocument();
  expect(screen.queryByText("shell_tool")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: "Enable memories" }));
  await waitFor(() => expect(client.setCodexFeature).toHaveBeenCalledWith("memories", true));
  expect(screen.getByRole("checkbox", { name: "Disable memories" })).toBeChecked();
});

test("advanced configuration connects an authenticated remote Codex app-server without storing a token", async () => {
  render(<App />);
  await enableAdvancedMode();
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(within(screen.getByRole("navigation", { name: "Settings sections" })).getByRole("button", { name: "Configuration" }));
  fireEvent.click(await screen.findByText("Codex app-server transport"));

  const transport = screen.getByLabelText("Codex app-server transport configuration");
  fireEvent.change(within(transport).getByLabelText("Codex remote endpoint"), { target: { value: "wss://codex.example.test/socket" } });
  fireEvent.change(within(transport).getByLabelText("Codex remote bearer token environment variable"), { target: { value: "CODEX_REMOTE_TOKEN" } });
  fireEvent.click(within(transport).getByRole("button", { name: "Connect remote" }));

  await waitFor(() => expect(client.updateCodexTransport).toHaveBeenCalledWith(
    2,
    "wss://codex.example.test/socket",
    "CODEX_REMOTE_TOKEN"
  ));
});

test("account settings expose Codex device-code login", async () => {
  client.startCodexLogin.mockResolvedValue({ loginId: "device-login", verificationUrl: "https://auth.openai.com/device", userCode: "ABCD-EFGH" });
  client.getCodexAccount
    .mockResolvedValueOnce({ authenticated: false, requires_openai_auth: true })
    .mockResolvedValue({ authenticated: true, email: "owner@example.com", plan_type: "plus", requires_openai_auth: true });
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(within(screen.getByRole("navigation", { name: "Settings sections" })).getByRole("button", { name: "Account" }));
  fireEvent.click(await screen.findByRole("button", { name: "Use device code" }));
  await waitFor(() => expect(client.startCodexLogin).toHaveBeenCalledWith("device_code", undefined));
  expect(await screen.findByText("ABCD-EFGH")).toBeInTheDocument();
  expect(client.open).toHaveBeenCalledWith("https://auth.openai.com/device");
  await waitFor(() => expect(screen.getAllByText("Using shared Codex account")).not.toHaveLength(0), { timeout: 2_000 });
});

test("advanced account settings submit API credentials once and clear the field", async () => {
  client.startCodexLogin.mockResolvedValue({ authenticated: true, account_type: "api_key", requires_openai_auth: true });
  client.getCodexAccount
    .mockResolvedValueOnce({ authenticated: false, requires_openai_auth: true })
    .mockResolvedValue({ authenticated: true, account_type: "api_key", requires_openai_auth: true });
  render(<App />);
  await enableAdvancedMode();
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(within(screen.getByRole("navigation", { name: "Settings sections" })).getByRole("button", { name: "Account" }));
  fireEvent.click(await screen.findByText("Credential login"));
  const field = screen.getByLabelText("Codex login credential");
  fireEvent.change(field, { target: { value: "sk-fixture" } });
  fireEvent.click(screen.getByRole("button", { name: /^Sign in$/ }));
  await waitFor(() => expect(client.startCodexLogin).toHaveBeenCalledWith("api_key", "sk-fixture"));
  await waitFor(() => expect(screen.queryByLabelText("Codex login credential")).not.toBeInTheDocument());
});

test("advanced configuration runs governed Windows sandbox setup and read grants", async () => {
  const userAgent = vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
  const project = {
    schema_version: 1,
    id: "00000000-0000-4000-8000-000000000640",
    project_id: "00000000-0000-4000-8000-000000000641",
    root: "C:\\Projects\\sandbox-app",
    observed_at: "2026-08-22T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: [],
    build_systems: [],
    test_systems: [],
    instructions: { conflicts: [] }
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.getCodexPermissionProfiles.mockResolvedValue([
    { id: ":workspace", description: "Project workspace", allowed: true },
    { id: "blocked", description: "Managed policy", allowed: false }
  ]);
  client.getCodexConfigurationStatus
    .mockResolvedValueOnce({ requirements: {}, provider_capabilities: {}, windows_sandbox: { status: "notConfigured" } })
    .mockResolvedValue({ requirements: {}, provider_capabilities: {}, windows_sandbox: { status: "ready" } });
  client.pickSandboxReadFolder.mockResolvedValue("C:\\Reference\\docs");
  render(<App />);

  expect(screen.queryByText("Windows sandbox", { exact: true })).not.toBeInTheDocument();
  await enableAdvancedMode();
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(within(screen.getByRole("navigation", { name: "Settings sections" })).getByRole("button", { name: "Configuration" }));
  fireEvent.click(await screen.findByText("Windows sandbox", { exact: true }));

  const sandbox = screen.getByLabelText("Windows sandbox configuration");
  await waitFor(() => expect(within(sandbox).getByLabelText("Windows sandbox project")).toHaveValue(project.project_id));
  fireEvent.click(within(sandbox).getByRole("button", { name: "Set up" }));
  await waitFor(() => expect(client.setupWindowsSandbox).toHaveBeenCalledWith("elevated", project.project_id));
  expect(await within(sandbox).findByText(/Elevated Windows sandbox is configured/)).toBeInTheDocument();

  fireEvent.click(within(sandbox).getByRole("button", { name: "Browse" }));
  await waitFor(() => expect(within(sandbox).getByLabelText("Windows sandbox read directory")).toHaveValue("C:\\Reference\\docs"));
  fireEvent.click(within(sandbox).getByRole("button", { name: "Grant read access" }));
  await waitFor(() => expect(client.grantWindowsSandboxReadDirectory).toHaveBeenCalledWith(project.project_id, "C:\\Reference\\docs", ":workspace"));
  expect(await within(sandbox).findByText(/Codex sandbox read access is ready/)).toBeInTheDocument();
  userAgent.mockRestore();
});

test("Git settings complete browser sign-in and show accessible repositories", async () => {
  client.githubStatus.mockResolvedValue({ configured: true, connected: false });
  client.startGitHubLogin.mockResolvedValue({ session_id: "github-login", authorization_url: "https://github.com/login/oauth/authorize" });
  client.completeGitHubLogin.mockResolvedValue({ configured: true, connected: true, account: { login: "owner", name: "Owner", html_url: "https://github.com/owner" } });
  client.listGitHubRepositories.mockResolvedValue({ repositories: [{ id: 1, full_name: "owner/project", private: true, html_url: "https://github.com/owner/project", clone_url: "https://github.com/owner/project.git", default_branch: "main" }] });
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Open owner menu" }));
  fireEvent.click(within(screen.getByLabelText("Owner menu")).getByRole("button", { name: /Settings/ }));
  fireEvent.click(within(screen.getByRole("navigation", { name: "Settings sections" })).getByRole("button", { name: "Git" }));

  fireEvent.click(await screen.findByRole("button", { name: "Sign in to GitHub" }));
  await waitFor(() => expect(client.completeGitHubLogin).toHaveBeenCalledWith("github-login"));
  expect(client.open).toHaveBeenCalledWith("https://github.com/login/oauth/authorize");
  expect(await screen.findByText("owner/project")).toBeInTheDocument();
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

  await selectNewChatProject("sample");
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Explain this repository" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect(await screen.findByRole("heading", { name: "Explain this repository" })).toBeInTheDocument();
  expect(screen.getByLabelText("Chat conversation")).toBeInTheDocument();
  expect(client.createConversation).toHaveBeenCalledWith({
    title: "Explain this repository",
    project_id: project.project_id,
    execution_preferences: { model_id: undefined, reasoning_effort: "medium", service_tier: undefined, approval_mode: "ask", approval_policy: undefined, sandbox_mode: undefined, permission_profile: undefined, additional_writable_roots: undefined, codex_feature_overrides: {}, codex_config_overrides: [], codex_profile: undefined, strict_config: false, team_preset: "automatic", personality: undefined }
  });
  expect(client.sendConversationMessage).toHaveBeenCalledWith("00000000-0000-4000-8000-000000000c01", {
    content: "Explain this repository",
    execution_preferences: { model_id: undefined, reasoning_effort: "medium", service_tier: undefined, approval_mode: "ask", approval_policy: undefined, sandbox_mode: undefined, permission_profile: undefined, additional_writable_roots: undefined, codex_feature_overrides: {}, codex_config_overrides: [], codex_profile: undefined, strict_config: false, team_preset: "automatic", personality: undefined },
    mentions: []
  });
  expect(client.createTask).not.toHaveBeenCalled();
  expect(client.launchTask).not.toHaveBeenCalled();
  expect(screen.getByText("Fixture response")).toBeInTheDocument();
});

test("new chat resolves an at mention into a structured project reference", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000010",
    project_id: "00000000-0000-4000-8000-000000000011",
    root: "C:\\Projects\\sample",
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: false },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] },
    world: { nodes: [], relationships: [] },
    watched_inputs: {},
    warnings: []
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.searchProjectMentions.mockResolvedValue({ candidates: [{ kind: "file", label: "main.ts", resource: "src/main.ts", detail: "src/main.ts" }] });
  render(<App />);
  await selectNewChatProject("sample");
  const composer = screen.getByLabelText("Message");
  fireEvent.change(composer, { target: { value: "Review @ma", selectionStart: 10 } });
  fireEvent.click(await screen.findByRole("option", { name: /main\.ts/ }));
  expect(composer).toHaveValue("Review @main.ts ");
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({
      content: "Review @main.ts",
      mentions: [{ kind: "file", label: "main.ts", resource: "src/main.ts", byte_start: 7, byte_end: 15, plugin_revision_id: undefined, installation_revision: undefined }]
    })
  ));
});

test("established chat keeps the New Chat composer and routes uncommitted review through the visible conversation", async () => {
  const project = {
    id: "00000000-0000-4000-8000-000000000010",
    project_id: "00000000-0000-4000-8000-000000000011",
    root: "C:\\Projects\\sample",
    observed_at: "2026-08-14T00:00:00Z",
    repository: { is_git: true, branch: "main", dirty: true },
    languages: ["TypeScript"],
    build_systems: ["npm"],
    test_systems: ["vitest"],
    instructions: { conflicts: [] }
  };
  client.listProjects.mockResolvedValue({ projects: [project] });
  client.sendConversationMessage.mockImplementation(async (conversationId: string, request: { content: string; execution_preferences?: Record<string, unknown> }) => ({
    conversation: {
      schema_version: 1,
      id: conversationId,
      title: "Inspect this project",
      project_id: project.project_id,
      state: "active",
      execution_preferences: request.execution_preferences ?? {},
      created_at: "2026-08-22T00:00:00Z",
      updated_at: "2026-08-22T00:00:01Z"
    },
    turns: [{ schema_version: 1, id: crypto.randomUUID(), conversation_id: conversationId, state: "completed", execution_preferences: request.execution_preferences ?? {}, created_at: "2026-08-22T00:00:00Z", updated_at: "2026-08-22T00:00:01Z" }],
    messages: [],
    activities: []
  }));
  render(<App />);

  await selectNewChatProject("sample");
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Inspect this project" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  expect(await screen.findByRole("heading", { name: "Inspect this project" })).toBeInTheDocument();
  expect(screen.getByLabelText("Chat workspace context")).toBeInTheDocument();
  expect(screen.getByLabelText("Project: sample")).toBeInTheDocument();
  expect(screen.getByLabelText("Workspace: Local")).toBeInTheDocument();
  expect(screen.getByLabelText("Branch: main")).toBeInTheDocument();
  expect(screen.getByLabelText("Add files and more")).toBeInTheDocument();
  expect(screen.getByLabelText("Change permissions")).toBeInTheDocument();
  expect(document.querySelector('summary[aria-label="Chat model, effort, speed, and team"]')).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Send" })).toHaveClass("new-chat-send");

  client.sendConversationMessage.mockClear();
  fireEvent.click(screen.getByRole("button", { name: "Review uncommitted changes" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({
      content: expect.stringMatching(/Review code changes in sample: review all staged, unstaged, and untracked changes[\s\S]*Do not modify the working tree/)
    })
  ));
  expect(client.startOwnedCodexReview).not.toHaveBeenCalled();
  expect(await screen.findByText("The uncommitted-changes review was added to this chat.")).toBeInTheDocument();
});

test("delegated coding work stays inline and Chat presents the final preview without expanding an agent", async () => {
  const createdAt = new Date(Date.now() - 5_000).toISOString();
  const acknowledgedAt = new Date(Date.now() - 4_000).toISOString();
  // Batch creation is atomic with delegation and can precede the visible Chat acknowledgement.
  const batchAt = new Date(Date.now() - 4_500).toISOString();
  const completedAt = new Date(Date.now() - 2_000).toISOString();
  client.sendConversationMessage.mockImplementation(async (conversationId: string, request: { content: string; execution_preferences?: Record<string, unknown> }) => ({
    conversation: {
      schema_version: 1,
      id: conversationId,
      title: request.content,
      state: "active",
      execution_preferences: request.execution_preferences ?? {},
      created_at: createdAt,
      updated_at: createdAt
    },
    turns: [{ schema_version: 1, id: "00000000-0000-4000-8000-000000000d05", conversation_id: conversationId, state: "completed", execution_preferences: request.execution_preferences ?? {}, created_at: createdAt, updated_at: completedAt }],
    messages: [
      { schema_version: 1, id: "user-message", conversation_id: conversationId, turn_id: "00000000-0000-4000-8000-000000000d05", role: "user", content: request.content, created_at: createdAt },
      { schema_version: 1, id: "ack-message", conversation_id: conversationId, turn_id: "00000000-0000-4000-8000-000000000d05", role: "assistant", content: "The Coordinator accepted the request.", created_at: acknowledgedAt },
      { schema_version: 1, id: "final-message", conversation_id: conversationId, turn_id: "00000000-0000-4000-8000-000000000d05", role: "assistant", content: "Your preview is ready.", created_at: completedAt }
    ],
    activities: [
      { schema_version: 1, conversation_id: conversationId, turn_id: "00000000-0000-4000-8000-000000000d05", agent_session_id: "00000000-0000-4000-8000-000000000d08", delegation_id: "00000000-0000-4000-8000-000000000d01", codex_item_id: "reasoning-1", kind: "reasoning_summary" as const, state: "completed" as const, command: "Inspecting the requested site structure", started_at: "2026-08-24T18:00:02Z", updated_at: "2026-08-24T18:00:02Z" },
      { schema_version: 1, conversation_id: conversationId, turn_id: "00000000-0000-4000-8000-000000000d05", agent_session_id: "00000000-0000-4000-8000-000000000d08", delegation_id: "00000000-0000-4000-8000-000000000d01", codex_item_id: "command-1", kind: "command" as const, state: "completed" as const, command: "npm test", output: "Tests passed", exit_code: 0, duration_ms: 1200, started_at: "2026-08-24T18:00:03Z", updated_at: "2026-08-24T18:00:04Z" }
    ],
    team: {
      sessions: [
        { schema_version: 4, id: "00000000-0000-4000-8000-000000000d02", conversation_id: conversationId, role_id: "role-coordinator", organization_revision_id: "org", kind: "coordinator", display_name: "Coordinator", state: "retired", prompt_revision: 4, created_at: createdAt, updated_at: completedAt },
        { schema_version: 4, id: "00000000-0000-4000-8000-000000000d08", conversation_id: conversationId, role_id: "role-coder", organization_revision_id: "org", kind: "coder", display_name: "Coding Agent Atlas", parent_session_id: "00000000-0000-4000-8000-000000000d02", state: "retired", prompt_revision: 4, created_at: createdAt, updated_at: completedAt }
      ],
      messages: [],
      blackboard: [],
      worktrees: [],
      delegations: [{
        schema_version: 1,
        id: "00000000-0000-4000-8000-000000000d01",
        conversation_id: conversationId,
        source_session_id: "00000000-0000-4000-8000-000000000d02",
        project_id: "00000000-0000-4000-8000-000000000d03",
        organization_revision_id: "00000000-0000-4000-8000-000000000d04",
        revision: 1,
        prompt_revision: 1,
        coder_session_id: "00000000-0000-4000-8000-000000000d08",
        state: "completed" as const,
        brief: {
          objective: "Build the Springfield plumbing website",
          scope: [], constraints: [], success_conditions: [], context_references: []
        },
        governance: {
          contract: { objective_sha256: "a".repeat(64), allowed_scopes: [], required_checks: ["typecheck"], required_evidence: ["checks performed"], require_integration_ready: true, context_token_budget: 12000, max_attempts: 2 },
          preflight: { passed: true, checks: ["project root exists and is a directory"], blockers: [] },
          context_receipts: [{ source_count: 2, approximate_tokens: 640, token_budget: 12000, truncated: false, source_sha256: "b".repeat(64) }],
          outcome: { latency_ms: 90000, model_turns: 1, approximate_tokens: 640, tool_calls: 4, attempts: 1, owner_interventions: 0, changed_file_count: 2, check_count: 1, successful: true },
          failure_class: "none",
          recovery: "none",
          dependencies: [],
          reserved_scopes: []
        },
        changed_files: [],
        checks: [],
        created_at: createdAt,
        updated_at: createdAt
      }],
      coordination_batches: [{
        schema_version: 6,
        id: "00000000-0000-4000-8000-000000000d06",
        conversation_id: conversationId,
        origin_turn_id: "00000000-0000-4000-8000-000000000d05",
        coordinator_session_id: "00000000-0000-4000-8000-000000000d02",
        delivery_target: "codex_sites",
        delivery_rationale: "New website requests default to Sites.",
        state: "completed",
        revision: 1,
        delegation_ids: ["00000000-0000-4000-8000-000000000d01"],
        coordinator_turn_count: 1,
        coordinator_approximate_tokens: 100,
        coordinator_bypassed: true,
        routing_rationale: "Deterministic preflight selected one bounded Coding Agent.",
        owner_intervention_count: 0,
        created_at: batchAt,
        updated_at: batchAt
      }],
      previews: [{
        schema_version: 6,
        id: "00000000-0000-4000-8000-000000000d07",
        conversation_id: conversationId,
        coordination_batch_id: "00000000-0000-4000-8000-000000000d06",
        delegation_id: "00000000-0000-4000-8000-000000000d01",
        project_id: "00000000-0000-4000-8000-000000000d03",
        delivery_target: "codex_sites",
        strategy: "static_files",
        state: "stopped",
        source_revision: 1,
        source_sha256: "abc",
        source_root: "C:\\Projects\\sample",
        entry_path: "index.html",
        browser_tab_id: "conversation-preview-fixture",
        created_at: batchAt,
        updated_at: batchAt
      }]
    }
  }));
  client.startConversationPreview.mockResolvedValue({
    schema_version: 6,
    id: "00000000-0000-4000-8000-000000000d07",
    conversation_id: "00000000-0000-4000-8000-000000000c01",
    coordination_batch_id: "00000000-0000-4000-8000-000000000d06",
    delegation_id: "00000000-0000-4000-8000-000000000d01",
    project_id: "00000000-0000-4000-8000-000000000d03",
    delivery_target: "codex_sites",
    strategy: "static_files",
    state: "running",
    source_revision: 1,
    source_sha256: "abc",
    source_root: "C:\\Projects\\sample",
    entry_path: "index.html",
    safe_url: "http://127.0.0.1:43123/",
    browser_tab_id: "conversation-preview-fixture",
    created_at: batchAt,
    updated_at: completedAt
  });
  render(<App />);

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Build the site" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  const coordinatorActivity = await screen.findByLabelText("Coordinator activity");
  fireEvent.click(within(coordinatorActivity).getByText("Coordinator"));
  const coordinatorTabs = within(coordinatorActivity).getByRole("tablist", { name: "Coordinator details" });
  expect(within(coordinatorTabs).getByRole("tab", { name: "Chat room" })).toHaveAttribute("aria-selected", "true");
  const activity = await within(coordinatorActivity).findByRole("region", { name: "Coding agent activity" });
  expect(within(activity).getByText("Build the Springfield plumbing website")).toBeInTheDocument();
  expect(within(activity).getByText("Completed")).toBeInTheDocument();
  expect(within(activity).getByRole("progressbar", { name: /Completed/ })).toHaveAttribute("aria-valuenow", "100");
  fireEvent.click(within(activity).getByText("Coding Agent Atlas"));
  expect(within(activity).getByRole("tab", { name: "Chat room" })).toHaveAttribute("aria-selected", "true");
  const latestActivity = activity.querySelector<HTMLElement>(".agent-live-activity")!;
  expect(latestActivity).not.toHaveAttribute("open");
  expect(within(latestActivity).getAllByText("npm test").length).toBeGreaterThanOrEqual(1);
  expect(within(latestActivity).getByText("2 events")).toBeInTheDocument();
  fireEvent.click(latestActivity.querySelector(":scope > summary")!);
  expect(within(latestActivity).getByLabelText("Agent activity history")).toHaveTextContent("Inspecting the requested site structure");
  fireEvent.click(within(activity).getByRole("tab", { name: "Activity" }));
  expect(within(activity).getByText("project root exists and is a directory", { exact: false })).toBeInTheDocument();
  expect(within(activity).getByText(/Context receipt: ~640/)).toBeInTheDocument();
  fireEvent.click(within(coordinatorTabs).getByRole("tab", { name: "Activity" }));
  expect(within(coordinatorActivity).getByText("deterministic single-Coder")).toBeInTheDocument();
  const acknowledgement = screen.getByText("The Coordinator accepted the request.").closest("article")!;
  const batch = screen.getByRole("region", { name: "Delegated agent progress" });
  const completion = screen.getByText("Your preview is ready.").closest("article")!;
  expect(acknowledgement.compareDocumentPosition(batch) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(batch.compareDocumentPosition(completion) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(within(activity).queryByRole("button", { name: "Open preview" })).not.toBeInTheDocument();
  const resultActions = screen.getByRole("region", { name: "Chat Agent result actions" });
  expect(within(resultActions).getByText(/Temporary local preview/)).toBeInTheDocument();
  expect(within(resultActions).queryByText(/ChatGPT Sites ·/)).not.toBeInTheDocument();
  expect(resultActions.compareDocumentPosition(completion) & Node.DOCUMENT_POSITION_PRECEDING).toBeTruthy();
  fireEvent.click(within(resultActions).getByRole("button", { name: "Open preview" }));
  await waitFor(() => expect(client.startConversationPreview).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    "00000000-0000-4000-8000-000000000d07"
  ));
  expect(await screen.findByRole("tab", { name: /Local preview/ })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByLabelText("MetaCog browser content")).toBeInTheDocument();
});

test("Coordinator lifecycle and tool traces are inline without a separate Agent Room", async () => {
  const now = "2026-08-24T18:00:00Z";
  const conversationId = "00000000-0000-4000-8000-000000000e01";
  const chatId = "00000000-0000-4000-8000-000000000e02";
  const coordinatorId = "00000000-0000-4000-8000-000000000e03";
  const coderId = "00000000-0000-4000-8000-000000000e04";
  const agentRoomView = {
    conversation: { schema_version: 1, id: conversationId, title: "Build in parallel", state: "active", execution_preferences: {}, created_at: now, updated_at: now },
    turns: [{ schema_version: 1, id: "room-turn", conversation_id: conversationId, state: "completed", execution_preferences: {}, created_at: now, updated_at: now }],
    messages: [{ schema_version: 1, id: "room-user", conversation_id: conversationId, turn_id: "room-turn", role: "user", content: "Build in parallel", created_at: now }], activities: [],
    team: {
      sessions: [
        { schema_version: 4, id: chatId, conversation_id: conversationId, role_id: "role-chat", organization_revision_id: "org", kind: "chat", display_name: "Chat Agent", state: "ready", prompt_revision: 4, created_at: now, updated_at: now },
        { schema_version: 4, id: coordinatorId, conversation_id: conversationId, role_id: "role-coordinator", organization_revision_id: "org", kind: "coordinator", display_name: "Coordinator", parent_session_id: chatId, state: "working", prompt_revision: 4, created_at: now, updated_at: now },
        { schema_version: 4, id: coderId, conversation_id: conversationId, role_id: "role-coder", organization_revision_id: "org", kind: "coder", display_name: "Coding Agent Atlas", parent_session_id: coordinatorId, state: "working", prompt_revision: 4, created_at: now, updated_at: now }
      ],
      delegations: [], blackboard: [], worktrees: [],
      messages: [
        { schema_version: 4, id: "join", conversation_id: conversationId, from_session_id: coordinatorId, to_session_id: coordinatorId, kind: "joined", room: "backstage", content: "Coordinator started coordinating this request", created_at: now },
        { schema_version: 4, id: "call", conversation_id: conversationId, from_session_id: coordinatorId, to_session_id: coordinatorId, kind: "tool_call", room: "backstage", tool_name: "delegate_code", correlation_id: "trace-1", content: "Calling delegate_code", created_at: now },
        { schema_version: 4, id: "result", conversation_id: conversationId, from_session_id: coordinatorId, to_session_id: coordinatorId, kind: "tool_result", room: "backstage", tool_name: "delegate_code", correlation_id: "trace-1", content: "delegate_code completed", created_at: now },
        { schema_version: 4, id: "brief", conversation_id: conversationId, from_session_id: coordinatorId, to_session_id: coderId, kind: "delegation", room: "backstage", content: JSON.stringify({ objective: "Build the responsive shell", scope: ["src"], constraints: ["Keep it accessible"], success_conditions: ["Responsive shell renders"], context_references: [] }), created_at: now }
      ],
      context_checkpoints: [{ schema_version: 4, id: "checkpoint", conversation_id: conversationId, session_id: coordinatorId, room: "backstage", revision: 1, summary: "Delegated the responsive shell.", source_references: ["brief"], approximate_tokens: 8, created_at: now }]
      ,coordination_batches: [{ schema_version: 6, id: "room-batch", conversation_id: conversationId, origin_turn_id: "room-turn", coordinator_session_id: coordinatorId, state: "running", revision: 1, delegation_ids: [], coordinator_turn_count: 1, coordinator_approximate_tokens: 10, created_at: now, updated_at: now }]
    }
  };
  client.sendConversationMessage.mockResolvedValue(agentRoomView);
  client.getConversation.mockResolvedValue(agentRoomView);
  render(<App />);
  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Build in parallel" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  expect(screen.queryByRole("button", { name: /^Agent Room/ })).not.toBeInTheDocument();
  expect(screen.queryByRole("navigation", { name: "Conversation rooms" })).not.toBeInTheDocument();
  const coordinator = await screen.findByLabelText("Coordinator activity");
  expect(within(coordinator).getAllByText("Planning and assigning work").length).toBeGreaterThan(0);
  fireEvent.click(coordinator.querySelector(":scope > summary")!);
  expect(within(coordinator).getByLabelText("Coordinator chat room")).toHaveAttribute("tabindex", "0");
  expect(within(coordinator).getByText("Coordinator started coordinating this request")).toBeInTheDocument();
  expect(within(coordinator).getAllByText(/delegate_code/).length).toBeGreaterThanOrEqual(2);
  expect(within(coordinator).getByText("Build the responsive shell")).toBeInTheDocument();
  expect(within(coordinator).getByText("Keep it accessible")).toBeInTheDocument();
  expect(within(coordinator).queryByText(/"objective":/)).not.toBeInTheDocument();
  expect(screen.getByLabelText("Message")).toBeInTheDocument();
});

test("pending delivery includes batch and wake callbacks after coding is terminal", () => {
  type Team = Parameters<typeof conversationTeamHasPendingDelivery>[0];
  const base = { sessions: [], delegations: [], messages: [], blackboard: [], worktrees: [] };
  expect(conversationTeamHasPendingDelivery({ ...base, coordination_batches: [{ state: "chat_pending" }] } as unknown as Team)).toBe(true);
  expect(conversationTeamHasPendingDelivery({ ...base, wakes: [{ state: "pending" }] } as unknown as Team)).toBe(true);
  expect(conversationTeamHasPendingDelivery({
    ...base,
    coordination_batches: [{ state: "completed" }],
    wakes: [{ state: "completed" }]
  } as unknown as Team)).toBe(false);
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
    "Use the blue design",
    []
  ));
  expect(screen.queryByLabelText("Queued messages")).not.toBeInTheDocument();

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Keep the current structure" } });
  fireEvent.keyDown(screen.getByLabelText("Message"), { key: "Enter", code: "Enter", ctrlKey: true });
  await waitFor(() => expect(client.steerConversationTurn).toHaveBeenLastCalledWith(
    conversation.id,
    turn.id,
    "Keep the current structure",
    []
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

  await selectNewChatProject("sample");
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

test("advanced project chats discover and persist allowed Codex permission profiles", async () => {
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
  client.getCodexPermissionProfiles.mockResolvedValue([
    { id: "workspace-safe", description: "Write only inside the project.", allowed: true },
    { id: "organization-blocked", description: "Blocked by managed policy.", allowed: false }
  ]);
  render(<App />);

  await enableAdvancedMode();
  await selectNewChatProject("sample");
  await waitFor(() => expect(client.getCodexPermissionProfiles).toHaveBeenCalledWith(project.project_id));
  fireEvent.click(screen.getByLabelText("Change permissions"));
  expect(await screen.findByRole("menuitemradio", { name: /workspace-safe/i })).toBeInTheDocument();
  expect(screen.queryByRole("menuitemradio", { name: /organization-blocked/i })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("menuitemradio", { name: /workspace-safe/i }));
  expect(screen.getByLabelText("Change permissions")).toHaveTextContent("workspace-safe");

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Use the managed profile" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({
      execution_preferences: expect.objectContaining({
        approval_mode: "ask",
        permission_profile: "workspace-safe"
      })
    })
  ));
});

test("selecting a standard permission mode clears a named Codex profile", async () => {
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
  client.getCodexPermissionProfiles.mockResolvedValue([
    { id: "workspace-safe", description: "Write only inside the project.", allowed: true }
  ]);
  render(<App />);

  await enableAdvancedMode();
  await selectNewChatProject("sample");
  fireEvent.click(screen.getByLabelText("Change permissions"));
  fireEvent.click(await screen.findByRole("menuitemradio", { name: /workspace-safe/i }));
  fireEvent.click(screen.getByLabelText("Change permissions"));
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Read only/ }));
  expect(screen.getByLabelText("Change permissions")).toHaveTextContent("Read only");

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Inspect only" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({
      execution_preferences: expect.objectContaining({
        approval_mode: "read_only",
        permission_profile: undefined
      })
    })
  ));
});

test("read-only permission is available for project chats and persists with the turn", async () => {
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

  await selectNewChatProject("sample");
  fireEvent.click(screen.getByLabelText("Change permissions"));
  fireEvent.click(screen.getByRole("menuitemradio", { name: /Read only/ }));
  expect(screen.getByLabelText("Change permissions")).toHaveTextContent("Read only");

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Inspect without editing" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ approval_mode: "read_only" }) })
  ));
});

test("team presets are selected before prompting and persisted with the conversation turn", async () => {
  render(<App />);

  const { dialog } = openNewChatRunSettings();
  fireEvent.click(within(dialog).getByRole("button", { name: /^Team/ }));
  fireEvent.click(within(screen.getByRole("menu", { name: "Team preset" })).getByRole("menuitemradio", { name: /Balanced/ }));
  fireEvent.click(document.querySelector('summary[aria-label="New chat model, effort, speed, and team"]') as HTMLElement);
  expect(within(screen.getByRole("dialog", { name: "New chat model, effort, speed, and team" })).getByRole("button", { name: /^Team/ })).toHaveTextContent("Balanced");

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Plan this with a balanced team" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  await waitFor(() => expect(client.sendConversationMessage).toHaveBeenCalledWith(
    "00000000-0000-4000-8000-000000000c01",
    expect.objectContaining({ execution_preferences: expect.objectContaining({ team_preset: "balanced" }) })
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

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Explain streaming" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  expect(await screen.findByText("Here is a partial", {}, { timeout: 5000 })).toBeInTheDocument();
  expect(screen.getByText(/^Responding ·/)).toBeInTheDocument();
  const chatViewport = container.querySelector<HTMLElement>(".chat-message-list");
  expect(chatViewport).not.toBeNull();
  let scrollTop = 450;
  Object.defineProperties(chatViewport!, {
    scrollHeight: { configurable: true, get: () => 1_000 },
    clientHeight: { configurable: true, get: () => 500 },
    scrollTop: {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => { scrollTop = Math.min(value, 500); }
    }
  });
  fireEvent.scroll(chatViewport!);
  client.getConversation.mockResolvedValue({
    ...streamedView,
    messages: [{ ...streamedView.messages[0] }, { ...streamedView.messages[1], content: "A near-bottom partial response" }]
  });
  expect(await screen.findByText("A near-bottom partial response", {}, { timeout: 5000 })).toBeInTheDocument();
  await waitFor(() => expect(scrollTop).toBe(500));

  scrollTop = 250;
  fireEvent.scroll(chatViewport!);
  client.getConversation.mockResolvedValue({
    ...streamedView,
    messages: [{ ...streamedView.messages[0] }, { ...streamedView.messages[1], content: "A second partial response" }]
  });
  expect(await screen.findByText("A second partial response", {}, { timeout: 5000 })).toBeInTheDocument();
  expect(scrollTop).toBe(250);

  scrollTop = 450;
  fireEvent.scroll(chatViewport!);
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
        output: "# MetaCog",
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
  expect(screen.getByText("# MetaCog")).toBeInTheDocument();
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
    origin: {
      conversation: { source_type: "mcg.conversation.v2", locator: "00000000-0000-4000-8000-000000000d99", content_hash: { algorithm: "blake3-256", domain: "MetaCog.ApprovalConversation", digest_hex: "a".repeat(64) } },
      turn: null,
      invocation: null,
      command: null,
      agent: null,
      evidence: []
    },
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
  client.getApproval.mockResolvedValue(approval);
  client.approveRequest.mockResolvedValue({ ...approval, state: "approved" });
  client.sendConversationMessage.mockReturnValue(new Promise((resolve) => { finishSend = resolve; }));
  render(<App />);

  fireEvent.change(screen.getByLabelText("Message"), { target: { value: "Build the site" } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  expect(await screen.findByText("Permission needed to continue", {}, { timeout: 1500 })).toBeInTheDocument();
  expect(screen.getByText(approval.reason)).toBeInTheDocument();
  expect(screen.getByText(approval.resource)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "View exact source" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("is not available in the current compatibility projection");
  expect(client.getConversation).not.toHaveBeenCalledWith(approval.origin.conversation.locator);
  fireEvent.click(screen.getByRole("button", { name: "Allow once" }));

  await waitFor(() => expect(client.approveRequest).toHaveBeenCalledWith(approval.id));
  expect(client.resumeTask).not.toHaveBeenCalled();
  finishSend?.(waitingView);
});

test("legacy blocked tasks do not crowd the normal new-chat workspace", async () => {
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

  expect(await screen.findByRole("heading", { name: "What should we build?" })).toBeVisible();
  expect(screen.queryByRole("button", { name: blockedTask.goal })).not.toBeInTheDocument();
  expect(client.retryTask).not.toHaveBeenCalled();
});

test("legacy task approvals stay out of the normal new-chat workspace", async () => {
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

  expect(await screen.findByRole("heading", { name: "What should we build?" })).toBeVisible();
  expect(screen.queryByRole("button", { name: waitingTask.goal })).not.toBeInTheDocument();
  expect(client.approveRequest).not.toHaveBeenCalled();
  expect(client.resumeTask).not.toHaveBeenCalled();
});

test("legacy denied-task controls stay out of the normal new-chat workspace", async () => {
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

  expect(await screen.findByRole("heading", { name: "What should we build?" })).toBeVisible();
  expect(screen.queryByRole("button", { name: waitingTask.goal })).not.toBeInTheDocument();
  expect(client.denyRequest).not.toHaveBeenCalled();
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

  fireEvent.click(screen.getByLabelText("Add files and more"));
  const addMenu = screen.getByRole("menu", { name: "Add" });
  fireEvent.click(within(addMenu).getByRole("menuitem", { name: /Open project folder/ }));
  expect((await screen.findAllByText("sample")).length).toBeGreaterThan(0);
  expectNewChatProject("sample");
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
  await waitFor(() => {
    expect(JSON.parse(window.localStorage.getItem("mcg.global-folders") ?? "[]")).toEqual([root]);
  });

  fireEvent.click(within(screen.getByLabelText("Global folders")).getByRole("button", { name: "Remove" }));
  expect(within(screen.getByLabelText("Global folders")).getByText("No global folders added.")).toBeInTheDocument();
});

test("desktop menus follow the Codex command structure with MetaCog branding", async () => {
  render(<App />);

  const applicationMenu = screen.getByLabelText("Application menu");
  const titlebarBrand = within(applicationMenu).getByLabelText("MetaCog");
  expect(titlebarBrand).toHaveTextContent("MetaCog");
  expect(titlebarBrand.querySelector("img")).toHaveAttribute("src", expect.stringContaining("mcg-logo"));
  const brand = within(screen.getByLabelText("Primary navigation")).getByLabelText("MetaCog");
  expect(brand.querySelector("img")).toHaveAttribute("src", expect.stringContaining("mcg-logo"));
  expect(within(applicationMenu).getByRole("button", { name: "Hide sidebar" }).querySelector("svg")).toBeInTheDocument();
  expect(within(applicationMenu).getByRole("button", { name: "Minimize" }).querySelector("svg")).toBeInTheDocument();
  expect(within(applicationMenu).getByRole("button", { name: "Maximize or restore" }).querySelector("svg")).toBeInTheDocument();
  expect(within(applicationMenu).getByRole("button", { name: "Close window" }).querySelector("svg")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "File" }));
  let menu = within(document.querySelector(".desktop-menu-popover.open") as HTMLElement);
  expect(menu.getByRole("button", { name: "New Window" })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /New Chat.*Ctrl\+N/ })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /New Temporary Chat/ })).toBeDisabled();
  expect(menu.getByRole("button", { name: /Open Folder.*Ctrl\+O/ })).toBeInTheDocument();
  expect(menu.getByRole("button", { name: /Quit MetaCog.*Ctrl\+Q/ })).toBeInTheDocument();
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
  expect(menu.getByRole("button", { name: "About MetaCog" })).toBeInTheDocument();
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

  fireEvent.click(screen.getByLabelText("Choose project"));
  await within(screen.getByRole("menu", { name: "Project choices" })).findByRole("menuitemradio", { name: /sample/ });
  fireEvent.click(screen.getByLabelText("Choose project"));
  fireEvent.click(screen.getByLabelText("Add files and more"));
  fireEvent.click(within(screen.getByRole("menu", { name: "Add" })).getByRole("menuitem", { name: /Open project folder/ }));

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
  expectNewChatProject("remembered");
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
}, 30_000);

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
    fresh_until: "2036-08-27T12:01:00Z",
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

  fireEvent.click(screen.getByLabelText("Add files and more"));
  fireEvent.click(within(screen.getByRole("menu", { name: "Add" })).getByRole("menuitem", { name: /Open project folder/ }));

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
