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

MCG is in active implementation. M0 foundations and the M1 durable runtime spine
are complete; the full vision remains in scope and is delivered through tested,
end-to-end milestones.

Start with the [product requirements](MCG_PRD.md) and the
[product and engineering baseline](docs/decisions/0001-product-engineering-baseline.md).

Implementation planning:

- [System architecture](docs/architecture/system.md)
- [M1 runtime spine](docs/architecture/runtime-spine.md)
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
