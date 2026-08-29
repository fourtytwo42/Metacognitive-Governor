# MetaCog — MetaCognitive Governor

MetaCog is a consumer-friendly, cross-platform AI runtime that works directly on
projects and computers while governing how models, agents, tools, memory,
permissions, and budgets are used.

MetaCog is designed as one persistent Rust runtime with every primary interface:

- Native Tauri desktop application
- Interactive and machine-readable CLI
- Headless daemon
- OpenAI-compatible provider API
- MCP client and server

The system is intentionally model-agnostic. Initial provider targets are OpenAI
API credentials, an isolated ChatGPT/Codex OAuth integration, OpenRouter, local
OpenAI-compatible servers, and arbitrary OpenAI-compatible endpoints.

## Status

MetaCog is in active implementation. M0 through M8 have substantial qualified
checkpoints, while M9 and M10 remain partial and the signed/updatable M11 consumer
release is not complete. M12 remains a long-term implementation milestone. The
full vision remains in scope and is delivered through tested, end-to-end
milestones; see the [production-readiness evidence matrix](docs/roadmap/production-readiness.md)
for the explicit open gates.

Start with the [product requirements](MCG_PRD.md) and the
[product and engineering baseline](docs/decisions/0001-product-engineering-baseline.md).

Implementation planning:

- [System architecture](docs/architecture/system.md)
- [M1 runtime spine](docs/architecture/runtime-spine.md)
- [M2 provider fabric](docs/architecture/provider-fabric.md)
- [M3 tool and policy plane](docs/architecture/tool-policy-plane.md)
- [M4 project intelligence and memory](docs/architecture/project-intelligence-memory.md)
- [M5 agent organization and run evidence](docs/architecture/agent-organization.md)
- [Signed organization sharing and publisher trust](docs/architecture/organization-sharing.md)
- [M6 MCP boundary](docs/architecture/mcp.md)
- [M8 platform capabilities and sessions](docs/architecture/platform-sessions.md)
- [M8 Sites, deployment, and monitoring](docs/architecture/sites.md)
- [M8/M10 typed SSH remote execution and multi-host workflows](docs/architecture/typed-ssh.md)
- [Durable owner settings](docs/architecture/settings.md)
- [Portable backup and restore](docs/architecture/backups.md)
- [Secret-free system diagnostics](docs/architecture/diagnostics.md)
- [Interactive, piped, JSON, and streaming consumer CLI](docs/architecture/consumer-cli.md)
- [Native packaging, startup, upgrade, and removal](docs/architecture/packaging.md)
- [Dependency-ordered milestones](docs/roadmap/milestones.md)
- [Production-readiness evidence matrix](docs/roadmap/production-readiness.md)
- [Automated testing strategy](docs/testing/strategy.md)

## Repository documents

- `MCG_PRD.md` — canonical requirements index
- `docs/spec/` — product requirements split by concern
- `docs/decisions/` — durable product and architecture decisions
- `docs/integrations/` — verified development-service connection contracts
- `docs/archive/` — original source documents retained for provenance
- `MEMORY.md` — concise agent handoff and verified repository state

## Development

Prerequisites: Rust 1.93+ and Node.js 24+.

```powershell
npm install
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
npm run check
npm run test
npm run build
npm run test:e2e
```

Build the complete no-install Windows owner-UAT folder with:

```powershell
npm run build:portable:windows
```

This command prepares the release sidecars and runs Tauri's production build
before assembling a versioned `target/portable/MetaCog-Portable-Windows-x64-v*/MetaCog-Desktop-v*.exe`.
The Windows portable output is one self-contained executable. On first launch it verifies and
extracts its version-matched private service helpers beneath the normal MetaCog local data directory;
no adjacent files are required or searched. Do not substitute
`cargo build -p mcg-desktop`: a raw Cargo build can retain Tauri's development
URL instead of embedding the production frontend.

Run the daemon and CLI in separate terminals:

```powershell
cargo run -p mcg-daemon
cargo run -p mcg-cli -- status
cargo run -p mcg-cli -- doctor
cargo run -p mcg-cli -- organization inherit BASE_ID --name "Website specialists"
cargo run -p mcg-cli -- organization history ORGANIZATION_ID
cargo run -p mcg-cli -- organization export REVISION_ID --output team.mcg-organization.json
cargo run -p mcg-cli -- organization import team.mcg-organization.json --import-id REQUEST_UUID
cargo run -p mcg-cli -- organization compare LEFT_REVISION_ID RIGHT_REVISION_ID
cargo run -p mcg-cli -- organization publishers
cargo run -p mcg-cli -- organization review-share signed-team.json
cargo run -p mcg-cli -- organization import-share REVIEW_ID --reason "Reviewed for this project"
cargo run -p mcg-cli -- stop
```

