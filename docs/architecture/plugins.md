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
does not use stderr as a data channel. This gives the future daemon launcher an
OS-killable boundary around module validation, compilation, and guest execution.

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

## Remaining integration gates

The execution kernel is only the first M10 boundary. Before plugins become a
consumer feature, MCG still must add:

- daemon/runtime binding to the package-local parent client, including durable
  invocation identity and crash/timeout recording as non-replayable uncertainty;
- content-addressed module installation, signature/provenance policy, immutable
  manifest revisions, enable/disable state, and invocation audit in SQLite;
- explicit broker-issued capability adapters instead of ambient WASI grants;
- REST, MCP, CLI, Tauri, and simple/advanced desktop management surfaces;
- backup/restore, migration, crash-recovery, adversarial, package, and native UAT;
- the complete Wasmtime Apache-2.0-with-LLVM-exceptions license in packages that
  contain the runtime.

Until those gates pass, the kernel remains unavailable to consumer task routing.
That fail-closed absence is intentional and must not be represented as complete
plugin support.
