# Typed SSH Remote Execution

- Status: M8 implementation contract.
- Contract version: `SSH_SCHEMA_VERSION = 1`.
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

## Interfaces And Exit Evidence

Authenticated loopback REST, CLI, MCP, Tauri commands, and the simple/advanced
desktop expose the same host registry, operation request, approval, result, and
history. The OpenAI-compatible provider API does not duplicate host management.

Exit evidence requires real Windows-controller-to-Linux and Linux-controller-to-
Linux scenarios proving pinned-host success, wrong-key rejection, approval before
mutation, argv/control-character confinement, bounded/redacted output, exact replay,
file hash verification, restart uncertainty, interface interoperability, and native
UI usability. No GitHub-hosted runner is used.
