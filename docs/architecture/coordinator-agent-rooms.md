# Governed inline conversation teams

## Product topology

```text
Owner ⇄ Chat Agent ⇄ Coordinator Agent ⇄ Coding Agent 1..8
                         ↕
              durable exchange + blackboard + governance receipts
```

The owner always addresses the persistent Chat Agent. The Chat Agent is the only role allowed to create owner-visible assistant messages. It may answer conversational requests itself, but all project work and specialist coordination is sent to the persistent Coordinator Agent. The Coordinator decomposes work, starts independently isolated Coding Agents, monitors their messages, and returns internal results to Chat. The metacognitive-governor model role and all unrelated organization roles remain dormant.

## Timeline and visibility

- **Chat** is the frontstage one-to-one transcript. It contains owner messages and Chat Agent responses only.
- The owner sees one chronological transcript. Coordinator and Coding Agent work appears as inline, independently expandable activity widgets after the originating Chat acknowledgement and before Chat's final synthesis.
- Coding Agent cards remain the task-centric projection for preflight, context budget, worktree, checks, evidence, classified failure/recovery, review, apply, retry, cancel, and discard actions. Exceptions open by default; successful worker detail remains compact.
- The Chat timeline keeps a compact live team indicator from Coordinator acceptance through final settlement and delivery. It disappears only after the durable batch and Chat callback are terminal.
- Every internal message has durable sender/recipient session IDs, room, timestamp, optional delegation/correlation identity, and type. No hidden reasoning is exposed.

## Authority

- Chat: `message_coordinator`, scoped context search, and blackboard read/write. No direct Coder delegation or mutation tools.
- Coordinator: `delegate_code`, `message_chat`, `message_coder`, cancellation, scoped context search, and blackboard read/write. No filesystem, shell, Git, browser, computer-use, or built-in nested-agent tools.
- Coder: `message_coordinator`, scoped context search, blackboard read/write, and required `report_result`, plus only the managed-worktree command boundary.
- Server-side role credentials remain authoritative. Prompt or blackboard text cannot add tools. The Coordinator cannot create owner-visible messages, and Coder cannot bypass Coordinator to address Chat or the owner.

## Context and compaction

The complete frontstage transcript, backstage messages, blackboard, project context, and repository instructions remain durable and searchable. Tool traces, routine progress, and presence events remain visible in inline widgets but are not repeatedly embedded in model context. Each agent receives a role-specific context window assembled from:

1. its immutable security and organization prompt layers;
2. the latest per-session/per-room context milestone;
3. new messages addressed to it since that checkpoint;
4. explicitly retrieved attributed context results;
5. the current delegation brief and bounded project context.

Context milestones are append-only summaries with a semantic cause, canonical content hash, source references, approximate token count, and through-message cursor. They are created only for batch sealing, questions/blockers, final review, or a meaningful size threshold. Incremental assembly and hash deduplication reduce prompt size without deleting or hiding source history. Agents can search the full room when a summary is insufficient.

## Lifecycle and limits

The Coordinator assigns stable, friendly generated names to Coders. Join and leave events are durable. Up to eight active Coding Agents may run concurrently, further bounded by the pinned organization revision and available managed-worktree capacity. Each Coder retains its own Codex thread, lease, branch, worktree, context checkpoint, and cancellation authority. Restarts reconcile durable state and never blindly replay accepted turns.

Before any Coder starts, the deterministic runtime compiles a typed completion contract and records a preflight receipt. The contract pins the objective hash, declared scopes, required checks/evidence, integration-readiness requirement, context budget, and attempt ceiling. Preflight verifies the project root, organization capacity, isolation mode, and declared path reservations. Parallel work without explicit disjoint scope reservations fails before execution; overlapping parent/child scopes cannot run concurrently.

Obvious bounded single-Coder requests may use an adaptive deterministic route that skips the initial Coordinator model planning turn. Architectural, research-heavy, ambiguous, or explicitly parallel requests still receive Coordinator planning. The final aggregate Coordinator review remains required, so bypass reduces overhead without removing supervision.

Each delegation records attributed context receipts and outcome telemetry: source count/hash, bounded approximate tokens, truncation, latency, model turns, tool calls, attempts, owner interventions, files, checks, success, typed failure class, and a deterministic recovery directive. These are runtime facts, not model claims.

Each owner project-work turn creates one durable coordination batch. The backstage request, batch, and initial Coordinator wake are one atomic recovery boundary. The initial turn seals the batch after planning; ordinary Coder updates are streamed and stored without waking the Coordinator, while a question or blocker creates one deduplicated high-priority wake. Once every Coder in the sealed batch is reported or terminal, one final Coordinator turn reviews all results together. Settlement revisions are bound to that wake, not copied from model text. One corrective final-review turn is permitted only when required settlement or delivery tools were omitted.

The Coordinator-to-Chat report, Chat wake, and `chat_pending` batch state are also atomic. The Chat Agent receives one synthesis turn and writes exactly one owner-visible completion. If synthesis cannot safely start or finish, the runtime writes a deterministic structured fallback. Restart recovery never blindly replays an accepted model turn: expired uncertain Coordinator work becomes an explicit recovery blocker, while an unfinished Chat callback is harvested when already visible or completed with the deterministic fallback.

An accepted Chat-to-Coordinator project handoff durably marks the Coordinator `working` before app-server startup. If its first completed planning turn fails to call `delegate_code`, MCG issues one bounded corrective Coordinator turn. A second no-delegation completion fails visibly and produces a Chat update; it cannot leave the owner with a permanent but false “accepted” acknowledgement.

MCG-owned Codex threads are tagged in rollout metadata with `thread_source: "mcg"`. Idle Chat and Coordinator threads, and every terminal Coder thread, are archived and unsubscribed so they do not appear in Codex Recents. MCG unarchives a retained role thread immediately before resuming it. Packaged-service startup also archives unarchived threads carrying that exact tag; it never uses a title, prompt, or working-directory heuristic and never deletes Codex history.

## Governor integration boundary

Chat, Coordinator, and Coder prompt policies are compiled from immutable organization revisions. Conversation telemetry feeds the existing supervision/shadow-experiment subsystem; it does not directly change live prompts. Candidate organizations remain evaluation-only until the existing benchmark, confidence, promotion, and regression-reversion gates succeed. The metacognitive-governor model role stays dormant, cannot mutate an in-flight session silently, and never gains ambient execution authority.