The first-class consumer CLI uses the same durable project/task/run authority as
the desktop:

```powershell
mcg
mcg "fix the failing tests"
mcg -C C:\projects\example "build and verify the website"
git diff | mcg "review this diff"
mcg -C C:\projects\example run --json "fix issue 419"
mcg -C C:\projects\example run --stream-json "fix issue 419"
```

Piped input is bounded and becomes durable owner-supplied task context; do not
pipe credentials. Streaming mode emits task-scoped durable events followed by a
complete result as NDJSON. Existing administrative subcommands remain intact.

MetaCog shares Codex account authentication and supports the same stable login
forms. Browser login is the default; device login works on headless systems, and
API/access tokens are read from standard input so they never appear in process
arguments:

```powershell
mcg login
mcg login --device-auth
$env:OPENAI_API_KEY | mcg login --with-api-key
$env:CHATGPT_ACCESS_TOKEN | mcg login --with-access-token
```

Advanced Settings can point the pinned Codex adapter at an authenticated remote
WebSocket app-server. MetaCog stores only the endpoint and the name of the
environment variable containing the bearer token; it never stores that token.
Unencrypted `ws://` is accepted only for loopback endpoints, while remote hosts
must use `wss://`.

Owner-facing behavior is restart-safe and shared by every interface. Use the
desktop Settings workspace or `mcg settings show|set|history` to select Guided,
Balanced, or Autonomous operation, persist simple/advanced mode, and explicitly
opt into shadow experiments. Updates require the current revision, retain an
immutable reasoned history, and apply to new authority decisions; startup-only
trust roots such as data paths, loopback binding, and executable locations are
not mutable through this surface.

Owner-signed release builds also expose an explicit update flow in Settings. MetaCog
rechecks the reviewed version, verifies the complete package against its embedded
minisign public key, creates and verifies a credential-free backup, stops the local
service cleanly, installs, and restarts. Development and unsigned UAT builds show
updates as unavailable and never fall back to unsigned packages.

The desktop Projects workspace keeps common memory work simple: select a
project, name a fact, and record its value. Advanced mode reveals confidence,
provenance, correction/reinforcement/decay/retraction controls, immutable
history, an optional freshness deadline, the discovered project world graph,
and managed `MEMORY.md` output. Expired facts automatically enter decay through
bounded scheduler batches and retract after a seven-day grace period while
retaining their immutable provenance and history.

Advanced project intelligence also renders a source-backed topology from common
package/Cargo/Python/Go, Docker Compose, Kubernetes, Helm, CloudFormation/SAM,
OpenAPI/Swagger, and deployment
manifests. Named services, databases, dependencies, runtimes, API origins,
deployment targets, and their declared relationships use stable identities and
remain bounded; credential-bearing URLs are never admitted to the graph. The
same readable, provenance-bearing topology is injected into bounded worker and
evaluator context, with dependency relationships retained ahead of lower-value
inventory when compression is required.

Selecting a project also derives a read-only topology plan from the current
immutable snapshot. Only explicit manifest `depends_on` relationships create
ordering; containment, routing, and inventory never invent execution authority.
Independent nodes share a deterministic stage, and any dependency cycle fails
closed without presenting partial executable-looking stages. The plan is visible
in desktop simple/advanced modes and through `mcg project plan PROJECT_ID`, REST,
MCP, and Tauri.

Running a discovered command requires a separate owner review. In the desktop,
choose an exact discovered command and stage, record why it should run, select
**Review this plan**, and then **Run reviewed plan**. The immutable review pins
the project fingerprint, topology digest, and structured executable arguments;
fresh discovery and the ordinary policy/approval broker revalidate those pins
before every step. Advanced CLI users can create the same contract with
`mcg project reviewed PROJECT_ID create REQUEST.json`, then use `list`, `get`,
`run`, or `cancel`. A crash after a process claim is reported as uncertain and is
never automatically retried.

