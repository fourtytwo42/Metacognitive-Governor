import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const client = vi.hoisted(() => ({
  start: vi.fn(),
  complete: vi.fn(),
  cancel: vi.fn(),
  open: vi.fn(),
  listProviders: vi.fn(),
  configureProvider: vi.fn(),
  discoverProvider: vi.fn(),
  listMcp: vi.fn(),
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
  pickProject: vi.fn(),
  discoverProject: vi.fn(),
  listProjects: vi.fn(),
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
  reclaimBrowserSession: vi.fn()
}));

vi.mock("./desktop-client", () => ({
  startOpenAiOAuth: client.start,
  completeOpenAiOAuth: client.complete,
  cancelOpenAiOAuth: client.cancel,
  openExternalUrl: client.open,
  listProviders: client.listProviders,
  configureProvider: client.configureProvider,
  discoverProvider: client.discoverProvider,
  listMcpConnections: client.listMcp,
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
  pickProjectDirectory: client.pickProject,
  discoverProject: client.discoverProject,
  listProjects: client.listProjects,
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
  reclaimBrowserSession: client.reclaimBrowserSession
}));

import { App } from "./App";

beforeEach(() => {
  client.start.mockReset();
  client.complete.mockReset();
  client.cancel.mockReset();
  client.open.mockReset();
  client.listProviders.mockReset();
  client.configureProvider.mockReset();
  client.discoverProvider.mockReset();
  client.listMcp.mockReset();
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
  client.pickProject.mockReset();
  client.discoverProject.mockReset();
  client.listProjects.mockReset();
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
  client.open.mockResolvedValue(undefined);
  client.listProviders.mockResolvedValue({ providers: [] });
  client.listMcp.mockResolvedValue({ connections: [] });
  client.listTasks.mockResolvedValue({ tasks: [] });
  client.listProjects.mockResolvedValue({ projects: [] });
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
  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
  fireEvent.change(within(scheduler).getByLabelText("Trigger"), { target: { value: "cron" } });
  expect(within(scheduler).getByLabelText("Five-field cron")).toHaveValue("0 2 * * *");
  fireEvent.click(screen.getByRole("button", { name: "Simple mode" }));
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
    max_model_calls: 8
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

test("advanced mode reveals runtime details", () => {
  render(<App />);

  expect(screen.queryByRole("heading", { name: "Runtime status" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
  expect(screen.getByRole("heading", { name: "Runtime status" })).toBeInTheDocument();
});

test("OpenAI subscription login exposes verification then confirms connection", async () => {
  client.start.mockResolvedValue({
    session_id: "fixture-session",
    connection_id: "openai-subscription",
    auth_mode: "chat_gpt_device_code",
    verification_url: "https://auth.openai.com/codex/device",
    user_code: "ABCD-1234"
  });
  client.complete.mockResolvedValue({
    connection: {
      id: "openai-subscription",
      display_name: "OpenAI Subscription",
      auth_mode: "chat_gpt_device_code"
    },
    catalog: { models: [] }
  });
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "Connect with OpenAI" }));
  expect(await screen.findByText("Code:", { exact: false })).toHaveTextContent("ABCD-1234");
  expect(screen.getByRole("button", { name: "Open verification page" })).toBeInTheDocument();
  expect(client.start).toHaveBeenCalledWith("browser");
  expect(client.open).toHaveBeenCalledWith("https://auth.openai.com/codex/device");

  fireEvent.click(screen.getByRole("button", { name: "I’ve authorized MCG" }));
  expect(await screen.findByText("OpenAI Subscription connected")).toBeInTheDocument();
  expect(client.complete).toHaveBeenCalledWith("fixture-session");
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

  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
  expect(screen.getByText("Local Model")).toBeInTheDocument();
  expect(screen.getByText("Cost unknown")).toBeInTheDocument();
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

  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
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

  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
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
