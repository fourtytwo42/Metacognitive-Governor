# Typed SSH Remote Execution

- Status: M8 typed-operation contract plus M10 multi-host workflow implementation.
- Contract versions: `SSH_SCHEMA_VERSION = 1`, `SSH_WORKFLOW_SCHEMA_VERSION = 1`.
- Date: 2026-08-14.

## Purpose And Boundary

SSH is an outbound governed tool for operating an owner-authorized machine. It is
not a listener, a distributed MCG control protocol, or built-in remote access to
the local harness. The loopback daemon remains loopback-only.

Windows and Linux controllers use an installed OpenSSH client through structured
argument vectors. MCG never constructs a local shell command. Initial qualification
targets POSIX remote hosts; a remote platform is explicit and cannot be guessed from
command output.

## Durable Host Identity

A remote host has a caller-owned UUID, bounded display name, DNS name or IP address,
port, username, declared platform, authentication mode, and pinned OpenSSH
`known_hosts` entry. Definitions contain no password, private key, bearer token, or
other credential value. Exact retries replay one definition hash; changed identity
reuse conflicts. Disabling or archiving a host retains its complete history.

Hosts may also carry up to 64 normalized, revisioned routing capabilities such as
`os:linux`, `arch:x86_64`, `tool:cargo`, or `gpu:nvidia`. They are explicit owner
configuration, not claims inferred from model output or host names. The typed
inspection result is the evidence used to verify a declaration before relying on it;
changing capabilities creates a new host revision and invalidates older authority.

Host keys are pinned before a connection can run. MCG does not silently trust on
first use, invoke an interactive authenticity prompt, weaken strict host checking,
or use the user's ambient `known_hosts` file. Key rotation requires an explicit,
revision-checked host update and owner-visible reason.

Authentication initially uses the controller's SSH agent. Agent forwarding is
always disabled. A later key credential adapter may materialize executor-only,
owner-readable temporary key bytes from the OS credential service; key paths and
key bytes must never enter agent/model context or durable records.

On Windows, MCG prefers the native `%SystemRoot%\System32\OpenSSH` client so native
paths and the Windows OpenSSH agent use one dialect. The `ssh-agent` service must be
enabled and the owner's key loaded before execution; MCG does not start the service,
ask for an elevation/password prompt, or fall back to an identity file. Linux uses
the active `SSH_AUTH_SOCK`. Missing agent authority fails closed without a password
prompt.

## Typed Operations

The public contract has distinct operations rather than a free-form shell field:

- capability inspection using fixed, versioned commands;
- command execution with a bounded executable, argument vector, optional absolute
  POSIX working directory, timeout, and output ceiling;
- upload from a capability-confined project file to an absolute remote path;
- download from an absolute remote regular file to a capability-confined local path.

Remote command arguments are encoded with one audited POSIX argument encoder. NUL,
control characters, newlines, shell expansion, environment injection, interactive
TTY allocation, proxy commands, local commands, forwarding, and arbitrary SSH
options are rejected. Transfer paths are absolute, normalized, and cannot contain
control characters. Symlink and overwrite semantics must be explicit.

Inspection is read-only. Remote command execution and upload are external mutations
and always require owner approval, including Autonomous mode. Downloads require
remote-read and local-write capabilities and follow protected-path policy.

## Execution, Evidence, And Recovery

Every operation has a caller-owned UUID and an immutable request hash. Policy,
approval, pre-execution audit, and lease acquisition occur before the operation
ledger is claimed and before OpenSSH starts. Exact terminal retries are observation-
only. Conflicting UUID reuse fails closed.

Bounded stdout/stderr and transferred content become content-addressed artifacts.
Durable evidence records the host UUID/revision, pinned host-key fingerprint,
operation kind, exit status, byte counts, hashes, timestamps, and redacted detail;
it never records credential material. A daemon restart converts a claimed operation
to `uncertain`. MCG never automatically repeats an uncertain remote mutation or
transfer. Safe inspection may be retried only under its original caller identity.

## Multi-Host Workflows

A workflow is an immutable, bounded directed acyclic graph of typed SSH operations.
Each workflow and step has a caller-owned UUID. A step either supplies an exact host
UUID/revision or a non-empty set of required capabilities. For a new capability-
routed workflow, MCG deterministically selects the lowest stable UUID among active
matching hosts and immediately persists that exact host revision. Exact workflow
replay returns the retained selection before consulting later fleet state; it never
silently reroutes. Every step receives a deterministic operation UUID and names zero
or more predecessor steps. Duplicate step or operation identities, missing
dependencies, cycles, empty graphs, partial host pins, unavailable capabilities,
stale revisions, and changed-definition identity replay fail closed.

Ready steps advance through the ordinary SSH broker; workflows never create a
second execution path around policy, approval, leases, artifacts, or audit. The
simple desktop composer creates sequential read-only inspection steps over selected
hosts. Advanced callers can submit the complete typed graph. Execution pauses as
`waiting` at approval or controller-setup boundaries, stops and skips dependents on
failure, and becomes terminal `uncertain` whenever safe completion cannot be proven.
Cancellation cannot race an operation whose remote completion may still be in
flight.

SQLite schema 31 commits workflow projections and state events atomically. Startup
first converts crash-interrupted SSH operations to non-replayable uncertainty, then
reconciles workflow steps from that authoritative operation ledger. It never starts,
retries, or invents remote work during recovery. A completed operation whose
workflow projection was not committed before a crash is adopted deterministically;
repeated recovery is observation-only.

## Interfaces And Exit Evidence

Authenticated loopback REST, CLI, MCP, Tauri commands, and the simple/advanced
desktop expose the same host registry, operation request, approval, result, and
history. Workflows additionally expose list/create/get/run/cancel through REST,
`mcg ssh workflow-*`, MCP query/mutation tools, Tauri, and simple/advanced desktop
controls. The OpenAI-compatible provider API does not duplicate host management.

Exit evidence requires real Windows-controller-to-Linux and Linux-controller-to-
Linux scenarios proving pinned-host success, wrong-key rejection, approval before
mutation, argv/control-character confinement, bounded/redacted output, exact replay,
file hash verification, restart uncertainty, interface interoperability, and native
UI usability. Workflow evidence additionally requires graph validation, exact replay,
stale-pin rejection, deterministic capability routing and no-match refusal, crash
uncertainty with dependent skipping, completed-operation projection reconciliation,
cancellation, and two-host live/package UAT. No
GitHub-hosted runner is used.
