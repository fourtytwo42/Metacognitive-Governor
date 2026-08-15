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

With a terminal and no arguments, `mcg` opens a small prompt; `:quit` and
`:exit` close it. Each entered goal becomes a separately durable task. Without
a terminal, argument-free standard input becomes the goal. When a goal is also
present, piped UTF-8 is attached as owner-supplied task context. Standard input
is rejected above 1 MiB and goals are rejected above 32,768 characters.

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

Existing administrative families such as `mcg task`, `mcg run plan-atomic`,
`mcg provider`, `mcg schedule`, and `mcg doctor` remain available. A quoted bare
goal avoids ambiguity when its first word is also an administrative command.
