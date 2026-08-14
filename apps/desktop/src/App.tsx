import { useEffect, useState } from "react";

import {
  approveRequest,
  cancelOpenAiOAuth,
  completeOpenAiOAuth,
  configureMcpConnection,
  configureProvider,
  createTask,
  denyRequest,
  discoverProject,
  discoverMcpConnection,
  discoverProvider,
  listMcpConnections,
  listProviders,
  listApprovals,
  listProjects,
  listTasks,
  launchTask,
  loadTaskAudit,
  loadTaskEvents,
  type McpAuthMode,
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
  pickProjectDirectory,
  type ProjectModel,
  type RiskLevel,
  retryTask,
  resumeTask,
  startOpenAiOAuth,
  type TaskEnvelope,
  type ApprovalRequest,
  type TaskEventsResponse,
  type ToolAuditResponse,
  transitionTask
} from "./desktop-client";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
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
  advanced
}: {
  history: TaskEventsResponse;
  audit: ToolAuditResponse;
  advanced: boolean;
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

  return (
    <section className="task-evidence" aria-label="Task evidence">
      <div className="evidence-overview">
        <div><span>Outcome</span><strong>{evaluation?.payload?.outcome ?? "In progress"}</strong></div>
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
  const [oauthMethod, setOauthMethod] = useState<OpenAiOAuthMethod>("browser");
  const [login, setLogin] = useState<OAuthLoginStart | null>(null);
  const [providerStatus, setProviderStatus] = useState("Not connected");
  const [busy, setBusy] = useState(false);
  const [providerError, setProviderError] = useState<string | null>(null);
  const [providers, setProviders] = useState<ProviderListItem[]>([]);
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
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([]);

  useEffect(() => {
    let active = true;
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
        if (active) setTaskError(errorMessage(error));
      });
    listApprovals()
      .then((response) => {
        if (active) setApprovals(response.approvals);
      })
      .catch((error) => {
        if (active) setTaskError(errorMessage(error));
      });
    return () => {
      active = false;
    };
  }, []);

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

  function taskEvidence(taskId: string) {
    const history = taskHistory[taskId];
    const audit = taskAudit[taskId];
    return history && audit
      ? <TaskEvidence history={history} audit={audit} advanced={advanced} />
      : null;
  }

  async function chooseProject() {
    setTaskBusy(true);
    setTaskError(null);
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
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
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
          <a className="nav-item" href="#providers">Providers</a>
          <a className="nav-item" href="#mcp">Tools</a>
        </nav>
        <button
          className="mode-button"
          type="button"
          aria-pressed={advanced}
          onClick={() => setAdvanced((value) => !value)}
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
                <textarea value={taskSuccess} onChange={(event) => setTaskSuccess(event.target.value)} />
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
              <button className="secondary" type="button" disabled={taskBusy} onClick={chooseProject}>Open folder</button>
              <button className="primary" type="button" disabled={taskBusy} onClick={addTask}>
                {taskBusy ? "Creating..." : "Create task"}
              </button>
            </div>
            {taskError && <p className="error-message" role="alert">{taskError}</p>}
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
                  {taskEvidence(task.id)}
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
              <button className="secondary" type="button" disabled={taskBusy} onClick={chooseProject}>Add project folder</button>
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
          </article>

          <article className="panel runtime-panel">
            {advanced ? (
              <>
                <p className="eyebrow">ADVANCED</p>
                <h2>Runtime status</h2>
                <dl>
                  <div><dt>Daemon API</dt><dd>127.0.0.1:7777</dd></div>
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
      </main>
    </div>
  );
}
