# Code Review And Pull Requests

## Consumer workflow

Simple mode treats Pull requests as a repository review workspace, not as a queue
of MCG runtime permissions. A user chooses a Git-backed project and reviews one of
four scopes: the current branch against a base branch, all uncommitted changes,
the current commit, or a custom range and focus. Starting a review opens a
project-bound chat with an explicit read-only instruction.

The review reports prioritized, actionable findings with file and line context.
It focuses on correctness, regressions, security, missing tests, and repository
instructions. Reviewing never modifies the working tree. Fixing a finding is a
separate follow-up; after a fix, the user inspects the resulting diff and chooses
what to stage, commit, and push.

MCG runtime approval requests are a different concept. They remain visible under
`Governed approvals` in Advanced mode and must never be presented as pull requests.

- Status: consumer workflow and GitHub account/repository connection foundation
  implemented; pull-request synchronization remains pending.
- Date: 2026-08-24.

## Alignment baseline

The workflow follows the current Codex review model:

- review a branch against a base, uncommitted changes, a commit, or custom scope;
- return findings without changing the working tree;
- make findings prioritized and line-specific;
- show repository and pull-request context alongside the diff when GitHub is
  connected;
- keep fixing, diff inspection, staging, committing, and pushing as explicit
  follow-up decisions;
- use repository guidance such as `AGENTS.md` to specialize review behavior.

GitHub-hosted automated review is deliberately high signal. Mechanical linting
and deterministic checks belong in local test lanes rather than being disguised
as AI review rules. MCG continues to prohibit GitHub Actions and other paid
GitHub-hosted build or test execution.

## Current implementation boundary

The desktop client can start and retain project-bound local code-review chats now.
It displays review history from durable MCG tasks whose goal is a code review.
Repository discovery supplies Git status, branch, head, commits, and instructions.

MCG now supports browser authorization through a configured GitHub App public
client ID, keeps the bearer token in the operating-system credential vault, lists
authorized repositories, creates an empty owner repository, and links an existing
local Git project without replacing an existing `origin`. New desktop-attached
plain folders are initialized with a local empty baseline commit by default; owner
files are never automatically staged.

Release builders configure the public client identifier with
`MCG_GITHUB_CLIENT_ID`; it may be supplied at compile time for packaged builds or
at service startup for development. MCG deliberately has no client-secret setting:
desktop clients use authorization-code flow with PKCE and a state-validated
loopback callback. If the public client identifier is absent, Settings reports the
missing configuration and sign-in fails closed.

MCG does not yet have a GitHub pull-request synchronization contract. Until that
exists, the UI must not fabricate pull-request titles, comments, checks, changed
files, or review state. The page states this boundary explicitly. Local review of
checked-out pull-request branches remains usable.

## Required GitHub integration

The next pull-request integration must add typed, durable contracts for:

1. Pull-request identity, title, base and head revisions, author, changed files,
   checks, review comments, and refresh time.
2. Diff hunks and line-addressable findings that remain bound to an exact head
   revision and become stale when that revision changes.
3. Explicit commands to request a review, post selected findings, ask for a fix,
   inspect the new diff, and only then stage, commit, or push.
4. Repository-specific review rules sourced from checked-in instructions.
5. Integration tests for authentication loss, force-push staleness, pagination,
   rate limiting, retries, duplicate webhook or refresh delivery, and restart
   recovery.

All builds, tests, and qualification remain local or on owner-provided VMs. The
GitHub integration may read and write pull-request data when authorized, but it
must not create GitHub Actions workflows or consume GitHub-hosted compute.
