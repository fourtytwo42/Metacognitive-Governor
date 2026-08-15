# Sites, Deployment, And Monitoring

- Status: M8 implementation contract.
- Contract version: `SITE_SCHEMA_VERSION = 1`.
- Date: 2026-08-14.

## Purpose

A Site is durable deployment knowledge for one web property, not a bookmark and
not a remote MCG daemon. It binds a consumer-facing URL to a discovered project,
an environment, a hosting provider label, an optional exact deployment command,
an optional opaque credential reference, and a credential-free health endpoint.
Every primary interface reads and mutates the same SQLite-backed contract.

## Definition And Revision Rules

Site creation uses a caller-owned UUID and a server-generated SHA-256 definition
hash. Exact retries return the original record; changed reuse conflicts. Updates
require the current revision and create an immutable revision record in the same
transaction that advances the projection. History cannot be edited or deleted.

The public and health URLs must be credential-free HTTP(S). Userinfo and fragments
are rejected. Health URLs must share the public URL's origin. Project association
uses a durable project UUID rather than accepting an ungoverned repository path.
Deployment working directories are project-relative and cannot traverse upward.
Executable plus argument vectors are stored structurally; shell command strings
are not accepted. Provider and credential-reference fields are bounded labels and
never contain credential values.

Sites can be active, monitoring-paused, or archived. Archival preserves revisions,
deployment attempts, health observations, logs, notifications, and related task
evidence.

## Deployment Authority

Every deployment is external publication and therefore requires owner approval,
including in Autonomous mode. The immutable approval displays the exact site,
environment, provider, project, executable, arguments, and revision. Approval,
execution lease, pre-execution audit, and deployment claim must commit before the
host process starts.

A caller-owned deployment UUID has one request hash and one terminal disposition:
`succeeded`, `failed`, or `uncertain`. Startup converts a crash-interrupted
`executing` attempt to `uncertain`; it is never automatically replayed. Exact
successful retries are observation-only. Output is bounded, redacted, and captured
as content-addressed artifacts; durable records never contain credentials.

Credential references will resolve through an executor-owned OS credential service
and inject only into the child environment. Until a provider adapter implements a
declared credential binding, a credential-bearing deployment fails as unavailable;
it never exposes a secret to a model, request, approval, log, or audit record.

## Health, Logs, And Monitoring

Health checks are bounded exact-origin `GET` requests with redirect enforcement,
timeouts, response-size ceilings, and content-addressed evidence. A check records
status code, latency, observed health, and artifact identity. The response body is
not copied into the site projection.

Monitoring reuses the durable scheduler rather than adding a second timer system.
One site monitor owns one interval schedule and creates idempotent health-check
occurrences. State changes emit durable notifications; repeated identical failures
are coalesced while history remains immutable. A daemon restart can repeat a safe
read but cannot duplicate a deployment side effect.

Logs are immutable bounded artifacts associated with deployment attempts or
monitor observations. Provider-specific log collection must declare a typed,
credential-isolated adapter; arbitrary shell scraping is not a log API.

## Interfaces And Exit Evidence

Authenticated loopback REST, CLI, MCP, native Tauri commands, and the simple/
advanced desktop Sites dashboard expose the same registry, revisions, deployment,
health, monitoring, and log evidence. The OpenAI-compatible provider API does not
duplicate management operations.

Exit evidence requires Windows and Linux scenarios proving registration replay and
conflict, revision history, production approval before process execution, crash
recovery without deployment replay, exact-origin health evidence, monitoring state
change notifications, interface interoperability, and real native UI use.
