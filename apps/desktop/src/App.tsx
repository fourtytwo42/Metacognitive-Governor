import { useEffect, useState } from "react";

import {
  cancelOpenAiOAuth,
  completeOpenAiOAuth,
  configureMcpConnection,
  discoverMcpConnection,
  listMcpConnections,
  type McpAuthMode,
  type McpListItem,
  type McpProtocolMode,
  type McpTransportKind,
  openExternalUrl,
  type OAuthLoginStart,
  type OpenAiOAuthMethod,
  startOpenAiOAuth
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

  useEffect(() => {
    let active = true;
    listMcpConnections()
      .then((response) => {
        if (active) setMcpConnections(response.connections);
      })
      .catch((error) => {
        if (active) setMcpError(errorMessage(error));
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

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand" aria-label="MetaCognitive Governor">
          <span className="brand-mark">M</span>
          <span>MCG</span>
        </div>
        <nav>
          <a className="nav-item active" href="#projects">Projects</a>
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

      <main className="workspace" id="projects">
        <header className="topbar">
          <div>
            <p className="eyebrow">METACOGNITIVE GOVERNOR</p>
            <h1>Your AI workspace</h1>
          </div>
          <span className="status-pill"><i /> Local runtime</span>
        </header>

        <section className="hero-card">
          <div>
            <p className="eyebrow">START A PROJECT</p>
            <h2>What would you like to build?</h2>
            <p className="muted">
              Open a folder or describe a goal. MCG will choose the smallest useful
              organization, work within your permissions, and verify the result.
            </p>
          </div>
          <div className="composer">
            <label htmlFor="goal">Project goal</label>
            <textarea id="goal" placeholder="Build and test a responsive website…" />
            <div className="composer-actions">
              <button className="secondary" type="button">Open folder</button>
              <button className="primary" type="button">Start</button>
            </div>
          </div>
        </section>

        <section className="grid">
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
