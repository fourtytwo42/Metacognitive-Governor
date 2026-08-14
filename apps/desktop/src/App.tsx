import { useState } from "react";

const progress = [
  "Project instructions discovered",
  "Provider fabric ready for connections",
  "Policy boundary initialized"
];

export function App() {
  const [advanced, setAdvanced] = useState(false);

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
          <a className="nav-item" href="#settings">Settings</a>
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
          <span className="status-pill"><i /> Runtime scaffold</span>
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
                  <div><dt>Provider</dt><dd>Not connected</dd></div>
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
      </main>
    </div>
  );
}

