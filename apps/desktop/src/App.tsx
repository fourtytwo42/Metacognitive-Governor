import { useEffect, useState } from "react";

import {
  approveRequest,
  cancelOpenAiOAuth,
  completeOpenAiOAuth,
  configureMcpConnection,
  createTask,
  discoverProject,
  discoverMcpConnection,
  listMcpConnections,
  listApprovals,
  listProjects,
  listTasks,
  launchTask,
  loadTaskEvents,
  type McpAuthMode,
  type McpListItem,
  type McpProtocolMode,
  type McpTransportKind,
  openExternalUrl,
  type OAuthLoginStart,
  type OpenAiOAuthMethod,
  pickProjectDirectory,
  type ProjectModel,
  type RiskLevel,
  retryTask,
  resumeTask,
  startOpenAiOAuth,
  type TaskEnvelope,
  type ApprovalRequest,
  type TaskEventsResponse,
  transitionTask
} from "./desktop-client";

const progress = [
  "Project instructions discovered",
  "Provider fabric ready for connections",
  "Policy boundary initialized"
];

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function App() {
  const [advanced, setAdvanced] = useState(false);
  const [oauthMethod, setOauthMethod] = useState<OpenAiOAuthMethod>("browser");
  const [login, setLogin] = useState<OAuthLoginStart | null>(null);
  const [providerStatus, setProviderStatus] = useState("Not connected");
  const [busy, setBusy] = useState(false);
  const [providerError, setProviderError] = useState<string | null>(null);
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
      return;
    }
    setTaskBusy(true);
    setTaskError(null);
    try {
      const events = await loadTaskEvents(taskId);
      setTaskHistory((history) => ({ ...history, [taskId]: events }));
    } catch (error) {
      setTaskError(errorMessage(error));
    } finally {
      setTaskBusy(false);
    }
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
                  {taskHistory[task.id] && <ol className="event-history">
                    {taskHistory[task.id]?.events.map((event) => (
                      <li key={event.sequence}><span>{event.sequence}</span>{event.kind.replaceAll("_", " ")}</li>
                    ))}
                  </ol>}
                </div>
                <div className="task-actions">
                  {task.state === "pending" && <button className="primary" type="button" disabled={taskBusy || !task.project_id} title={task.project_id ? undefined : "Select a project before starting"} onClick={() => startTask(task.id)}>Start task</button>}
                  {task.state === "blocked" && <button className="primary" type="button" disabled={taskBusy} onClick={() => retryBlockedTask(task.id)}>Retry</button>}
                  {task.state === "waiting_for_approval" && approvals.find((approval) => approval.task_id === task.id) && <button className="primary" type="button" disabled={taskBusy} onClick={() => approveAndResume(task.id, approvals.find((approval) => approval.task_id === task.id)!.id)}>Approve and continue</button>}
                  {!(["completed", "failed", "cancelled"] as string[]).includes(task.state) && <button className="secondary" type="button" disabled={taskBusy} onClick={() => changeTaskState(task.id, "cancelled")}>Cancel</button>}
                  <button className="secondary" type="button" disabled={taskBusy} onClick={() => toggleTaskHistory(task.id)}>{taskHistory[task.id] ? "Hide history" : "History"}</button>
                </div>
              </article>
            ))}
          </div>
        </section>

        <section className="grid" id="projects">
          <article className="panel">
            <p className="eyebrow">FOUNDATION STATUS</p>
            <h2>Ready for implementation</h2>
            <ul className="progress-list">
              {progress.map((item) => <li key={item}><span>✓</span>{item}</li>)}
            </ul>
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