Create a routine portable, secret-free backup from the desktop Settings workspace
or with `mcg backup create`. Advanced mode or `mcg backup create
--include-secrets` can instead include the exact provider/MCP credential catalog
under passphrase-authenticated encryption; the installation-local bearer token is
never included. `mcg backup list` re-verifies stored bundles and `mcg backup verify
PATH` validates a transferred bundle offline. Restore is an explicit offline
recovery operation: run `mcg stop`, then `mcg backup restore PATH --confirm`; add
`--restore-secrets` for an encrypted bundle. The restore retains the destination
token and a pre-restore database in `backups/`. Owner-controlled automation may
use redirected `--passphrase-stdin`; passphrases are never accepted in arguments
or environment variables.

Run the explicit read-only health surface from desktop Settings or with
`mcg doctor`. `mcg doctor --json --strict` is the machine-readable qualification
gate: it validates local storage, authentication, backups, provider discovery,
and capability inventories without contacting providers or including secrets,
paths, identifiers, prompts, or stored content.

The authenticated organization and run-control surface is available through the
same daemon. Create a consumer-default organization, list it, plan work for an
existing task, and inspect or control the resulting run with `mcg organization`
and `mcg run`. Run `mcg organization --help` or `mcg run --help` for the exact
commands and JSON-file schema. After starting a run, `mcg run execute` dispatches
one ready work item through configured providers and project-confined governed
tools; run, work-item, and project UUIDs are explicit positional arguments.
Use `mcg run evaluate RUN evaluation.json` to submit an immutable objective
`EvaluationReport`. Independent-review plans require the report to identify the
run's Evaluator instance. `mcg run get RUN` reads the ordinary durable run view;
`mcg run replay RUN` verifies the task, pinned organization, plan graph,
authority, usage, evaluations, results, events, and tool audit without dispatching
a provider or tool. Add `--json` for the complete digest-bearing evidence report.

Signed organization sharing is opt-in and review-first. `organization
trust-publisher` pins one Ed25519 public key to exact canonical HTTPS registry
origins; `review-share` verifies a package but does not import it. A separate
`import-share` or `reject-share` action requires an owner reason, rechecks the
current trust head, and retains durable digest evidence. Revocation is terminal,
and imported organizations receive fresh local authority rather than inheriting
publisher permissions. See the
[sharing threat model](docs/architecture/organization-sharing.md).

Durable unattended work is controlled with `mcg schedule` and the native
Scheduler dashboard. One-time jobs accept an RFC 3339 timestamp; fixed intervals,
five-field UTC cron, project-relative file watches, durable system events, and
adaptive busy/idle heartbeats share the same occurrence contract. The
daemon claims due work in SQLite, creates the task and occurrence atomically,
and dispatches through the same governed consumer run path as `mcg task launch`.
`mcg schedule list/history/pause/resume/remove` expose persisted state without
deleting execution history. `wake`, `notifications`, and `acknowledge` expose
subsystem attention requests without silently overriding an owner pause. A daemon restart immediately reclaims the same
occurrence and task identity, so the launch path observes existing provider/tool
evidence instead of duplicating side effects. Approval waits and exhausted task
budgets pause recurring admission until the owner explicitly resumes it. New
unattended work defaults to bounded tokens, duration, and model-call count without
inventing a dollar limit for providers whose cost is unknown; advanced desktop
settings can add a cost ceiling and tune those limits. Schedule creation uses a
caller-owned UUID across REST, CLI, desktop, and MCP: an exact retry returns the
existing definition, while reuse for different input fails instead of admitting
duplicate or ambiguous unattended work. CLI creation commands accept
`--schedule-id UUID` when a script or operator needs retry continuity across
separate command invocations.

Local browser and computer-use authority is exposed through the native
Computer & Browser dashboard and `mcg platform`. Capability inventory is a
privacy-safe observed snapshot, not an optimistic feature list. A caller-owned
session UUID makes uncertain request retries idempotent across REST, CLI,
desktop, and MCP. New records remain `requested` until a concrete executor is
attached; daemon restart changes any live authority to `interrupted` rather than
silently resuming control. `list`, `history`, and `close` retain the complete
lifecycle. The persistent browser and bounded computer executor now share that
custody model. Computer actions are confined to an immutable display region and
explicit screen/mouse/keyboard grants, require a freshly matching screenshot
digest, and use caller-owned durable batch IDs. A daemon crash converts a claimed
but unfinished batch to `uncertain` and never automatically repeats host input.
The qualified Linux path currently requires an X11 owner-session `DISPLAY`;
headless Linux and unqualified Wayland sessions report the capability unavailable.

