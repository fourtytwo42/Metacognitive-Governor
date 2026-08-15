# M1 Runtime Spine

The M1 runtime spine is the first durable vertical slice shared by the daemon,
CLI, desktop, and future protocol adapters.

## Durable task model

- Task envelopes carry an explicit schema version, typed identifiers, timestamps,
  risk, budget, constraints, success conditions, and validated lifecycle state.
- State transitions are closed and explicit. Terminal states cannot restart, and
  illegal edges fail before persistence.
- Event envelopes use a separate schema version, global monotonic sequence, stable
  snake-case event kind, timestamp, optional task identity, and typed JSON payload.

## SQLite transaction and recovery model

`mcg-storage` treats the append-only event log as authoritative and the `tasks`
table as a rebuildable projection. Creating or transitioning a task writes the
event and projection in one SQLite transaction. WAL mode, full synchronous writes,
a busy timeout, and versioned migrations are enabled for file-backed databases.

At startup the daemon applies migrations and rebuilds task projections by replaying
the event log. Replay rejects missing creation events, invalid state transitions,
unknown event kinds, invalid identifiers, and malformed payloads instead of silently
accepting corrupt state.

## Local authentication boundary

The daemon binds only to a loopback address. Health is public to local process
supervisors; every status, task, history, and streaming route requires a bearer
token. The daemon generates a 256-bit token on first start, stores it outside the
repository in the MCG data directory, never logs it, and reuses it across restarts.
On Unix the token file is created with mode `0600`.

The CLI reads the same token file automatically. `MCG_AUTH_TOKEN` and `--token-file`
are explicit overrides for controlled tests and alternate local clients.

## Versioned API

| Method | Route | Purpose |
| --- | --- | --- |
| `GET` | `/api/v1/system/health` | Unauthenticated local liveness probe |
| `GET` | `/api/v1/system/status` | Recovery and runtime status |
| `POST` | `/api/v1/tasks` | Atomically create a task and event |
| `GET` | `/api/v1/tasks` | List current projections in deterministic newest-updated order |
| `GET` | `/api/v1/tasks/{id}` | Read the task projection |
| `POST` | `/api/v1/tasks/{id}` | Apply a validated state transition |
| `POST` | `/api/v1/tasks/{id}/launch` | Idempotently plan, start, and advance consumer work |
| `POST` | `/api/v1/tasks/{id}/retry` | Explicitly retry bounded incomplete work for a blocked task |
| `POST` | `/api/v1/tasks/{id}/resume` | Revalidate and continue an owner-approved local-tool turn |
| `GET` | `/api/v1/tasks/{id}/events` | Read ordered durable task history |
| `GET` | `/api/v1/events/stream` | Receive committed events over SSE |

The SSE endpoint emits sequence IDs and stable event names. A reconnecting client
can supply `?after=<sequence>` or `Last-Event-ID` to replay durable missed events
before switching to the live broadcast stream.

The native desktop uses this same authenticated boundary. Simple mode creates a
goal-only pending task, lists current projections, starts or cancels legal states,
and loads durable event history on demand. Advanced mode adds one-per-line
constraints and success conditions, explicit risk, an optional cost ceiling, and
stable task ids. A dedicated Rust command opens the platform-native Tauri folder
dialog, returns only the selected local path, performs daemon-side project
discovery, and associates the durable project id with the task. Existing projects
are loaded after restart and can be reselected without granting browser code a
general filesystem scope. Start requires that durable project identity; it
atomically binds the task to exactly one consumer organization/run, derives
candidate build/test/static-analysis conditions from discovered commands, and
advances Coder then independent Evaluator turns through the existing provider
and tool authority boundaries. A repeated launch observes durable progress and
does not issue duplicate model work. Model output is never objective completion:
the run remains visibly awaiting evaluation until evidence is recorded. The CLI
mirrors the flow with `mcg task create --project-id ...` and `mcg task launch`.
Provider/tool failures move the task to `blocked`; retry is an explicit owner
action and each work item is capped at its durable lease retry limit. Cancelling
a launched task atomically cancels its run and revokes active agent instances and
leases, so the task projection cannot leave hidden execution authority behind.
Denied or cancelled approvals are also one atomic authority settlement: the
approval becomes terminal, the waiting instance and retained lease fail, the run
returns to its retryable running state, and the task becomes blocked with ordered
events in the same SQLite transaction.
When a local tool needs permission, desktop shows the pending request on its task
card. Approve and continue revalidates the live lease, organization, provider
routes, tool permissions, and current project-context fingerprint before restoring
running authority. Tool request ids are derived from the lease plus the canonical
model call, so replay recognizes the approved request and the broker does not
repeat its side effect. SQLite migration 12 checkpoints the exact bounded model
request/response, pinned route, attributed model calls, redacted completed tool
outcomes, next tool position, and pending request identity before approval can
pause the turn. Restart preserves only waiting authority backed by that checkpoint
and a matching unexpired pending/approved request. Resume revalidates current
project context, role, catalog, tool schema/classification, and exact approval,
then continues from the checkpoint without replaying the paid provider call.
Local and external MCP tools use the same continuation path; completion,
rejection, failure, and cancellation remove the checkpoint atomically.
Checkpoints are private local working data and can contain bounded project/model
content, but never provider credentials or authorization headers.

