# Plugin execution architecture

MCG supports the PRD's two plugin trust classes through different structural
boundaries. A plugin never chooses its own trust class at runtime, and failure in
one class never falls back to the other.

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

The initial executor grants no host capabilities. It inherits no environment,
host terminal, host stdio, directory preopen, or network address. Unknown imports
fail linking. A manifest requesting filesystem, network, process, or credential
authority is denied rather than partially executed.

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
evidence fails closed. Runtime installation accepts unsigned local/registry
imports but refuses a `verified` signature claim until a trusted cryptographic
verifier exists; consumer routing remains disabled regardless.

Invocation projections retain identities, task linkage, input/result hashes,
bounded error codes, and timestamps rather than prompt or output content. Start
and terminal snapshots are appended to immutable history in the same transaction
as their projection change. Exact retries are observation-only. Runtime startup
turns every interrupted `running` invocation into terminal `uncertain` evidence,
and it never retries that authority automatically.

The runtime captures module bytes in the existing content-addressed artifact
store, re-verifies an existing digest before catalog commit, and installs every
new revision disabled. Only an explicit optimistic enable transition makes the
head invocable. It durably records `running` before calling the adjacent runner.
Successful result JSON is captured and replayed by verified digest; guest failure
stores only a bounded code. Missing/tampered modules fail deterministically,
whereas timeout, malformed process output, or failure to persist an executed
result settles as `uncertain`. Input/output content never enters SQLite. A reused
invocation identity must match plugin, task, and input digest exactly.

Release packaging builds and installs `mcg-plugin-runner` adjacent to
`mcg-service` on Windows and Linux. The daemon's dependency tree still excludes
the executor and Wasmtime; only the sidecar contains them. Development builds do
not depend on generated sidecars, and no consumer interface routes plugin work yet.

## Remaining integration gates

The execution kernel is only the first M10 boundary. Before plugins become a
consumer feature, MCG still must add:

- the trusted signature verifier and install policy for registry/community
  packages; unsigned local imports must remain explicit and disabled by default;
- explicit broker-issued capability adapters instead of ambient WASI grants;
- REST, MCP, CLI, Tauri, and simple/advanced desktop management surfaces;
- plugin-aware backup/restore, migration, additional adversarial package tests,
  and native Windows/Linux UAT;
- the complete Wasmtime Apache-2.0-with-LLVM-exceptions license in packages that
  contain the runtime.

Until those gates pass, the kernel remains unavailable to consumer task routing.
That fail-closed absence is intentional and must not be represented as complete
plugin support.
