import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const client = vi.hoisted(() => ({
  start: vi.fn(),
  complete: vi.fn(),
  cancel: vi.fn(),
  open: vi.fn(),
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
  pickProject: vi.fn(),
  discoverProject: vi.fn(),
  listProjects: vi.fn()
}));

vi.mock("./desktop-client", () => ({
  startOpenAiOAuth: client.start,
  completeOpenAiOAuth: client.complete,
  cancelOpenAiOAuth: client.cancel,
  openExternalUrl: client.open,
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
  pickProjectDirectory: client.pickProject,
  discoverProject: client.discoverProject,
  listProjects: client.listProjects
}));

import { App } from "./App";

beforeEach(() => {
  client.start.mockReset();
  client.complete.mockReset();
  client.cancel.mockReset();
  client.open.mockReset();
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
  client.pickProject.mockReset();
  client.discoverProject.mockReset();
  client.listProjects.mockReset();
  client.open.mockResolvedValue(undefined);
  client.listMcp.mockResolvedValue({ connections: [] });
  client.listTasks.mockResolvedValue({ tasks: [] });
  client.listProjects.mockResolvedValue({ projects: [] });
  client.listApprovals.mockResolvedValue({ approvals: [] });
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

  fireEvent.click(screen.getByRole("button", { name: "History" }));
  expect(await screen.findByText("task transitioned")).toBeInTheDocument();
  expect(screen.getByText("Objective result: passed")).toBeInTheDocument();
  expect(screen.getByText("2 of 2 checks passed")).toBeInTheDocument();
  expect(client.taskEvents).toHaveBeenCalledWith(pendingTask.id);
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
