# Coordinator-led conversation rooms

## Product topology

```text
Owner ⇄ Chat Agent ⇄ Coordinator Agent ⇄ Coding Agent 1..8
                         ↕
              durable backstage room + blackboard
```

The owner always addresses the persistent Chat Agent. The Chat Agent is the only role allowed to create owner-visible assistant messages. It may answer conversational requests itself, but all project work and specialist coordination is sent to the persistent Coordinator Agent. The Coordinator decomposes work, starts independently isolated Coding Agents, monitors their messages, and returns internal results to Chat. The metacognitive-governor model role and all unrelated organization roles remain dormant.

## Rooms and visibility

- **Chat** is the frontstage one-to-one transcript. It contains owner messages and Chat Agent responses only.
- **Agent Room** is the backstage transcript. It is owner-readable but has no owner composer. It shows named-agent joins/leaves, delegations, questions, answers, reports, and bounded tool-call/result traces.
- Coding Agent cards remain the task-centric projection for worktree, checks, evidence, review, apply, retry, cancel, and discard actions.
- The Chat room keeps a compact live team indicator from Coordinator acceptance through the final Coder settlement. It distinguishes Coordinator planning from active Coding Agents, links to the Agent Room, and disappears only after the durable team projection reaches an idle or terminal state. The desktop continues refreshing during the pre-Coder Coordinator phase instead of waiting for a delegation row to exist.
- Every internal message has durable sender/recipient session IDs, room, timestamp, optional delegation/correlation identity, and type. No hidden reasoning is exposed.

## Authority

- Chat: `message_coordinator`, scoped context search, and blackboard read/write. No direct Coder delegation or mutation tools.
- Coordinator: `delegate_code`, `message_chat`, `message_coder`, cancellation, scoped context search, and blackboard read/write. No filesystem, shell, Git, browser, computer-use, or built-in nested-agent tools.
- Coder: `message_coordinator`, scoped context search, blackboard read/write, and required `report_result`, plus only the managed-worktree command boundary.
- Server-side role credentials remain authoritative. Prompt or blackboard text cannot add tools. The Coordinator cannot create owner-visible messages, and Coder cannot bypass Coordinator to address Chat or the owner.

## Context and compaction

The complete frontstage transcript, backstage messages, blackboard, project context, and repository instructions remain durable and searchable. Tool traces, routine progress, and presence events remain visible in the Agent Room but are not repeatedly embedded in model context. Each agent receives a role-specific context window assembled from:

1. its immutable security and organization prompt layers;
2. the latest per-session/per-room context milestone;
3. new messages addressed to it since that checkpoint;
4. explicitly retrieved attributed context results;
5. the current delegation brief and bounded project context.

Context milestones are append-only summaries with a semantic cause, canonical content hash, source references, approximate token count, and through-message cursor. They are created only for batch sealing, questions/blockers, final review, or a meaningful size threshold. Incremental assembly and hash deduplication reduce prompt size without deleting or hiding source history. Agents can search the full room when a summary is insufficient.

## Lifecycle and limits

The Coordinator assigns stable, friendly generated names to Coders. Join and leave events are durable. Up to eight active Coding Agents may run concurrently, further bounded by the pinned organization revision and available managed-worktree capacity. Each Coder retains its own Codex thread, lease, branch, worktree, context checkpoint, and cancellation authority. Restarts reconcile durable state and never blindly replay accepted turns.

Each owner project-work turn creates one durable coordination batch. The backstage request, batch, and initial Coordinator wake are one atomic recovery boundary. The initial turn seals the batch after planning; ordinary Coder updates are streamed and stored without waking the Coordinator, while a question or blocker creates one deduplicated high-priority wake. Once every Coder in the sealed batch is reported or terminal, one final Coordinator turn reviews all results together. Settlement revisions are bound to that wake, not copied from model text. One corrective final-review turn is permitted only when required settlement or delivery tools were omitted.

The Coordinator-to-Chat report, Chat wake, and `chat_pending` batch state are also atomic. The Chat Agent receives one synthesis turn and writes exactly one owner-visible completion. If synthesis cannot safely start or finish, the runtime writes a deterministic structured fallback. Restart recovery never blindly replays an accepted model turn: expired uncertain Coordinator work becomes an explicit recovery blocker, while an unfinished Chat callback is harvested when already visible or completed with the deterministic fallback.

An accepted Chat-to-Coordinator project handoff durably marks the Coordinator `working` before app-server startup. If its first completed planning turn fails to call `delegate_code`, MCG issues one bounded corrective Coordinator turn. A second no-delegation completion fails visibly and produces a Chat update; it cannot leave the owner with a permanent but false “accepted” acknowledgement.

MCG-owned Codex threads are tagged in rollout metadata with `thread_source: "mcg"`. Idle Chat and Coordinator threads, and every terminal Coder thread, are archived and unsubscribed so they do not appear in Codex Recents. MCG unarchives a retained role thread immediately before resuming it. Packaged-service startup also archives unarchived threads carrying that exact tag; it never uses a title, prompt, or working-directory heuristic and never deletes Codex history.

## Future governor integration

Chat, Coordinator, and Coder prompt policies are compiled from immutable organization revisions. A later metacognitive-governor feature may publish new revisions or adjust future-session prompt policy through the existing revision mechanism. It must never mutate an in-flight session silently or gain ambient execution authority.
