# Product Vision and Success

> Canonical split specification. Generated from the original PRD by
> `scripts/split-prd.ps1`; edit the split documents directly after the initial
> generation. The archived original remains available for provenance.

## 1. Executive Summary

MCG is a cross-platform, GUI-first AI harness and cognitive runtime designed to operate as both:

1. A complete autonomous multi-agent system capable of working directly on the host operating system.
2. A provider/proxy layer that other AI harnesses can use to gain MCG's metacognitive governance, memory, project intelligence, model routing, and optimization capabilities.

MCG is designed around a central **MetaCognitive Governor** that does not normally perform user-facing work itself. Instead, it observes the operation of the broader agent organization, evaluates effectiveness, adjusts prompts, roles, models, routing, budgets, permissions, and organizational structure, and learns from historical outcomes.

The guiding architectural principle is:

> Agents think about the world.  
> The Executive thinks about the work.  
> The MCG thinks about the thinking and the organization doing the thinking.

MCG is intended to run close to bare metal, with a small native runtime, strong operating-system access, persistent scheduling, browser/computer use, multi-provider model support, project awareness, robust CLI integration, and a clean native desktop UI.

---

---

## 2. Product Vision

MCG should feel like an AI-native operating environment rather than a chatbot application.

The system should:

- Understand projects automatically when opened in a folder.
- Work directly with files, repositories, shells, browsers, and computers.
- Persist long-running work across threads and scheduled tasks.
- Adapt which models are used for which jobs.
- Continuously evaluate whether its current agent organization is effective.
- Experiment with alternate models, prompts, roles, and topologies.
- Learn from completed work and user corrections.
- Keep internal complexity mostly hidden from normal users.
- Expose enough metrics and organizational information that users understand what the system is doing.
- Offer deeper internals only through developer mode.
- Be usable interactively from a GUI, from a CLI like Codex CLI or Claude Code, through structured automation interfaces, through MCP, and as an OpenAI-compatible provider endpoint.
- Be portable across Linux, Windows, macOS, and Android, with platform-specific capability differences where necessary.

---

---

## 3. Core Design Principles

### 3.1 GUI First, Daemon First Architecturally

The native GUI is the primary human experience.

The actual product core is a persistent native daemon/service that owns state, scheduling, agent execution, memory, model routing, permissions, and tool execution.

The GUI, CLI, provider gateway, and MCP interfaces are clients of the same runtime.

### 3.2 Native and Close to Bare Metal

The preferred implementation stack is Rust for the core runtime.

Primary goals:

- Native compilation.
- Low runtime overhead.
- Fast startup.
- Direct OS integration.
- Strong async/concurrency support.
- Cross-platform builds.
- Minimal dependency on heavyweight interpreted runtimes.

A Tauri-based UI is preferred for a native-feeling desktop application while preserving a Rust backend.

### 3.3 Model-Agnostic Agents

An agent is not a model.

An agent consists of:

- Role.
- Prompt/policy.
- Memory.
- Tools.
- Permissions.
- Context.
- Performance history.
- Relationships to other agents.

The underlying model is an interchangeable cognitive engine selected by MCG.

### 3.4 Objective Outcomes Over Self-Reported Success

Whenever possible, task completion should be evaluated using executable success criteria:

- Tests.
- Health checks.
- Build results.
- Browser flows.
- API responses.
- File state.
- Git diff validation.
- Explicit user confirmation.
- Other externally observable signals.

### 3.5 Self-Improvement Must Be Measurable and Reversible

MCG may alter prompts, model assignments, routing, agent roles, and topology, but meaningful changes must be versioned, observable, testable, and reversible.

### 3.6 User Control Above MCG

The user/owner defines an immutable constitutional policy layer.

MCG may govern the system beneath that policy layer but may not remove, weaken, or bypass the owner's constitutional restrictions.

---

---

## 58. Non-Goals

MCG should not initially attempt to:

- Reimplement an entire operating system.
- Replace every IDE.
- Invent proprietary project instruction formats where existing standards work.
- Treat every task as multi-agent.
- Store all memory in vectors.
- Allow MCG to bypass owner policy.
- Expose internal metacognitive details to normal users by default.
- Depend on one model vendor.

---

---

## 59. Success Criteria

MCG is successful when it can:

1. Be opened on a code/project folder and immediately understand relevant project instructions and structure.
2. Perform useful real-world work through files, shell, git, browser, and eventually computer use.
3. Maintain continuity across threads and scheduled work.
4. Dynamically select and escalate models based on observed effectiveness.
5. Learn which model works best for which type of work.
6. Detect when agents or organizational structures are inefficient.
7. Modify prompts, models, routing, and eventually topology based on evidence.
8. Revert changes that reduce performance.
9. Preserve the evolved organization through versioning and backup.
10. Be used through a clean native GUI.
11. Be invoked from the terminal like Codex CLI or Claude Code.
12. Be called programmatically through structured CLI/API interfaces.
13. Act as an OpenAI-compatible provider for other harnesses.
14. Act as an MCP client and server.
15. Hide unnecessary internal complexity from ordinary users while exposing deep diagnostics in developer mode.
16. Achieve reliable outcomes using the minimum cognitive machinery necessary.

---

---

## 60. Product Identity

**Name:** MCG  
**Meaning:** MetaCognitive Governor

MCG governs the cognitive organization rather than directly controlling every action.

The product should communicate a simple external experience:

> One AI system that understands your projects, remembers what matters, works directly on your machines, chooses the right intelligence for the job, and gets better at organizing itself over time.

Internally, MCG is a versioned, measurable, experimentally improvable cognitive organization.
