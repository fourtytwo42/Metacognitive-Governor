# Plugin execution architecture

MCG supports the PRD's two plugin trust classes through different structural
boundaries. A plugin never chooses its own trust class at runtime, and failure in
one class never falls back to the other.

## Consumer workflow

Simple mode follows the Codex plugin-directory model. Plugins are grouped by
OpenAI, workspace, personal, and installed sources; users search or browse,
review requested access separately, turn an installed plugin on or off, and use
an enabled plugin from a new chat with an explicit `@Plugin` prompt. Raw manifest
and module import, signed-package verification, exact broker scopes, structured
JSON invocation, immutable identities, digests, and invocation history remain in
Advanced mode.

The current daemon has a qualified installed-plugin catalog, local import, and
verified-registry installation boundary. It does not yet synchronize OpenAI's
universal public marketplace or implement third-party connector OAuth. Simple
mode states that boundary instead of displaying fabricated public entries or a
nonfunctional sign-in action. Registry plugins appear under workspace and local
imports under personal; Installed shows both.

Remaining directory parity requires a typed, signed marketplace index with
OpenAI/workspace/personal provenance; install and uninstall mutations; connector
and MCP authentication lifecycle; bundled skill, hook, browser-extension, UI,
and scheduled-template metadata; new-chat/session activation; and runtime
resolution of natural-language or explicit `@` plugin selection. Connector
disconnection remains a separate account action from plugin uninstall. These
extensions must preserve the existing host sandbox, approval, and exact-scope
contracts. They must not use GitHub Actions or other GitHub-hosted compute.

## Trusted native plugins

Trusted native plugins implement `TrustedNativePlugin` and are statically linked
into an MCG build. The host validates their versioned manifest, rejects duplicate
identity, and routes an invocation to one exact registered implementation.
Dynamic-library loading is deliberately excluded: it would require unsafe loading,
weaken reproducible packaging, and let a file replacement silently change native
authority. Native capabilities belong to the host integration and cannot be
requested by a plugin manifest.

## Sandboxed WASI plugins

Third-party plugins are bounded `WASIp1` command modules executed by Wasmtime
45.0.3, the newest patch release compatible with the workspace's Rust 1.93 floor.
One JSON document is provided on an in-memory stdin stream and exactly one JSON
document is required on bounded stdout. Any stderr output makes the invocation
fail; it is never treated as a successful result.

The executor grants no ambient host capabilities. It inherits no environment,
host terminal, host stdio, directory preopen, or network address. Unknown imports
fail linking. Declaring filesystem, network, process, or credential authority in
a manifest does not change the WASI context; declarations only make the bounded
daemon request protocol eligible for separate owner grants.

Every invocation has explicit module, input, output, linear-memory, table,
instance, fuel, and asynchronous wall-time ceilings. Fuel terminates CPU loops;
the asynchronous Wasmtime/WASI path lets the wall timer cancel a guest blocked in
a WASI host call. Store limits reject oversized memories/tables/instances, and
the in-memory pipes prevent unbounded output.

`mcg-plugin-runner` wraps that kernel in a one-request process contract. It reads
at most 32 MiB from stdin, accepts only one versioned JSON document, requires
canonical base64 and the exact lowercase SHA-256 of the module bytes, executes
once, writes one bounded structured response, and exits nonzero on failure. It
does not use stderr as a data channel. This gives the daemon an OS-killable
boundary around module validation, compilation, and guest execution.

The same crate exposes the package-local parent client used by the daemon
integration. It accepts only a non-symlink regular file with the fixed platform
runner name adjacent to the trusted application binary, clears the child
environment, fixes its working directory, bounds and fully drains both output
pipes, and places request writing plus process lifetime under one outer deadline.
Timeout and pipe failures kill and reap the child. The parent accepts a response
only when schema, plugin identity, structured success/failure fields, and exit
status all agree.

All cross-process manifests, limits, requests, results, and errors live in the
lightweight `mcg-plugin-protocol` crate. `mcg-plugin-runner` exposes its parent
client without executor features, while the executable requires the `executor`
feature and links `mcg-plugins` plus Wasmtime. This split is structural: the
daemon can launch and validate the sidecar without embedding the untrusted-module
compiler/runtime in its own process.

## Durable catalog and invocation evidence

