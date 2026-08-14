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
When a local tool needs permission, desktop shows the pending request on its task
card. Approve and continue revalidates the live lease, organization, provider
routes, tool permissions, and current project-context fingerprint before restoring
running authority. Tool request ids are derived from the lease plus the canonical
model call, so replay recognizes the approved request and the broker does not
repeat its side effect. The provider turn is currently replayed from its bounded
initial request and can incur another model call. External MCP approval resume and
target-scoped resumes require a persisted provider-turn checkpoint and therefore
fail closed until that checkpoint format is implemented.

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

All builds and tests run on owner-controlled Windows and Ubuntu machines. GitHub is
source hosting only; GitHub Actions is disabled for this repository.
