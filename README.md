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

MCG is in product and architecture definition. The complete vision is in scope
and will be delivered incrementally through tested, end-to-end milestones.

Start with the [product requirements](MCG_PRD.md) and the
[product and engineering baseline](docs/decisions/0001-product-engineering-baseline.md).

## Repository documents

- `MCG_PRD.md` — canonical requirements index
- `docs/spec/` — product requirements split by concern
- `docs/decisions/` — durable product and architecture decisions
- `docs/integrations/` — verified development-service connection contracts
- `docs/archive/` — original source documents retained for provenance
- `MEMORY.md` — concise agent handoff and verified repository state

## Development

The application workspace has not yet been scaffolded. Planned implementation:
Rust, Tokio, SQLite, Tauri, TypeScript/React, and Playwright, with unit,
integration, contract, scenario, and end-to-end testing treated as product
infrastructure rather than a final release phase.