After all planned Coder and independent Evaluator turns complete, command-based
`Build`, `Test`, and `StaticAnalysis` conditions are executed immediately through
typed `process.exec` requests. Runtime accepts only commands that still exactly
match current project discovery, gives each condition a stable run-scoped request
identity, and routes it through capability policy, immutable tool audit, and
durable execution leases. A validated independent `EvaluationReport` carries the
redacted exit evidence. SQLite records that report and changes the task/run to
completed or failed in one transaction; an inconclusive report blocks the task
while leaving the run retryable. Start replay observes terminal evidence and never
reissues model or objective work. Guided mode durably checkpoints evaluation and
requests one approval for the exact supported objective batch against the immutable
project snapshot. Denial blocks the task and bounded retry creates a fresh approval
identity; approval delegates only that batch, and resume after daemon restart
continues from the stored condition index without another provider turn or repeated
completed check. Conditions without a deterministic evaluator remain visibly
awaiting evaluation rather than accepting model self-report as objective proof.

`FILE EXISTS project/relative/path` becomes a typed `FileExists` condition. Runtime
rejects absolute, traversing, and non-normalized paths, opens only a regular file
under the registered project capability, and streams at most 1 GiB into the
content-addressed artifact store. The report records size and SHA-256 but not file
contents; structured plans may additionally require a minimum size and exact digest.
`COMMAND SUCCEEDS exact command` is the only executable custom-text form. The full
command must still exactly match current project discovery, including safely named
manifest scripts, and executes as structured process argv through the same policy,
lease, approval, and audit path. Other custom prose remains inconclusive.

An advanced consumer success-condition line in the exact form `HTTP STATUS URL`
becomes a typed `HttpResponds` condition. Runtime grants only that parsed origin,
constructs a server-owned credential-free GET with a 1 MiB body limit, and sends it
through the same policy, lease, audit, redaction, redirect, and artifact plane as
agent HTTP tools. Same-origin redirects remain bounded and cross-origin redirects
fail closed. Evaluation compares the `http.status` verification value rather than
generic 2xx success, so an expected 404 can pass while a reachable status mismatch,
timeout, or connection failure settles as failed objective evidence.

`SERVICE HEALTHY URL` becomes a typed `ServiceHealthy` condition for the common
case where the exact successful status is intentionally not pinned. Runtime grants
only the credential-free endpoint origin and performs the same bounded, audited,
same-origin GET as other HTTP objectives. A final 200-299 status is healthy; every
reachable 4xx/5xx response is a measured unhealthy failure with an immutable body
artifact, while connection and timeout failures retain no fabricated response
artifact. The report uses distinct `governed_service_health` evidence and records
the observed status. Terminal launch replay performs neither another provider call
nor another health request.

`PAGE RENDERS URL` becomes a typed `PageRenders` condition backed by an actual
ephemeral Chromium CDP session, not an HTTP-body heuristic. The browser uses a fresh
profile, bounded viewport/time, no automatic download, and pre-send interception.
Only credential-free GET/HEAD requests to the exact condition origin plus non-network
`data:`/`blob:` resources continue; cross-origin requests and mutating methods are
blocked. Runtime verifies ready state and body layout after JavaScript, captures
post-render HTML and a viewport PNG as content-addressed artifacts, and records the
screenshot hash in evaluation evidence. Missing local browser runtime is inconclusive
and blocks for setup/retry; a browser that launches but cannot render the page fails
the objective.

`ACCESSIBILITY CHECK URL` becomes a typed `Accessibility` condition using the same
fresh-profile, same-origin, GET/HEAD-only Chromium boundary. MCG accessibility
baseline v1 audits the hydrated DOM for a document language and non-empty title,
exactly one main landmark, image alternative attributes, names for form controls and
interactive elements, unique IDs, sequential heading levels, and the absence of
positive `tabindex` values. The full versioned JSON audit is content-addressed beside
the rendered HTML and PNG; the evaluation points to that audit rather than a prose
claim. A baseline failure is an objective failure, while a missing browser remains an
honest setup boundary. This deterministic baseline reduces manual regression work but
does not claim complete WCAG conformance or replace assistive-technology UAT.

`ELEMENT EXISTS URL selector` uses the same browser boundary but adds a bounded CSS
selector assertion after hydration. `BROWSER WORKFLOW project/relative/file.json`
binds a versioned JSON workflow and SHA-256 before model work. Evaluation first
re-reads the file through project-scoped filesystem authority and rejects any byte,
artifact, name, or parsed-document mismatch. Its interpreter supports only bounded
element/text/URL assertions plus click, type, and allowlisted key presses; arbitrary
JavaScript is not a workflow operation. Every asserted URL and post-step page URL
must remain on the start origin. Request interception permits only same-origin
GET/HEAD plus `data:`/`blob:` resources, while interaction inspection rejects
downloads, new contexts, cross-origin destinations, and non-GET forms. Final JSON,
HTML, PNG, and ordered step evidence are content-addressed and audited.

