# Codex CLI and desktop parity matrix

Last verified 2026-08-28 against:

- pinned Codex app-server `0.144.6` schema
  `e710974af2ef773a512e039f7b38b6040f533035465ad899929f4cc52ce7a2a4`;
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
| Terminal | Transcript, multiline composer, palettes, history, `Ctrl+R`, `Ctrl+O`, `Ctrl+L`, steering, queueing, reconnect/replay | Native | `Ctrl+L` clears only the rendered transcript; durable conversation history and replay remain intact. Initial image attachments enter the same durable mention queue as interactive picker selections. |
| References | `@`, `/mention`, `$skill`, `$app` | Partial | Files/folders, plugins, skills, and initial image attachments work; app completion is missing. |
| Sessions | `/new`, `/clear`, `/resume`, `/rename`, `/fork`, `/side`/`/btw`, `/archive`, `/delete`, `/exit`/`/quit` | Partial | Cycle 1 adds clear naming and archive confirmation/exit; selectors and side-session presentation need comparison. |
| Work | `/review`, `/diff`, `/compact`, `/copy`, `/status`, `/usage`, `/raw` | Partial | Review needs base/commit selection; raw must expose terminal scrollback; usage/status need exact presentation. |
| Execution | `/model`, `/fast`, `/personality`, `/permissions`, `/approve`, `/plan`, `/goal` | Partial | Cycle 3 adds a durable read-only preset shared with desktop. Named permission profiles, exact nested selectors, and remaining Plan/Goal edge cases are pending. Plan is unavailable while running as of Cycle 1. |
| Background work | `!`, `/ps`, `/stop`/`/clean` | Native | Cycle 1 makes stop/clean settle all conversation-owned commands. |
| Context | `/skills`, `/apps`, `/plugins`, `/mcp`, `/memories`, `/agent`, `/subagents` | Partial | Apps are missing; several commands print discovery data; agent selection does not replace the transcript viewport. |
| Preferences | `/keymap`, `/vim`, `/statusline`, `/title`, `/theme`, `/pets`/`/pet`, `/debug-config` | Partial | Keymap, status/title, theme, and pets are limited implementations. |
| Integration | `/ide`, `/init`, `/import`, `/hooks`, `/logout`, `/app`, `/feedback` | Partial | Init/logout and `/app` workspace/conversation handoff work. Direct desktop deep-linking to the active conversation, IDE context, and import remain; hooks/feedback are informational. |
| Windows | `/setup-default-sandbox`, `/sandbox-add-read-dir` | Missing | Implement a governed sandbox mutation contract and mirror it in Settings. |
| Experimental | `/experimental` | Missing | Requires discovered capabilities and durable feature configuration. |

Commands without implemented handlers must remain absent from completion.
Environment variables alone are not capability discovery.

## Top-level command compatibility

| Codex family | State | Notes |
| --- | --- | --- |
| `codex` TUI / prompt | Native | `mcg` launches the TUI and accepts a consumer prompt. |
| `exec` / `e` | Partial | Cycles 2-4 add the compatible top-level alias, JSONL lifecycle output, stdin prompts, global model selection, independent global `-a/--ask-for-approval` and `-s/--sandbox` controls including durable read-only execution, initial image references, output-last-message, ephemeral cleanup, Git checks, and durable exec resume. Exact event schemas, output schemas, profiles/config overlays, local providers, and hook/rules flags remain. |
| `doctor` | MetaCog | MetaCog has a broader diagnostic command with a different report contract. |
| `mcp`, `plugin` | MetaCog | Governed management exists; exact syntax and marketplace operations remain incomplete. |
| `archive`, `unarchive`, `delete` | Native | IDs take precedence over exact names; deletion previews descendants, prompts by default, restricts forced deletion to UUIDs, and uses the existing transactional safety gate. |
| `completion` | Native | Bash, Zsh, Fish, PowerShell (`power-shell`), and Elvish completions include both Codex-compatible and MetaCog-native commands and work without the daemon. |
| `resume`, `fork` | Partial | IDs, exact names, project-scoped `--last`, `--all`, durable forks, and terminal reopening work. The temporary numbered picker must be replaced by the same searchable TUI overlay, and non-interactive session classification/current-vs-saved-directory choice remain. |
| `review` | Partial | Uncommitted, base branch, commit/title, stdin, and custom-instruction targets use the native read-only review API. Human-formatted findings and remaining global overrides need exact alignment. |
| `login`, `logout` | Partial | Browser OAuth, status, and device-wide logout use the shared Codex app-server account. Device auth, API-key stdin, and access-token stdin remain unavailable from the pinned app-server adapter. |
| `app` | Partial | `mcg app [PATH]` is daemon-free, validates the workspace, prints the canonical Windows handoff path, and launches MetaCog by path on macOS. Direct Windows desktop launch/deep-linking to the active conversation remains. |
| `apply`, `features`, `sandbox`, `update` | Missing | Add compatible top-level behavior without removing MetaCog commands. |
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

Codex-compatible support remains for `--add-dir`, live search, local
providers/OSS, profiles, permission profiles, remote endpoints/authentication,
strict config, output schemas, `--no-alt-screen`, feature toggles, hook trust,
and repeatable `-c key=value` overrides. These flags must map into governed
requests; they may not bypass organization or approval policy.

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