Project-backed web properties are registered in the native Sites dashboard or with
`mcg site`. REST, CLI, MCP, and Tauri share the same caller-owned Site, health-check,
and deployment UUIDs, so an uncertain response can be retried without repeating a
health request or publication process. Health checks are bounded same-origin GETs
with immutable response artifacts. Deployment commands are executable/argument
vectors rather than shell strings, and every publication requires owner approval
before the attempt ledger is claimed or a process starts. Restart-interrupted
deployments become `uncertain` and are never automatically replayed.
Revision-checked updates retain immutable history; archive is terminal. Each Site
can own one durable interval monitor that survives daemon restart, coalesces
repeated outages, and resumes alerts when observed health changes.

The desktop task composer and `mcg task create/list/get/launch/transition/events` share
the same durable projections. Simple mode asks only for a goal; advanced mode can
set constraints, success conditions, risk, and a cost ceiling. Task cards expose
only legal start/cancel controls and load ordered event history plus governed tool
audit from SQLite rather than reconstructing progress in browser state. Simple mode
summarizes objective outcome, checks, tool runs, and artifacts; advanced mode shows
the full event timeline, verification methods, resources, and immutable hashes.
Developer and qualification instances can isolate a second loopback daemon with
`MCG_BIND_ADDRESS` and compile a matching desktop with `MCG_DAEMON_URL`; production
defaults remain `127.0.0.1:7777`, and the API still rejects non-loopback binding.
“Open folder” uses a native
Windows/Linux picker, discovers the repository through the daemon, and attaches
the resulting durable project identity to the task; previously discovered folders
remain selectable after restart and appear with detected language/build/test/Git
context in the Projects panel.

`Start task` is a real orchestration boundary rather than a status-only button.
It reserves one durable run, selects the consumer Coder and independent Evaluator,
derives useful command-based evidence from project discovery, and advances work
through configured providers and governed tools. Repeating Start never duplicates
paid model work, and the UI reports approval, provider, and objective-evaluation
waits without treating a model's completion message as proof.
Blocked work exposes an explicit bounded Retry action in desktop and
`mcg task retry`; ordinary Start replay remains observation-only. Automatic
attempts stop at the work item's retry ceiling, while each authenticated owner
Retry may create one fresh attempt without bypassing active/completed-authority,
dependency, concurrency, capability, budget, or organization-total limits. Cancelling a
launched task atomically cancels the run and terminates active agent authority.
Guided local-tool requests appear on the task card. Approval resumes through the
same durable run after revalidating context and authority; stable request identity
prevents the approved side effect from being duplicated. The CLI equivalent is
`mcg approval approve ID` followed by `mcg task resume TASK_ID`.
Denied/cancelled requests atomically revoke the waiting agent lease and block the
task for an explicit bounded retry.

For projects with discovered build, test, or static-analysis commands, Start now
runs those checks through the governed tool broker after the independent Evaluator
turn. Passing evidence atomically completes the task/run; a nonzero exit atomically
fails them. Task Evidence exposes the durable objective result and check counts,
with per-condition and governed-tool details in advanced mode. Guided mode requests one explicit
approval for the exact supported objective batch and immutable project snapshot.
That checkpoint survives daemon restart; denial blocks for a bounded retry with a
fresh request identity, while approval resumes the batch without repeating model
turns or completed checks.

Tool-loop approvals retain a separate bounded provider-turn checkpoint containing
the pinned route, response, conversation position, redacted tool outcomes, and
exact pending request. After restart, approving a local or external MCP action
continues from that point without replaying the pre-approval model request; the
governed action remains exactly-once and the checkpoint is removed atomically when
its authority ends. Free-form conditions without a typed evaluator remain
nonterminal; model text alone is never promoted to proof. Advanced
success-condition lines can use `FILE EXISTS project/relative/path` to stream a
regular file into bounded content-addressed evidence without exposing its contents
in task history. `COMMAND SUCCEEDS exact discovered command` runs only when the
entire command still matches project discovery; arbitrary custom text is never
executed. `CONFIG VALIDATES exact discovered command` is the typed configuration
counterpart: it is accepted only for an exact currently discovered command and
retains configuration-validation evidence instead of generic custom-command
evidence. Advanced lines can also use
`HTTP STATUS URL` (for example, `HTTP 200 http://127.0.0.1:3000/health`). MetaCog
performs one governed, body-bounded GET to that exact credential-free URL, captures
the response artifact, and compares the observed status exactly, including expected
non-2xx responses. Browser rendering remains a distinct evaluator rather than being
approximated by an HTTP fetch. Use `PAGE RENDERS URL` for a real ephemeral Chromium
render at 1280x720. MetaCog permits only credential-free GET/HEAD traffic to the exact
condition origin, blocks cross-origin requests and state-changing methods before
send, evaluates DOM readiness/layout after JavaScript, and captures rendered HTML
plus a viewport PNG. Verification uses a configured `MCG_BROWSER_EXECUTABLE` or an
installed Chrome/Chromium discovered locally; it never downloads a browser at task
execution time. Linux packages install the managed browser at
`/opt/metacognitive-governor/browser/chrome` with a path-specific AppArmor profile;
MetaCog never disables Chromium's sandbox or Ubuntu's global user-namespace restriction.
For non-HTTP services, use `PORT LISTENING tcp://host:port`. MetaCog performs only a
bounded TCP handshake through the normal capability, lease, approval, and audit
broker; it sends no application bytes, accepts no credentials/path/query/fragment,
and records artifact-free `governed_tcp_port` evidence. This does not substitute
for a protocol-specific health check when one exists.
Use `ELEMENT EXISTS URL selector` to require at least one rendered match after
JavaScript hydration. Use `BROWSER WORKFLOW project/relative/workflow.json` for a
bounded multi-step interaction. The version-1 JSON document declares a name,
credential-free same-origin start URL, and 1-64 typed steps: `assert_element`,
`click`, `type`, `press`, `assert_text`, or `assert_url`. For example:

