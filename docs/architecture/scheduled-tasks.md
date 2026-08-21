# Scheduled Tasks

## Consumer workflow

Simple mode treats Scheduled as an inbox for unattended work. A user starts by
describing the work in a project-aware chat. The setup conversation must clarify
the cadence, whether each run starts a separate chat or returns to the setup chat,
the stopping conditions, and the minimum permissions needed. The prompt should be
tested manually before the schedule is enabled, and the first runs should be
reviewed and refined.

Scheduled lists active, paused, and completed tasks, supports search and state
filters, shows the next run, exposes recent runs, and places unread findings in a
Needs attention inbox. Pausing and resuming remain immediate consumer actions.
Runtime trigger fields, exact budgets, wake requests, occurrence identities, and
durable diagnostic state remain in Advanced mode.

- Status: consumer inbox and chat setup implemented; execution-parity extensions
  listed below remain pending.
- Date: 2026-08-21.

## Local execution boundary

Project schedules require the owner computer to remain on, MCG to remain running,
and the project directory to remain available. Current MCG schedules execute in
one selected project. The Simple interface states this directly and does not claim
that dedicated worktrees, multi-project execution, or chat-return destinations
already affect the scheduler runtime.

Schedules run unattended under the persisted owner policy. The setup conversation
must recommend the narrowest authority that can succeed. MCG's existing policy,
approval, budget, lease, crash-recovery, and immutable occurrence contracts remain
authoritative. A background run cannot broaden authority merely because it was
scheduled.

## Alignment baseline

The consumer workflow follows the current Codex Scheduled model:

- schedules can be created or updated conversationally;
- standalone schedules create independent result chats, while chat schedules
  return to the same conversation context;
- Scheduled is an inbox for definitions, recent runs, and unread findings;
- tasks can be active, paused, or completed;
- prompts are tested before automation and early runs are reviewed;
- local Git schedules can choose the main project or an isolated worktree;
- skills and plugins can supply reusable behavior and connected context;
- exact model, reasoning, custom recurrence, and permission settings are optional
  advanced controls.

## Remaining parity work

The durable scheduler needs versioned extensions for:

1. standalone-versus-chat destination identity and creation of one durable chat
   per standalone run;
2. dedicated Git worktree execution, cleanup, retention, and stale-base behavior;
3. one schedule targeting multiple projects without duplicating one paid or
   side-effecting run accidentally;
4. RFC 5545 recurrence rules and local-time/time-zone presentation while retaining
   deterministic UTC admission;
5. explicit model and reasoning selection with honest provider availability and
   cost metadata;
6. skill and plugin binding that is revision-pinned for each admitted occurrence;
7. a first-party conversational scheduler tool so a setup chat can atomically
   create or update the reviewed proposal instead of returning a manual handoff;
8. archive and cleanup policy for chats, artifacts, and worktrees created by
   frequent schedules.

All scheduler builds and tests remain local or on owner-provided VMs. Scheduled
work must not create GitHub Actions or use GitHub-hosted compute.
