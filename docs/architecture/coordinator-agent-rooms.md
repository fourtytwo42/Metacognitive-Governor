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
- Every internal message has durable sender/recipient session IDs, room, timestamp, optional delegation/correlation identity, and type. No hidden reasoning is exposed.

## Authority

- Chat: `message_coordinator`, scoped context search, and blackboard read/write. No direct Coder delegation or mutation tools.
- Coordinator: `delegate_code`, `message_chat`, `message_coder`, cancellation, scoped context search, and blackboard read/write. No filesystem, shell, Git, browser, computer-use, or built-in nested-agent tools.
- Coder: `message_coordinator`, scoped context search, blackboard read/write, and required `report_result`, plus only the managed-worktree command boundary.
- Server-side role credentials remain authoritative. Prompt or blackboard text cannot add tools. The Coordinator cannot create owner-visible messages, and Coder cannot bypass Coordinator to address Chat or the owner.

## Context and compaction

The complete frontstage transcript, backstage messages, blackboard, project context, and repository instructions remain durable and searchable. Each agent receives a role-specific context window assembled from:

1. its immutable security and organization prompt layers;
2. the latest per-session/per-room context checkpoint;
3. new messages addressed to it since that checkpoint;
4. explicitly retrieved attributed context results;
5. the current delegation brief and bounded project context.

Checkpoints are append-only summaries with source references, an approximate token count, and a through-message cursor. They reduce prompt size without deleting or hiding the source history. Agents can search the full room when a summary is insufficient.

## Lifecycle and limits

The Coordinator assigns stable, friendly generated names to Coders. Join and leave events are durable. Up to eight active Coding Agents may run concurrently, further bounded by the pinned organization revision and available managed-worktree capacity. Each Coder retains its own Codex thread, lease, branch, worktree, context checkpoint, and cancellation authority. Restarts reconcile durable state and never blindly replay accepted turns.

MCG-owned Codex threads are tagged in rollout metadata with `thread_source: "mcg"`. Idle Chat and Coordinator threads, and every terminal Coder thread, are archived and unsubscribed so they do not appear in Codex Recents. MCG unarchives a retained role thread immediately before resuming it. Packaged-service startup also archives unarchived threads carrying that exact tag; it never uses a title, prompt, or working-directory heuristic and never deletes Codex history.

## Future governor integration

Chat, Coordinator, and Coder prompt policies are compiled from immutable organization revisions. A later metacognitive-governor feature may publish new revisions or adjust future-session prompt policy through the existing revision mechanism. It must never mutate an in-flight session silently or gain ambient execution authority.
