# System Diagnostics

MCG exposes one versioned, authenticated, read-only diagnostic report through
`GET /api/v1/system/diagnostics`, the native Settings workspace, the Tauri
bridge, and `mcg doctor`. It is intended for owner self-service, support, and
automated installation gates.

The report checks SQLite integrity and schema compatibility, loopback bearer
authentication, every portable backup, provider/model discovery inventory,
optional MCP and platform capability inventory, and durable task, schedule, and
approval counts. Checks have stable codes and `pass`, `warning`, or `error`
severity. The overall result is the highest severity. The current report schema
version is `1`.

Diagnostics never call a model provider, discover a remote endpoint, execute a
tool, or write application state. They report counts and capability summaries,
not content. Reports exclude credentials, bearer tokens, filesystem paths,
connection identifiers, model inputs and outputs, task goals, event payloads,
and artifact contents. A backup verification failure is described generically
so an underlying filesystem error cannot leak a local path.

`mcg doctor` prints a concise human report. `mcg doctor --json` emits the exact
versioned contract. Errors always produce a failing exit status; `--strict`
also treats warnings as failure, which makes it suitable for release and fleet
qualification. A fresh installation normally warns until at least one model
catalog and one verified portable backup exist.

The simple desktop interface shows only findings that need attention. Advanced
mode shows every check, schema/platform metadata, and content-free inventory
counts. Neither view automatically runs diagnostics, so potentially expensive
backup hashing remains an explicit owner action.