```json
{
  "schema_version": 1,
  "name": "submit the contact form",
  "start_url": "http://127.0.0.1:3000/contact",
  "steps": [
    { "action": "type", "selector": "#name", "value": "Ada" },
    { "action": "click", "selector": "#submit" },
    { "action": "assert_text", "selector": "#result", "contains": "Thanks, Ada" }
  ]
}
```

MetaCog hashes the workflow before model work and re-reads it through governed project
authority immediately before execution. The interpreter never accepts arbitrary
JavaScript. It confines navigation and assertions to the start origin, allows only
GET/HEAD traffic, and rejects downloads, new browsing contexts, and mutating forms.
The workflow JSON, final HTML, final screenshot, and per-step pass/fail evidence are
retained for audit and replay.
Use `SCREENSHOT MATCHES URL project/relative/baseline.png` for visual regression
evidence. MetaCog binds the baseline SHA-256 before model work, renders the URL at
1280x720, decodes both PNGs with dimension/allocation limits, and compares pixels.
The consumer default tolerates channel noise up to 8 and at most 1% changed pixels;
structured plans retain explicit thresholds. Baseline, actual screenshot, and HTML
remain content-addressed evidence. Missing or changed baselines fail closed.

Use `RESEARCH EVIDENCE project/relative/report.json` for source-backed research.
The report scope must exactly equal the task goal and must contain at least two
distinct credential-free HTTP(S) sources by default. Each claim names one or more
source URLs and a short quotation that MetaCog can locate in the freshly fetched body;
optional conflict records must reference at least two known claim IDs. For example:

```json
{
  "schema_version": 1,
  "scope": "Compare the documented deployment options",
  "claims": [
    {
      "id": "option-a",
      "statement": "Option A supports offline use.",
      "citations": [{
        "url": "https://docs.example.test/option-a",
        "quote": "Option A can run without a network connection.",
        "published_at": "2026-08-01T00:00:00Z"
      }]
    },
    {
      "id": "option-b",
      "statement": "Option B requires a hosted control plane.",
      "citations": [{
        "url": "https://docs.example.test/option-b",
        "quote": "The hosted control plane coordinates every deployment.",
        "published_at": "2026-08-02T00:00:00Z"
      }]
    }
  ],
  "conflicts": [{
    "claim_ids": ["option-a", "option-b"],
    "summary": "The deployment models make different connectivity assumptions."
  }]
}
```

MetaCog first captures the JSON file through governed project read authority, binds its
exact bytes to SHA-256, then authorizes only the declared source origins and fetches
at most 32 sources with a shared timeout and a 1 MiB limit per response. The JSON
and every fetched source remain content-addressed artifacts. Optional structured
conditions can require `max_age_days`; when configured, every citation must provide
an in-range `published_at`. These mechanical checks prove provenance, reachability,
scope, freshness metadata, and quote grounding—not semantic truth. The independent
Evaluator still judges whether sources support the claims and whether meaningful
conflicting evidence was surfaced.

