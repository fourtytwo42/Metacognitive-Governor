# MCG — MetaCognitive Governor

MCG is a consumer-friendly, cross-platform AI runtime that works directly on
projects and computers while governing how models, agents, tools, memory,
permissions, and budgets are used.

MCG is designed as one persistent Rust runtime with every primary interface:

- Native Tauri desktop application
- Interactive and machine-readable CLI
- Headless daemon
- OpenAI-compatible provider API
- MCP client and server

The system is intentionally model-agnostic. Initial provider targets are OpenAI
API credentials, an isolated ChatGPT/Codex OAuth integration, OpenRouter, local
OpenAI-compatible servers, and arbitrary OpenAI-compatible endpoints.

## Status

MCG is in active implementation. M0 through M5 have qualified core checkpoints,
and M6 interface-family work is active; the full vision remains in scope and is
delivered through tested, end-to-end milestones.

Start with the [product requirements](MCG_PRD.md) and the
[product and engineering baseline](docs/decisions/0001-product-engineering-baseline.md).

Implementation planning:

- [System architecture](docs/architecture/system.md)
- [M1 runtime spine](docs/architecture/runtime-spine.md)
- [M2 provider fabric](docs/architecture/provider-fabric.md)
- [M3 tool and policy plane](docs/architecture/tool-policy-plane.md)
- [M4 project intelligence and memory](docs/architecture/project-intelligence-memory.md)
- [M5 agent organization and run evidence](docs/architecture/agent-organization.md)
- [M6 MCP boundary](docs/architecture/mcp.md)
- [Dependency-ordered milestones](docs/roadmap/milestones.md)
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
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check
npm run test
npm run build
npm run test:e2e
```

Run the daemon and CLI in separate terminals:

```powershell
cargo run -p mcg-daemon
cargo run -p mcg-cli -- status
```

The authenticated organization and run-control surface is available through the
same daemon. Create a consumer-default organization, list it, plan work for an
existing task, and inspect or control the resulting run with `mcg organization`
and `mcg run`. Run `mcg organization --help` or `mcg run --help` for the exact
commands and JSON-file schema. After starting a run, `mcg run execute` dispatches
one ready work item through configured providers and project-confined governed
tools; run, work-item, and project UUIDs are explicit positional arguments.
Use `mcg run evaluate RUN evaluation.json` to submit an immutable objective
`EvaluationReport`. Independent-review plans require the report to identify the
run's Evaluator instance, and `mcg run get RUN` replays the stored report with
the rest of the run evidence.

The desktop task composer and `mcg task create/list/get/transition/events` share
the same durable projections. Simple mode asks only for a goal; advanced mode can
set constraints, success conditions, risk, and a cost ceiling. Task cards expose
only legal start/cancel controls and load ordered event history from SQLite rather
than reconstructing progress in browser state. “Open folder” uses a native
Windows/Linux picker, discovers the repository through the daemon, and attaches
the resulting durable project identity to the task; previously discovered folders
remain selectable after restart.

Configure an anonymous local/OpenAI-compatible endpoint with `mcg provider add`.
Connect an OpenAI subscription through the native Providers panel or with
`mcg provider login openai-subscription "OpenAI Subscription"`. Use
`--method device-code` on a headless machine. Subscription OAuth and API-key
billing are separate connections; an OAuth failure never switches billing modes.
For API-key or bearer authentication, select `--auth api-key` or
`--auth bearer-token` and pipe the credential on standard input; credentials are
never accepted as command-line arguments. Use `mcg provider list`, `models`, and
`discover` to inspect the secret-free durable catalog. Use `mcg provider probe
PROVIDER MODEL` to explicitly test and persist live Responses, streaming, and
required function-call behavior before assigning tool-using work.

External local harnesses can use MCG itself as an OpenAI-compatible provider at
`/v1/models`, `/v1/responses`, and `/v1/chat/completions` with the installation
bearer token. Initial aliases are `mcg:auto`, `mcg:coder`, `mcg:reasoner`,
`mcg:cheap`, and `mcg:local`; see
[`docs/architecture/provider-proxy.md`](docs/architecture/provider-proxy.md) for
routing, streaming, cost, and caller-owned tool semantics.

Modern MCP clients can use the authenticated stateless Streamable HTTP endpoint
at `POST /mcp`. The server exposes deterministic status, project, context,
memory, run-query, and idempotent governed agent-run tools under strict MCP
2026-07-28 metadata, header, and loopback-Origin validation. See
[`docs/architecture/mcp.md`](docs/architecture/mcp.md) for the exact contract and
the remaining interoperability and expanded run-control work.

MCG can also connect to Streamable HTTP or local stdio MCP servers. Use
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
discovered tool as `mcp::SERVER_ID::REMOTE_TOOL`; MCG exposes a provider-safe
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
