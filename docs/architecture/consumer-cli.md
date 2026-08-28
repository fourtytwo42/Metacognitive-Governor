# Consumer CLI

The `mcg` executable is both a consumer task interface and an administrative
client for the same authenticated loopback daemon used by the desktop.

Consumer entry points are:

```text
mcg
mcg "fix the failing tests"
mcg -C path/to/project "review this repository"
git diff | mcg "review this diff"
mcg -C path/to/project run --json "fix issue 419"
mcg -C path/to/project run --stream-json "fix issue 419"
```

With a terminal and no arguments, `mcg` opens a persistent, durable conversation;
`Ctrl+C`, `/exit`, `/quit`, `:exit`, and `:quit` close it. Typing `/` at the start
of the composer opens the interactive command picker. Typing `@` at a token
boundary opens bounded project-reference and enabled-plugin search. Arrow keys
move through either picker, Enter or Tab selects, Escape dismisses, and selected
mentions are sent through the same structured validation and pinning used by the
desktop. Literal email addresses remain ordinary text.

Interactive `/plan` enables the native read-only planning mode and presents any
multi-question choice requests in the terminal, with the agent's recommended
answer first. `/goal` can set, view, edit, pause, resume, or clear the durable
goal. `/mention` attaches a file or folder to the next message; plugins remain
searchable through `@`. The remaining interactive commands expose the same supported local conversation
controls as the desktop; `/help` is the authoritative inventory. Explicit
`conversation send --mention file:...|folder:...|plugin:...` remains available
for scripts and non-interactive callers.

Without a terminal, argument-free standard input becomes the goal. When a goal
is also present, piped UTF-8 is attached as owner-supplied task context. Standard
input is rejected above 1 MiB and goals are rejected above 32,768 characters.

`-C/--project` selects a directory and defaults to the current directory. MCG
canonicalizes and discovers a new project, or refreshes the matching existing
project before creating the task. Repeated CLI use therefore reuses one durable
project identity while observing current instructions and manifests.

Consumer launches reserve 8,192 output tokens per model call by default. This
allows a normal generated source file and its complete JSON tool envelope to be
returned without the former 4,096-token truncation boundary. Advanced callers
can still set a different bounded estimate; normal task and schedule budgets
continue to limit aggregate use.

Normal output shows project, task identity, durable state, execution guidance,
and the latest assistant response when one exists. `run --json` returns the
complete `LaunchTaskResponse`. `run --stream-json` writes newline-delimited JSON:
one `task` record, zero or more authenticated durable `event` records, and one
terminal `result` record. The CLI subscribes before launching, filters live
events by task identity, then reconciles durable history before the final record
so fast operations do not lose progress and reconnect timing cannot duplicate a
sequence.

A completed, objectively verified execution exits successfully. Blocked,
failed, approval-waiting, manually evaluated, or otherwise non-completed work
still prints its full human/JSON result and exits non-zero so shell automation
cannot mistake durable work requiring attention for success.

Piped content is intentionally stored in the durable task envelope and may be
sent to a selected model as governed context. Users should not pipe credentials
or other material they do not want retained. Tokens used to authenticate the
CLI are never included in human, JSON, or event output.

The process-level integration matrix runs the real `mcg` executable against a
real loopback daemon and deterministic OpenAI-compatible provider. Separate
human, JSON, and NDJSON journeys each complete a worker turn plus independent
Evaluator turn, execute the discovered project test command, settle objective
evidence, reuse one canonical project, and exclude the bearer token and piped
owner context from captured output. A complementary no-provider matrix requires
honest blocked state, nonzero exit, complete terminal records, bounded input, and
the same privacy guarantees.

Existing administrative families such as `mcg task`, `mcg run plan-atomic`,
`mcg provider`, `mcg schedule`, and `mcg doctor` remain available. A quoted bare
goal avoids ambiguity when its first word is also an administrative command.

`mcg organization` also exposes the complete portable lifecycle: `history`
lists immutable revisions, `export` writes a bounded content-addressed secret-free
genome, `import` creates fresh local authority, and `compare` reports semantic
definition changes without treating UUID changes as differences. Import accepts
`--import-id`; when omitted, the CLI prints the generated identity before making
the request so the same identity can be reused after an uncertain response.