Configure an anonymous local/OpenAI-compatible endpoint with `mcg provider add`.
The native Providers panel supports OpenAI subscription OAuth, OpenAI API keys,
OpenRouter, and arbitrary local or remote OpenAI-compatible endpoints. It discovers
the endpoint catalog before saving, labels subscription/usage/endpoint-reported
billing explicitly, and shows known per-model prices or `Cost unknown`. Connect an OpenAI subscription through the panel or with
`mcg provider login openai-subscription "OpenAI Subscription"`. Use
`--method device-code` on a headless machine. The default desktop and CLI method
opens normal browser sign-in and returns through MetaCog's loopback callback; it does
not require a Codex CLI pairing code. The optional device-code method deliberately
shows a separate one-time pairing code. Subscription OAuth and API-key
billing are separate connections; an OAuth failure never switches billing modes.
For API-key or bearer authentication, select `--auth api-key` or
`--auth bearer-token` and pipe the credential on standard input; credentials are
never accepted as command-line arguments. Use `mcg provider list`, `models`, and
`discover` to inspect the secret-free durable catalog. Use `mcg provider probe
PROVIDER MODEL` to explicitly test and persist live Responses, streaming, and
required function-call behavior before assigning tool-using work.
Use `mcg provider disconnect PROVIDER` (or **Disconnect** in the native Providers
panel) to remove the live connection and all of its protected credentials while
retaining secret-free usage and audit history. Expired subscription access tokens
refresh automatically once per failure burst and retry only the same subscription
route; they never fall back to API billing.

External local harnesses can use MetaCog itself as an OpenAI-compatible provider at
`/v1/models`, `/v1/responses`, and `/v1/chat/completions` with the installation
bearer token. Initial aliases are `mcg:auto`, `mcg:coder`, `mcg:reasoner`,
`mcg:cheap`, and `mcg:local`; see
[`docs/architecture/provider-proxy.md`](docs/architecture/provider-proxy.md) for
routing, streaming, cost, and caller-owned tool semantics.
Completed calls are recorded in a privacy-preserving local ledger without prompt
or generated text. Use `mcg provider usage` and `mcg provider conversations` to
inspect token/cost totals and durable conversation identities; the desktop
Providers workspace exposes the same data.

Modern MCP clients can use the authenticated stateless Streamable HTTP endpoint
at `POST /mcp`. The server exposes deterministic status, project, context,
memory, task/run query, complete consumer-task execution/retry/resume/cancellation,
evidence-validated evaluation, and atomic governed agent-run tools under strict
MCP 2026-07-28 metadata, header, and loopback-Origin validation. Complete MCP
tasks share the desktop/CLI decomposition, independent evaluation, objective
verification, durable replay, and terminal settlement path. Callers can opt
`mcg_run_task` into request-scoped SSE progress with `_meta.progressToken`; MetaCog
streams only durable task event kinds/sequences, returns the final tool result on
the same response, and cancels nonterminal task/run authority if that response is
closed. An owner-local contract test pins the official
`@modelcontextprotocol/client` 2.0.0 package and exercises discovery, bearer
authentication, listing, calls, progress, replay, and stream-close cancellation
against a real MetaCog loopback server. See
[`docs/architecture/mcp.md`](docs/architecture/mcp.md) for the exact contract and
remaining native interoperability work.

MetaCog can also connect to Streamable HTTP or local stdio MCP servers. Use
`mcg mcp add-http ID NAME URL`; select `--auth bearer-token` and pipe the secret
on standard input when needed. `mcg mcp list`, `tools`, and `discover` inspect
the secret-free durable catalog. Public endpoints require HTTPS, credentials are
stored only in the operating-system credential store, and configuration is not
committed unless live discovery succeeds. HTTP automatic mode probes the current
protocol first and can fall back to the `2025-11-25` initialize/session flow;
strict current and legacy pins are available. Local servers use
`mcg mcp add-stdio ID NAME EXECUTABLE -- ARG...`; the executable is spawned
without shell expansion, receives a minimal environment, and uses the same
five-second current probe with same-process legacy fallback. The desktop includes
simple URL-first setup plus advanced HTTP/stdio, id, protocol, bearer, command,
and one-argument-per-line controls with durable tool counts.
Agent work can explicitly request a
discovered tool as `mcp::SERVER_ID::REMOTE_TOOL`; MetaCog exposes a provider-safe
alias only within that lease and executes the call through its normal policy,
approval, audit, and execution-lease plane.

Start the browser-hosted desktop frontend during UI development:

```powershell
npm run dev
```

Run the complete owner-controlled local verification suite (GitHub-hosted builds
are intentionally disabled):

```powershell
.\scripts\verify.ps1
```

```bash
./scripts/verify.sh
```
