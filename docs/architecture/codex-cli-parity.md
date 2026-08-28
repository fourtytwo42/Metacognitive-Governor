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
| Terminal | Transcript, multiline composer, palettes, history, `Ctrl+R`, `Ctrl+O`, steering, queueing, reconnect/replay | Native | Add native images and `Ctrl+L` display clearing. |
| References | `@`, `/mention`, `$skill`, `$app` | Partial | Files/folders, plugins, and skills work; app completion and images are missing. |
| Sessions | `/new`, `/clear`, `/resume`, `/rename`, `/fork`, `/side`/`/btw`, `/archive`, `/delete`, `/exit`/`/quit` | Partial | Cycle 1 adds clear naming and archive confirmation/exit; selectors and side-session presentation need comparison. |
| Work | `/review`, `/diff`, `/compact`, `/copy`, `/status`, `/usage`, `/raw` | Partial | Review needs base/commit selection; raw must expose terminal scrollback; usage/status need exact presentation. |
| Execution | `/model`, `/fast`, `/personality`, `/permissions`, `/approve`, `/plan`, `/goal` | Partial | Named permission profiles, exact nested selectors, and remaining Plan/Goal edge cases are pending. Plan is unavailable while running as of Cycle 1. |
| Background work | `!`, `/ps`, `/stop`/`/clean` | Native | Cycle 1 makes stop/clean settle all conversation-owned commands. |
| Context | `/skills`, `/apps`, `/plugins`, `/mcp`, `/memories`, `/agent`, `/subagents` | Partial | Apps are missing; several commands print discovery data; agent selection does not replace the transcript viewport. |
| Preferences | `/keymap`, `/vim`, `/statusline`, `/title`, `/theme`, `/pets`/`/pet`, `/debug-config` | Partial | Keymap, status/title, theme, and pets are limited implementations. |
| Integration | `/ide`, `/init`, `/import`, `/hooks`, `/logout`, `/app`, `/feedback` | Partial | Init/logout work; IDE, import, app handoff are missing; hooks/feedback are informational. |
| Windows | `/setup-default-sandbox`, `/sandbox-add-read-dir` | Missing | Implement a governed sandbox mutation contract and mirror it in Settings. |
| Experimental | `/experimental` | Missing | Requires discovered capabilities and durable feature configuration. |

Commands without implemented handlers must remain absent from completion.
Environment variables alone are not capability discovery.

## Top-level command compatibility

| Codex family | State | Notes |
| --- | --- | --- |
| `codex` TUI / prompt | Native | `mcg` launches the TUI and accepts a consumer prompt. |
| `exec` / `e` | Partial | Cycle 2 adds the compatible top-level alias, JSONL lifecycle output, stdin prompts, model/sandbox selection, image references, output-last-message, ephemeral cleanup, Git checks, and durable exec resume. Exact event schemas, output schemas, profiles/config overlays, local providers, hook/rules flags, and persisted read-only selection remain. |
| `doctor` | MetaCog | MetaCog has a broader diagnostic command with a different report contract. |
| `mcp`, `plugin` | MetaCog | Governed management exists; exact syntax and marketplace operations remain incomplete. |
| `archive`, `unarchive`, `delete` | Native | IDs take precedence over exact names; deletion previews descendants, prompts by default, restricts forced deletion to UUIDs, and uses the existing transactional safety gate. |
| `completion` | Native | Bash, Zsh, Fish, PowerShell (`power-shell`), and Elvish completions include both Codex-compatible and MetaCog-native commands and work without the daemon. |
| `resume`, `fork` | Partial | IDs, exact names, project-scoped `--last`, `--all`, durable forks, and terminal reopening work. The temporary numbered picker must be replaced by the same searchable TUI overlay, and non-interactive session classification/current-vs-saved-directory choice remain. |
| `review` | Partial | Uncommitted, base branch, commit/title, stdin, and custom-instruction targets use the native read-only review API. Human-formatted findings and remaining global overrides need exact alignment. |
| `login`, `logout` | Partial | Browser OAuth, status, and device-wide logout use the shared Codex app-server account. Device auth, API-key stdin, and access-token stdin remain unavailable from the pinned app-server adapter. |
| `app`, `apply`, `features`, `sandbox`, `update` | Missing | Add compatible top-level behavior without removing MetaCog commands. |
| `app-server`, `cloud`, `debug`, `execpolicy`, `remote-control` | Missing | Experimental families remain gated until their contracts can be safely supported. |

## Global flag compatibility

MetaCog supports daemon host/token path and project path globally. Cycle 2 adds
`exec` model, workspace-write/danger-full-access sandbox mapping, image
references, ephemeral cleanup, JSONL, output-last-message, and Git-check flags.
Codex-compatible support remains for `--add-dir`, the complete approval policy,
persisted read-only sandboxing, search, local providers/OSS, profiles, permission
profiles, remote endpoints/authentication, strict config, output schemas, and
repeatable `-c key=value` overrides. These flags must map into governed requests;
they may not bypass organization or approval policy.

## Desktop alignment rule

Every capability affecting ordinary conversation work must be reachable in the
desktop through the shared API and current visual system. Terminal-only mechanics
such as shell completion and keymaps do not need GUI duplicates. Authentication,
permissions, models, profiles, apps, MCP, plugins, memory, sandbox roots,
cloud/remote sessions, reviews, session management, and updates do. New desktop
interactions require automated accessibility coverage and native UAT.

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
