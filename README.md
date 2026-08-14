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

The desktop task composer and `mcg task create/list/get/launch/transition/events` share
the same durable projections. Simple mode asks only for a goal; advanced mode can
set constraints, success conditions, risk, and a cost ceiling. Task cards expose
only legal start/cancel controls and load ordered event history from SQLite rather
than reconstructing progress in browser state. “Open folder” uses a native
Windows/Linux picker, discovers the repository through the daemon, and attaches
the resulting durable project identity to the task; previously discovered folders
remain selectable after restart.

`Start task` is a real orchestration boundary rather than a status-only button.
It reserves one durable run, selects the consumer Coder and independent Evaluator,
derives useful command-based evidence from project discovery, and advances work
through configured providers and governed tools. Repeating Start never duplicates
paid model work, and the UI reports approval, provider, and objective-evaluation
waits without treating a model's completion message as proof.
Blocked work exposes an explicit bounded Retry action in desktop and
`mcg task retry`; ordinary Start replay remains observation-only. Cancelling a
launched task atomically cancels the run and terminates active agent authority.
Guided local-tool requests appear on the task card. Approval resumes through the
same durable run after revalidating context and authority; stable request identity
prevents the approved side effect from being duplicated. The CLI equivalent is
`mcg approval approve ID` followed by `mcg task resume TASK_ID`.
Denied/cancelled requests atomically revoke the waiting agent lease and block the
task for an explicit bounded retry.

For projects with discovered build, test, or static-analysis commands, Start now
runs those checks through the governed tool broker after the independent Evaluator
turn. Passing evidence atomically completes the task/run; a nonzero exit atomically
fails them. Task History exposes the durable objective result and check counts,
with per-condition details in advanced mode. Guided mode requests one explicit
approval for the exact supported objective batch and immutable project snapshot.
That checkpoint survives daemon restart; denial blocks for a bounded retry with a
fresh request identity, while approval resumes the batch without repeating model
turns or completed checks. Free-form conditions without a typed evaluator remain
nonterminal; model text alone is never promoted to proof. Advanced
success-condition lines can use `FILE EXISTS project/relative/path` to stream a
regular file into bounded content-addressed evidence without exposing its contents
in task history. `COMMAND SUCCEEDS exact discovered command` runs only when the
entire command still matches project discovery; arbitrary custom text is never
executed. Advanced lines can also use
`HTTP STATUS URL` (for example, `HTTP 200 http://127.0.0.1:3000/health`). MCG
performs one governed, body-bounded GET to that exact credential-free URL, captures
the response artifact, and compares the observed status exactly, including expected
non-2xx responses. Browser rendering remains a distinct evaluator rather than being
approximated by an HTTP fetch. Use `PAGE RENDERS URL` for a real ephemeral Chromium
render at 1280x720. MCG permits only credential-free GET/HEAD traffic to the exact
condition origin, blocks cross-origin requests and state-changing methods before
send, evaluates DOM readiness/layout after JavaScript, and captures rendered HTML
plus a viewport PNG. Verification uses a configured `MCG_BROWSER_EXECUTABLE` or an
installed Chrome/Chromium discovered locally; it never downloads a browser at task
execution time. Linux packages install the managed browser at
`/opt/metacognitive-governor/browser/chrome` with a path-specific AppArmor profile;
MCG never disables Chromium's sandbox or Ubuntu's global user-namespace restriction.

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