Storage migration 26 records each sandboxed module as an immutable revision
bound to its validated manifest, exact lowercase SHA-256, byte size, provenance,
and signature state. A separate optimistic-revision head controls which revision
is active and enabled. Reusing a revision identity with different content,
invoking a disabled or stale revision, mutating history, or deleting catalog
evidence fails closed. Unsigned installation is restricted to explicit local
imports. Registry packages cross a separate `mcg-plugin-trust` boundary: a
configured Ed25519 public key has authority only for exact canonical HTTPS
registry identities, and the detached versioned proof signs a domain-separated
binary payload containing signer identity, registry/package identity, complete
manifest and requested capabilities, module SHA-256, and byte length. The trust
store rejects duplicate signer identities or public keys, cross-registry replay,
noncanonical proof/key encoding, and local or already-verified claims. Only the
verifier can construct the sealed type accepted by durable verified installation.

Invocation projections retain identities, task linkage, input/result hashes,
bounded error codes, and timestamps rather than prompt or output content. Start
and terminal snapshots are appended to immutable history in the same transaction
as their projection change. Exact retries are observation-only. Runtime startup
turns every interrupted `running` invocation into terminal `uncertain` evidence,
and it never retries that authority automatically.

The runtime captures module bytes in the existing content-addressed artifact
store, re-verifies an existing digest before catalog commit, and installs every
new local or verified registry revision disabled. Signature verification occurs
before artifact capture, while byte/digest verification occurs before catalog
commit. Only an explicit optimistic enable transition makes the
head invocable. It durably records `running` before calling the adjacent runner.
Successful result JSON is captured and replayed by verified digest; guest failure
stores only a bounded code. Missing/tampered modules fail deterministically,
whereas timeout, malformed process output, or failure to persist an executed
result settles as `uncertain`. Input/output content never enters SQLite. A reused
invocation identity must match plugin, task, and input digest exactly.

Release packaging builds and installs `mcg-plugin-runner` adjacent to
`mcg-service` on Windows and Linux. The daemon's dependency tree still excludes
the executor and Wasmtime; only the sidecar contains them. Development builds do
not depend on generated sidecars.

## Broker-issued capabilities

Owner grants are durable, optimistic-revisioned, and must cover every capability
declared by the active manifest before a plugin can be enabled. Filesystem access
is scoped only to the invocation task's discovered project, network access to
exact canonical HTTP(S) origins, process access to exact executable identities,
and credential access to exact provider/key references. Grant changes atomically
disable the plugin; enabling it again is always a separate explicit mutation.

A capability-aware guest receives a versioned `PluginHostTurn`, then either
returns a final value or one bounded `PluginCapabilityRequest`. The daemon
validates the request against the immutable manifest and current grants, resolves
project-relative paths without traversal, and translates it to the same
`ToolRequest`/`ToolBroker` path used by governed agents. The normal autonomy
preset, owner constitution, approval repository, leases, artifact store, output
redaction, and immutable tool audit therefore remain authoritative. The runner is
still one-shot and receives no preopen, socket, process, environment, or raw
secret authority.

At most eight capability requests are accepted. Returned tool data is a bounded,
redacted projection; complete artifacts and audit evidence stay in host storage.
Credential references are resolved only inside the HTTP/process executor and raw
values never cross back to the guest. Stable request identities bind the plugin
revision, task, original input digest, and complete capability operation, so an
owner can approve and retry a fresh plugin invocation without authorizing a
changed request. Approval waits settle the current invocation as failed; any
runner or persistence failure after a host operation settles as uncertain and is
never retried automatically.

## Consumer and automation interfaces

The authenticated loopback REST API is the canonical interface for listing,
local import, verified registry installation, complete grant replacement,
explicit enable/disable, stable invocation, and invocation-history reads. Request
bodies and decoded modules are bounded, and local import rejects noncanonical
base64 or mismatched module identities before durable mutation.

The CLI exposes those lifecycle stages as separate `mcg plugin` commands. Tauri
commands proxy the same loopback contracts without granting the webview direct
daemon credentials. The desktop Plugins workspace imports a manifest/module pair
disabled, explains requested capabilities, collects exact scopes, requires a
separate enable action, and invokes only enabled plugins. Advanced mode exposes
immutable identities, digests, invocation history, and signed registry package
import. The MCP server exposes bounded list, configure, and invoke tools; package
bytes remain on REST/CLI/Tauri transports so an MCP tool call cannot become a
large-package upload channel.

## Qualification status

Owner-controlled native Windows and Linux package UAT now executes a real
digest-bound WASI module and proves that a changed module digest fails closed from
each installed plugin runner. The package contract and both installed layouts also
verify the complete Wasmtime Apache-2.0-with-LLVM-exceptions license wherever the
runner is shipped. Cross-platform adversarial source tests cover capability scope,
tamper rejection, restart uncertainty, content-addressed replay, and brokered host
operations.

Plugin support is therefore implementation- and owner-package-qualified for the
current unsigned candidate. Public production release still inherits the product's
global trusted signing and final matched-candidate regression gates; passing an
unsigned owner build does not satisfy those distribution requirements.
