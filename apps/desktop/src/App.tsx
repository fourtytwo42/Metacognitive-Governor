import { useState } from "react";

import {
  cancelOpenAiOAuth,
  completeOpenAiOAuth,
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

  async function startLogin() {
    setBusy(true);
    setProviderError(null);
    try {
      const started = await startOpenAiOAuth(oauthMethod);
      setLogin(started);
      setProviderStatus("Waiting for authorization");
      window.open(started.verification_url, "_blank", "noopener,noreferrer");
    } catch (error) {
      setProviderError(errorMessage(error));
      setProviderStatus("Connection failed");
    } finally {
      setBusy(false);
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
                <a href={login.verification_url} target="_blank" rel="noreferrer">
                  Open verification page
                </a>
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
      </main>
    </div>
  );
}
