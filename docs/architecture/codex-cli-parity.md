# Codex CLI and desktop parity matrix

Last verified 2026-08-29 against:

- pinned Codex app-server `0.150.1` schema
  `844b52d4a5a8cda58794e28b3b119c3a3d20a588b7db83209c298bec62704092`;
- current official [Codex CLI command reference](https://learn.chatgpt.com/docs/developer-commands?surface=cli);
- current official [slash-command reference](https://learn.chatgpt.com/docs/reference/slash-commands).

This is a gap ledger, not a completion claim. `Native` means the interaction
and effect match current Codex. `MetaCog` means the same objective is supported
through a governed MetaCog equivalent but its invocation differs. `Partial`
means a real implementation still has missing behavior. `Missing` means there
is no usable equivalent. MetaCog-only operations remain under `/mcg` or
`mcg <subcommand>`.

## Interactive terminal

| Area | Codex surface | State | Remaining gap |
| --- | --- | --- | --- |
| Terminal | Transcript, multiline composer, palettes, history, `Ctrl+R`, `Ctrl+O`, `Ctrl+L`, steering, queueing, reconnect/replay, `--no-alt-screen` | Native | `Ctrl+L` clears only the rendered transcript; durable conversation history and replay remain intact. Initial image attachments enter the same durable mention queue as interactive picker selections. Inline mode preserves terminal scrollback without entering the alternate screen. |
| References | `@`, `/mention`, `$skill`, `$app` | Native | Files/folders, explicitly attached resources, MetaCog plugins, Codex skills, Apps, and initial image attachments use bounded searchable completion. `$` lists accessible enabled Apps and skills; `/apps` inserts the selected `$app-id` instead of sending a command. |
| Sessions | `/new`, `/clear`, `/resume`, `/rename`, `/fork`, `/side`/`/btw`, `/archive`, `/delete`, `/exit`/`/quit` | Partial | Searchable session selection, exact selectors, saved/current-directory choice, archive confirmation, and exit are native. Side-session presentation still needs comparison. |
| Work | `/review`, `/diff`, `/compact`, `/copy`, `/status`, `/usage`, `/raw` | Partial | Top-level review targets and human findings are native; interactive review target picking, raw terminal scrollback, and exact usage/status presentation remain. |
| Execution | `/model`, `/fast`, `/personality`, `/permissions`, `/approve`, `/plan`, `/goal` | Partial | Cycle 15 adds live project-aware Codex permission-profile discovery and durable selection in the nested `/permissions` picker. Exact remaining Plan/Goal edge cases are pending. Plan is unavailable while running as of Cycle 1. |
| Background work | `!`, `/ps`, `/stop`/`/clean` | Native | Cycle 1 makes stop/clean settle all conversation-owned commands. |
| Context | `/skills`, `/apps`, `/plugins`, `/mcp`, `/memories`, `/agent`, `/subagents` | Partial | `/apps` matches current picker/insertion behavior. `/agent` and `/subagents` now provide a searchable live-thread picker, replace the transcript viewport, and route replies through the selected governed session. Several other context commands still print discovery data, and approval-source thread jumping remains. |
| Preferences | `/keymap`, `/vim`, `/statusline`, `/title`, `/theme`, `/pets`/`/pet`, `/debug-config` | Partial | Keymap, status/title, theme, and pets are limited implementations. |
| Integration | `/ide`, `/init`, `/import`, `/hooks`, `/logout`, `/app`, `/feedback` | Partial | Init/logout and `/app` workspace/conversation handoff work. Direct desktop deep-linking to the active conversation, IDE context, and import remain; hooks/feedback are informational. |
| Windows | `/setup-default-sandbox`, `/sandbox-add-read-dir` | Native | The setup command is shown only when the pinned Codex readiness contract says configuration is required; both mutations are blocked while any conversation has active work. Advanced Settings exposes the same setup, project, permission-profile, and canonical read-directory flow. |
| Experimental | `/experimental` | Missing | Requires discovered capabilities and durable feature configuration. |

Commands without implemented handlers must remain absent from completion.
Environment variables alone are not capability discovery.

## Top-level command compatibility

| Codex family | State | Notes |
| --- | --- | --- |
| `codex` TUI / prompt | Native | `mcg` launches the TUI and accepts a consumer prompt. |
| `exec` / `e` | Partial | Cycles 2-18 add the compatible top-level alias, stdin prompts, global model selection, independent global `-a/--ask-for-approval` and `-s/--sandbox` controls including durable read-only execution, repeatable `--add-dir`, ordered session-local `--enable`/`--disable`, repeatable typed `-c/--config`, named `-p/--profile` layering, global `--search`, native `--strict-config` preflight, initial image references, bounded `--output-schema` for initial and resumed work, output-last-message, ephemeral cleanup, Git checks, and durable exec resume. Cycle 25 matches the pinned Codex 0.150.1 JSONL lifecycle, item, error, and usage envelopes over the replayable conversation stream. Local providers and hook/rules flags remain. |
| `doctor` | MetaCog | MetaCog has a broader diagnostic command with a different report contract. |
| `mcp`, `plugin` | MetaCog | Governed management exists; exact syntax and marketplace operations remain incomplete. |
| `archive`, `unarchive`, `delete` | Native | IDs take precedence over exact names; deletion previews descendants, prompts by default, restricts forced deletion to UUIDs, and uses the existing transactional safety gate. |
| `completion` | Native | Bash, Zsh, Fish, PowerShell (`power-shell`), and Elvish completions include both Codex-compatible and MetaCog-native commands and work without the daemon. |
| `resume`, `fork` | Native | IDs, exact names, project-scoped `--last`, `--all`, durable forks, searchable terminal selection, and terminal reopening work. `resume --include-non-interactive` includes durably classified exec/consumer sessions while explicit IDs and names always remain addressable. Different roots prompt in an interactive terminal unless `tui.resume_cwd` decides; explicit `-C` wins. |
| `review` | Partial | Uncommitted, base branch, commit/title, stdin, and custom-instruction targets use the native read-only review API and render only the final human-readable finding instead of protocol JSON. Remaining global overrides need exact alignment. |
| `login`, `logout` | Partial | Browser OAuth, status, and device-wide logout use the shared Codex app-server account. Device auth, API-key stdin, and access-token stdin remain unavailable from the pinned app-server adapter. |
| `app` | Partial | `mcg app [PATH]` is daemon-free, validates the workspace, prints the canonical Windows handoff path, and launches MetaCog by path on macOS. Direct Windows desktop launch/deep-linking to the active conversation remains. |
| `features` | Native | `list`, `enable`, and `disable` match Codex output and use the pinned app-server's shared configuration. Ordered global `--enable`/`--disable` and `-c features.*=bool` create session-local overrides without mutating that configuration. |
| `agents` | Partial | Lists every active MetaCog Chat, Coordinator, Coder, Tester, and future governed agent session across conversations. `/agent` provides searchable inspection and messaging inside the active conversation; a global cross-conversation live browser remains pending. |
| `queue` | Native | Resolves a conversation UUID or exact name, validates global execution and image-reference options, durably persists the queued owner turn, and returns after daemon acceptance while governed work continues FIFO. |
| `migrate-rollouts` | Native | Daemon-free pass-through to the pinned Codex 0.150.1 migration engine with dry-run default, `--apply`, repeated `--thread`, throughput, JSON, and verbose controls. MetaCog does not reinterpret Codex rollout storage. |
| global `--approve-for-me` | Native | Maps to MetaCog's existing governed Auto Review mode and forces the workspace-write sandbox and on-request policy while organization limits remain authoritative. It is mutually exclusive with explicit approval policy and full bypass. |
| `sandbox` | Native | `mcg sandbox` is a daemon-free pass-through to the pinned Codex 0.150.1 Windows restricted-token sandbox. It preserves explicit state JSON, readable roots, network disabling, named permission/config profiles, managed configuration, working directory, literal command arguments, and repeatable config/feature overrides. The child process exit code and diagnostics pass through unchanged. |
| `apply` / `a` | Native | Resolves the current canonical Git project, runs the pinned `codex apply TASK_ID` in an isolated shadow worktree seeded with the exact tracked, staged, dirty, and bounded-untracked owner baseline, validates and persists the resulting diff, then integrates it only if the live baseline is unchanged. Digest checks, dry-run application, preimage journaling, rollback, restart recovery, and project-scoped cleanup keep the owner workspace governed. The desktop exposes the same contract as a review-first Pull Requests flow with explicit Apply and Discard actions. |
| `update` | Native (packaged Windows) | `mcg update` uses the same immutable HTTPS/minisign authority as Desktop, selects only a strictly newer platform artifact, verifies the exact download, rechecks the feed identity, creates and verifies a fresh credential-free backup, binds the reviewed version to the Windows installer PE resources, stops the daemon, and hands replacement to the authenticated packaged supervisor. Development/unsigned builds fail closed. Linux and macOS CLIs direct installation through the existing Desktop updater until a complete packaged self-replacement contract is qualified there. |
| `mcp-server` | Superseded | Current Codex documentation marks this family deprecated and directs integrations to app-server. MetaCog already uses its pinned app-server contract and will not add a new deprecated ungoverned endpoint. |
| `app-server`, `cloud`, `debug`, `execpolicy`, `remote-control` | Missing | Experimental families remain gated until their contracts can be safely supported. |

## Global flag compatibility

MetaCog supports daemon host/token path and project path globally. Cycle 4 moves
model, approval policy, sandbox, `--yolo`, and repeatable/comma-separated initial
image attachments to the shared global parser. They now work before or after
`exec`, on the interactive root and bare/piped prompts, on resumed/forked TUI
sessions, and on the MetaCog consumer `run` path. The safer interactive and bare
prompt default is Ask; exact approval and sandbox selections remain independent
and persist in governed execution preferences. Ephemeral cleanup, JSONL,
output-last-message, and Git-check flags remain `exec`-specific.

Cycle 6 adds repeatable `--add-dir` before or after `exec`, on interactive and
bare/piped prompts, on resumed/forked sessions, and on the MetaCog consumer
`run` path. MetaCog canonicalizes and de-duplicates at most sixteen existing
directories, requires a linked project, persists them with the conversation and
turn, revalidates them before each originating turn, and forwards them only as
Codex `workspaceWrite.writableRoots`. Read-only execution rejects the setting;
normal approval and organization policy remain authoritative. The desktop offers
the same control in the existing Advanced run-settings disclosure.

Cycle 7 adds native `features list|enable|disable` behavior through the pinned
Codex app-server's typed feature discovery and mutation methods. This is a
process-wide Codex configuration surface, not a MetaCog authority override. The
same searchable flag list is available in Advanced Settings while Simple stays
uncluttered.

Cycle 8 adds repeatable global `--enable FEATURE` and `--disable FEATURE` before
or after compatible commands and on interactive, bare/piped, consumer-run, and
resumed/forked work. Raw argument order is preserved and the last occurrence
wins, matching Codex's `features.<name>` override behavior. Overrides are
persisted with the originating run but do not mutate shared Codex configuration.
Names are bounded and validated. MetaCog rejects flags controlling its shell,
agent, browser/computer, app/plugin, image-generation, or workspace-dependency
authority, and role-scoped configuration is recursively merged last.

Cycle 9 adds repeatable global `-c KEY=VALUE` and `--config KEY=VALUE` with
Codex-compatible dotted TOML keys, typed TOML values, and literal-string
fallback. Overrides work before or after compatible commands and persist with
interactive, bare/piped, consumer-run, resumed, and forked work. Raw order is
retained so later duplicate paths win. `features.*` values share the ordered
feature-override path. Runtime limits each run to 32 overrides and 16 KiB, then
rejects approval, sandbox, filesystem, network, agent, app/plugin, tool,
instruction, provider, credential, secret-bearing, and hidden-reasoning paths.
Role-scoped configuration remains authoritative and merges last.

Cycle 10 adds stable global `--search` across interactive, bare/piped,
consumer-run, exec, resume, fork, queue, and retry paths. Each occurrence is
recorded in raw argument order as `web_search = "live"`, so later `-c` or
`--search` occurrences win exactly as they do in Codex. Runtime validation also
accepts explicit `cached`, `live`, and `disabled` values while rejecting other
types or strings. Role-scoped configuration remains final.

Cycle 11 adds global `--no-alt-screen` to interactive, resumed, and forked
terminal sessions. It selects Ratatui's inline viewport, never enters the
terminal alternate screen, and restores raw mode, mouse capture, and cursor
state on exit while leaving prior terminal scrollback available.

Cycle 12 upgrades the pinned binary and complete generated app-server snapshot
from 0.144.6 to 0.150.1. Native qualification validates the real binary,
per-thread configuration boundary, and stable paginated `app/list` contract.
The CLI now exposes `/apps` only when discovery confirms support, inserts the
selected `$app-id`, and merges enabled accessible Apps into `$` completion with
skills. Pagination is bounded at 10,000 entries and rejects repeated cursors.

Cycle 14 adds the stable top-level `sandbox` family as an exact daemon-free
owner utility over the pinned Codex executable. Unlike model-generated shell
work, this command contains only the owner's literal argv and does not create or
expand Chat, Coordinator, Coder, Tester, project, approval, or organization
authority. MetaCog preserves the installed Codex permission/config profile and
managed-requirements behavior instead of reimplementing or weakening the
restricted-token sandbox. Direct-process parity is covered by complete argument
projection tests and by comparing the wrapper's real exit code and diagnostic
with the pinned Codex command on the same host.

Cycle 15 adds stable `permissionProfile/list` discovery to the pinned adapter
and the interactive `/permissions` picker. A selected profile is persisted with
the conversation and originating turn, shown in the terminal status line, and
re-resolved against the current project-aware Codex configuration immediately
before dispatch. Disallowed, missing, malformed, project-free, or explicitly
sandboxed combinations fail closed. The adapter supplies the profile through
the mutually exclusive `permissions` field on thread start, thread resume, and
turn start, while project-free Chat remains read-only and MetaCog's role,
organization, lease, integration, and approval boundaries stay authoritative.

Cycle 16 adds global `--strict-config` to interactive, bare/piped, consumer-run,
`exec`, resume, fork, and queue execution. The daemon asks the pinned Codex
binary to load the effective user and project configuration with its native
strict loader before persisting the owner message or turn. Unknown fields and
other strict-loader failures therefore fail before dispatch without creating an
orphaned pending turn. The bounded qualification process closes stdin, has a
30-second timeout, is killed on drop, and never disturbs the long-lived Codex
app-server. The selected preference is durable and auditable per originating
turn.

Cycle 17 adds global `-p NAME` and `--profile NAME` to interactive, bare/piped,
consumer-run, `exec`, resume, fork, queue, and Windows sandbox flows. The pinned
Codex loader first validates `$CODEX_HOME/<name>.config.toml`; MetaCog then stores
the selected name, source SHA-256, removed-key audit list, and a bounded safe
snapshot with the originating run. Credential-, approval-, sandbox-, network-,
tool-, provider-, instruction-, MCP-, plugin-, app-, and agent-authority settings
are stripped. The safe profile snapshot merges first, explicit ordered `-c` and
feature overrides merge next, and immutable role policy merges last. This keeps
resumed and delegated execution deterministic without allowing a profile to
expand MetaCog governance. The installed Codex 0.150.1 qualification proves the
documented named-file loader shape.

Cycle 18 adds Codex-compatible `--output-schema <FILE>` to initial `exec` and
`exec resume`. MetaCog reads one bounded JSON-object schema, validates it again
at the runtime boundary, persists it with the originating run, and forwards it
through native app-server `turn/start.outputSchema`. Internal routing remains
unconstrained so it can still delegate, but the final owner answer is constrained;
schema failure is terminal and never replaced with nonconforming fallback text.
Plan and active Goal modes reject structured output because their interactive
continuation contracts are incompatible with a single final JSON document.

Cycle 19 adds a durable interactive/non-interactive conversation classification,
uses it for Codex-compatible `resume --include-non-interactive`, and replaces the
blocking numbered resume/fork prompt with a searchable resize-safe Ratatui picker.
Direct session IDs and exact names remain addressable regardless of picker filtering.
Native `review` now extracts the latest completed `agentMessage` and prints the
human finding while retaining visible protocol failures; raw review turn JSON is
no longer presented as the ordinary result.

Cycle 20 adds the Codex-style active-conversation agent browser behind `/agent`
and `/subagents`. Active sessions sort before completed sessions, fuzzy filtering
and bounded scrolling work at compact terminal sizes, selection replaces the
main transcript with that agent's messages and activity, direct input is routed
through the existing role-scoped agent-session API, and Escape returns to the
owner-facing conversation. Messages to a working specialist steer only its
pinned active Codex turn; missing bindings fail before an agent-room message is
persisted, and ready/waiting sessions retain their governed continuation path.
MetaCog's existing desktop agent widgets already expose the same Chat,
Coordinator, Coder, Tester, message, and activity records, while Tester browser
authority remains independently bounded and non-mutating, so no desktop visual
change is required for this cycle. Cross-conversation agent browsing and jumping
from an approval overlay to its source thread remain explicit gaps.

Cycle 21 aligns top-level `resume` and `fork` working-directory behavior with
Codex. The CLI compares the current canonical root with the selected session's
saved project, uses a resize-safe current-versus-session picker when they differ,
and honors the native layered `tui.resume_cwd = "current" | "session"` value.
Ordered explicit `-c tui.resume_cwd=...` overrides the selected named profile
and effective user/project configuration, while explicit `-C/--cd` wins over
every preference. Choosing current performs an explicit governed project rebind
before the TUI opens; choosing session preserves the saved authority boundary.
Project-free conversations remain unbound unless `-C` explicitly targets a
project. Invalid values and non-interactive ambiguous choices fail visibly.

Cycle 22 adds native top-level `apply` and alias `a` without allowing Codex to
mutate the owner workspace directly. MetaCog creates a project-scoped shadow
worktree, overlays the exact tracked, staged, dirty, and bounded-untracked
baseline, invokes the pinned Codex cloud-apply command there, and stores a
bounded binary patch plus normalized path and digest evidence. Integration
revalidates HEAD and every baseline entry, dry-runs the patch, journals
preimages, verifies post-apply content identities, and rolls back on failure.
Interrupted preparation or integration is reconciled on daemon restart, and
discard removes both the isolated worktree and sensitive durable patch content.
The desktop Pull Requests surface lists recoverable candidates and keeps cloud
changes reviewable until an explicit Apply action; its light- and dark-theme
controls share the application tokens and meet the same accessibility gate.

Cycle 23 adds stable top-level `update` for complete packaged Windows releases.
The signed-feed parser and trust validation now live in `mcg-updater`, shared by
the CLI and Desktop adapter. The CLI accepts no runtime, project, configuration,
or feature overrides; it checks the immutable owner-controlled HTTPS feed,
selects only a strictly newer target, bounds the manifest and artifact, validates
canonical minisign envelopes, verifies the exact downloaded bytes, and rechecks
the candidate before mutation. Installation requires a fresh verified
credential-free database backup and an exact installed `mcg.exe`,
`mcg-desktop.exe`, and `mcg-updater.exe` topology. The Windows supervisor request
now pins either the packaged Desktop or packaged CLI requester identity, waits
for that exact process to exit, binds the signed artifact to its embedded PE
`ProductVersion`, supervises the installer, records failure evidence, recovers
the previous installation, and removes only its private staged files. Unsigned
development builds and incomplete installations fail closed. The Desktop update
layout and workflow are unchanged; they consume the same shared trust contract.

Codex-compatible support remains incomplete for local providers/OSS, remote
endpoints/authentication, exact event schemas, and hook trust. Current Codex 0.150.1
documents `--permission-profile` only for the
Windows `sandbox` command; that exact contract is already Native rather than a
missing global non-sandbox flag. These
flags must map into governed requests; they may not bypass organization or
approval policy.

## Desktop alignment rule

Every capability affecting ordinary conversation work must be reachable in the
desktop through the shared API and current visual system. Terminal-only mechanics
such as shell completion and keymaps do not need GUI duplicates. Authentication,
permissions, models, profiles, apps, MCP, plugins, memory, sandbox roots,
cloud/remote sessions, reviews, session management, and updates do. New desktop
interactions require automated accessibility coverage and native UAT.

Cycle 3 adds the shared Read only permission choice to New Chat and established
Chat. It is enforced through Coordinator/Coder delegation, report acceptance,
and integration, rather than treated as a presentation-only preference.

Cycle 6 adds Additional writable folders to the same Advanced run-settings
disclosure in New Chat, established Chat, and task execution. Project-free and
read-only runs cannot add them; selected folders remain removable and are sent
through the shared execution-preference contract.

Cycle 7 adds a searchable, independently scrollable Codex feature-flag disclosure
under Advanced Settings → Configuration. It shows the pinned runtime's stage,
effective state, and description, and writes through the same typed app-server
contract as `mcg features`; no flag can expand MetaCog organization, approval,
or delegated-agent authority.

Cycle 8 adds Session features to the existing Advanced run-settings hierarchy in
New Chat, established Chat, and task execution. It uses the discovered pinned
catalog, offers Default/On/Off per safe flag, clearly distinguishes per-run
overrides from persistent Settings, excludes MetaCog-governed capabilities, and
retains the same bounded responsive popover and independent scrolling behavior.

Cycle 9 adds Codex config beside Session features in the same Advanced
run-settings hierarchy. It accepts a validated dotted key plus a typed JSON value
or literal string, displays ordered removable rows, persists them across drafts,
queues, retries, and resumed conversations, and exposes the same governance
boundary as the CLI. Simple mode remains unchanged.

Cycle 10 adds a dedicated Web search panel to the same Advanced run-settings
hierarchy. Default uses cached search, Live maps to Codex's global `--search`
semantics, and Off remains available as the validated explicit configuration.
The setting persists with the originating run and remains absent from Simple.

Cycle 12 adds Apps to the shared New Chat and established Chat composer. The
Add menu, `/apps`, and direct `$` typing open the same searchable, independently
scrollable picker; selection inserts the stable `$app-id` syntax used by Codex.
Apps stay visually and semantically distinct from MetaCog's pinned-revision
plugin mentions. Outside interaction and Escape dismiss the picker consistently.

Cycle 13 adds Codex 0.150.1's global `--approve-for-me` shortcut to interactive,
bare/piped, consumer-run, exec, resume, fork, and queue paths. It selects the
same Auto Review option already available in both desktop composers, so no new
GUI control is needed. The new top-level `queue` endpoint accepts and persists a
turn before returning while the daemon retains responsibility for execution.
`migrate-rollouts` delegates unchanged to the pinned Codex migration engine.
The first `agents` surface exposes the complete governed session catalog, but
remains Partial until its searchable live terminal browser is implemented.

Cycle 15 adds project-aware Codex permission profiles to the existing Advanced
permission menu in New Chat and established Chat. Only profiles that the current
Codex configuration marks allowed are shown; Simple retains the four frequent
human-readable modes and still displays an already selected profile without
hiding the active execution choice. Selecting a standard mode clears the named
profile. The shared menu is independently scrollable, bounded, accessible, and
uses the existing MetaCog permission visual language.

Cycle 16 adds Strict config to the existing Advanced Codex config panel in both
New Chat and established Chat. Simple remains unchanged. The control uses the
shared run-preference model, persists with ordinary drafts and conversations,
and summarizes its active state in the existing selector without adding a new
page or exposing raw diagnostics by default.

Cycle 17 adds the named Codex profile beside Strict config in that same Advanced
panel. The compact summary shows the selected profile, strict state, and override
count; the responsive form explains the exact named-file location and that
MetaCog ignores authority- and secret-bearing keys. New Chat and established
Chat share the control and persistence path, while Simple remains unchanged.

Cycle 18 adds Output format to the same Advanced shared run-settings hierarchy.
It accepts a bounded JSON-object schema with inline validation, preserves it with
New Chat and established Chat drafts and turns, and summarizes the selection as
Structured JSON. Simple remains text-first and does not expose this expert-only
control. The panel is responsive, independently scrollable where needed, and
uses the existing MetaCog disclosure, field, error, and action styling.

Cycle 24 adds current Codex 0.150.1 Windows sandbox setup and additional
read-directory behavior. `/setup-default-sandbox` subscribes before invoking
`windowsSandbox/setupStart`, waits for the matching completion notification,
persists the exact `windows.sandbox` mode while clearing all three legacy
feature keys, and requires both an effective configuration write and `ready`
readiness. `/sandbox-add-read-dir` validates an existing canonical absolute
directory and asks the pinned `codex sandbox` engine to refresh the selected
project permission profile with that read root. Both operations are rejected
while any conversation has nonterminal work. Advanced Settings mirrors the
same governed contract without exposing it in Simple mode.

Cycle 25 replaces MetaCog's approximate `exec --json` receipts with the exact
pinned Codex 0.150.1 JSONL projection: `thread.started`, `turn.started`,
`item.started`, `item.updated`, `item.completed`, `error`, `turn.completed`, and
`turn.failed`. Command executions, file changes, MCP and collaboration calls,
web searches, agent messages, reasoning summaries, todo lists, nonfatal errors,
and token usage use the pinned item shapes, including cache-write input tokens.
The daemon's replayable conversation stream now carries the exact safe activity
or message delta alongside the backward-compatible full view, preventing short
lifecycle states from being lost when a fast turn completes before a client
connects. Human output and MetaCog's separate consumer JSON receipts are
unchanged.

## Safety invariants

- Mentions provide bounded context or a pinned plugin capability; they never
  expand filesystem, network, agent, plugin, or approval authority.
- Agent-session replies retain the target session's pinned delegation.
- Conversation deletion refuses active work, requires exact confirmation,
  deletes descendants transactionally, and retains only required tombstones.
- `!command` uses the daemon-side governed executor, never the unrestricted CLI.
- Only model-provided reasoning summaries are shown.

## Cycle gate

Each cycle updates this ledger, adds automated coverage for every behavior,
runs affected Rust and desktop suites, performs native GUI UAT for desktop
changes, removes obsolete portable outputs, builds one current Windows portable,
and pushes the verified commit. Re-audit when the pinned binary, generated schema,
or official reference changes.
