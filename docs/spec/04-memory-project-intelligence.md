# Memory and Project Intelligence

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

## 15. Memory Architecture

Memory should not be a single vector store.

MCG memory should use multiple layers with different purposes.

### 15.1 Layer 1 — Active Context Window

The active model context is the immediate working memory.

As it approaches limits, the harness progressively compresses older context while preserving:

- Current goals.
- Important decisions.
- Unresolved questions.
- Constraints.
- Relevant evidence.
- Active plans.

Compression should be structured and iterative rather than simple truncation.

### 15.2 Layer 2 — Managed MEMORY.md

Each relevant scope may maintain a structured Markdown memory file.

Possible scopes:

- User/global.
- Project.
- Agent role.
- Long-running thread.

The memory file is:

- Human-readable.
- Versioned.
- Managed by the system.
- Suitable for important durable knowledge.
- Diffable.
- Exportable.
- Restorable.

Example:

```text
MEMORY.md
```

or internal equivalents not necessarily exposed in project folders.

A dedicated memory agent may exist, but MCG should be able to decide:

- Whether a memory specialist is warranted.
- Whether memory updates can be performed directly.
- When to create new memory.
- When to consolidate or revise memory.
- When an entry should be removed because it is stale or incorrect.

### 15.3 Layer 3 — Versioned Structured Memory

Durable knowledge should also be represented in structured storage with provenance and version history.

Examples:

- Project facts.
- User preferences.
- Historical decisions.
- Agent lessons.
- Model performance.
- MCG lessons.
- Task outcomes.

### 15.4 Layer 4 — Vector / Semantic Retrieval

Vector storage is useful for relatively stable or large bodies of material such as:

- Codebases.
- Documentation.
- Historical threads.
- Large project corpora.
- Reference material.

Vectors should not be treated as the canonical source for frequently changing operational state.

### 15.5 Memory Lifecycle

Memories should support:

```text
New
↓
Working
↓
Reinforced?
├── No → Decay
└── Yes → Durable
```

Retention should be based on usefulness, recurrence, correction history, and confidence.

Examples:

```text
"npm failed once"
→ likely decay

"this project uses pnpm"
→ durable

"user corrected this assumption three times"
→ strongly durable

"coder repeatedly fails using strategy X"
→ metacognitive memory
```

---

---

## 16. Project Intelligence

Opening or invoking MCG inside a project folder should immediately provide structured project understanding.

### 16.1 Project Discovery

MCG should detect:

- Repository root.
- Git state.
- Languages.
- Build system.
- Test system.
- Package managers.
- Services.
- Docker configuration.
- CI configuration.
- Important directories.
- Recent commits.
- Project instructions.
- Known commands.
- Existing MCG project memory.

### 16.2 Instruction Standards

MCG should support existing ecosystem conventions rather than requiring proprietary conversion.

Initial standards:

- `AGENTS.md`
- `AGENTS.override.md`
- `CLAUDE.md`
- `CLAUDE.local.md`
- `.claude/rules/`
- `.cursor/rules/*.mdc`
- `README.md`
- `CONTRIBUTING.md`

The resolver should support hierarchical scope and applicability.

### 16.3 Conflict Detection

MCG should not blindly concatenate contradictory instruction files.

Example:

```text
AGENTS.md → use pnpm
CLAUDE.md → use npm
.cursor rule → use bun
```

MCG should:

- Detect conflict.
- Determine scope.
- Apply known precedence semantics.
- Prefer more specific applicable rules where appropriate.
- Surface unresolved ambiguity.

### 16.4 Project World Model

Project understanding should go beyond file indexing.

Example:

```text
Frontend
   │ calls
   ▼
API
   │ writes
   ▼
Postgres

GitHub main
   │ deploys
   ▼
Production
```

The world model may include:

- Services.
- Databases.
- Domains.
- APIs.
- Deployment targets.
- Dependencies.
- Repositories.
- Runtime environments.
- Commands.
- Relationships.

---