On Ubuntu with restricted unprivileged user namespaces, the Linux package places its
browser at a root-owned stable path and installs the repository's path-specific
AppArmor profile granting `userns` to that executable. Development and release gates
use the same layout. `--no-sandbox` and global AppArmor/sysctl weakening are forbidden.

`SCREENSHOT MATCHES URL project/relative/baseline.png` becomes a typed visual
regression condition. Launch streams and hashes the regular baseline before reserving
model work, binding that digest into the durable plan. Evaluation revalidates the
digest, renders at the fixed viewport under the same network confinement, decodes only
bounded PNG dimensions/allocations, and compares RGBA pixels with explicit per-channel
and changed-area thresholds. The default accepts channel deltas through 8 and no more
than 100 basis points (1%) of changed pixels. Dimension changes fail. HTML, actual PNG,
and baseline PNG are all content-addressed; the report points to the actual screenshot.

`RESEARCH EVIDENCE project/relative/report.json` is evaluated in two governed
stages. First, runtime reads and artifacts the bounded UTF-8 JSON file under project
read authority. It parses the exact bytes, binds the declared scope to the immutable
task goal, and derives the complete credential-free source set before granting any
network authority. Second, typed `research.verify` requests exact read authority for
the evidence path and each declared network origin, rechecks the JSON/artifact hash,
and fetches every distinct URL through the hardened HTTP executor. The verifier
accepts at most 128 claims, 256 citations, and 32 sources; each response is limited
to 1 MiB and all fetches share the objective timeout. It validates unique claim IDs,
at least one bounded quote per claim, the configured minimum distinct source count,
optional publication-age limits, and conflict records that reference at least two
known claims. The evidence JSON is the report's primary artifact and fetched source
bodies are retained on the audited tool completion. Structural and quote-grounding
evidence complements rather than replaces the independent Evaluator's semantic
judgment. Terminal replay performs neither another model call nor another fetch.

## Verification evidence

Automated tests cover schema round trips, legal and illegal transitions, injected
projection failure with event rollback, projection repair, reopen persistence,
authentication rejection, live streaming, durable stream replay, non-loopback bind
rejection, and an abrupt daemon-server restart followed by API projection/history
comparison. Consumer-launch fixtures additionally prove atomic one-run
reservation, restart durability, deterministic project-command success
conditions, missing-provider reporting, and retry idempotency.
The approval fixture proves that a guided-mode write remains absent while pending,
executes after approval, advances independent evaluation, and is not repeated by
a later Start replay.
The objective fixture uses real cross-platform Node/npm processes to prove passing
build/test/check conditions atomically complete a task, a nonzero test exit
atomically fails it, evidence is durable, and terminal replay adds neither model
calls nor tool audit records. A file-backed guided fixture denies and retries the
batch with distinct identities, approves it, restarts the daemon while waiting,
then resumes exactly once and verifies the checkpoint is removed only by atomic
terminal settlement.
The HTTP fixture proves an expected non-2xx response passes with a captured artifact,
a status mismatch fails, and an unreachable origin records failed tool audit plus a
terminal failed report instead of escaping as an API error.
The service-health fixture separately proves a 204 response passes, a captured 503
response fails as unhealthy, an unreachable endpoint fails without response evidence,
unsafe endpoint syntax stays non-automatable, and terminal replay adds neither a
provider call nor another network request.
The browser fixtures use real Chromium to prove JavaScript hydration, DOM/title
mutation, HTML/PNG capture, policy/audit integration, and pre-send blocking of a
cross-origin subresource plus same-origin POST.
The accessibility fixture proves a conforming hydrated page passes with versioned
JSON/HTML/PNG evidence, multiple independent violations fail with exact counts,
credential-bearing or ambiguous directive syntax stays non-automatable, and terminal
replay adds neither provider nor page requests.
The artifact fixture proves a binary file is streamed without stdout exposure,
deduplicated by digest, bounded by the evidence limit, and combined with an exact
discovered custom script. A missing file atomically fails the task/run with audited
execution evidence.
The screenshot fixture captures a real Chromium baseline before project discovery,
rejects a missing baseline before any model call, proves an unchanged JavaScript page
passes decoded-pixel comparison, proves a different page fails, and proves mutation
after objective binding is rejected by digest revalidation.
The research fixture binds its JSON scope to the exact task goal, fetches two real
local HTTP sources, grounds quotations in HTML and plain text, retains all three
artifacts, rejects a fabricated quotation, and proves terminal replay adds no model
call, fetch, or tool audit record.

All builds and tests run on owner-controlled Windows and Ubuntu machines. GitHub is
source hosting only; GitHub Actions is disabled for this repository.
