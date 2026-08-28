# Codex CLI parity matrix

Last verified 2026-08-27 against the pinned Codex app-server `0.144.6` schema
(`e710974af2ef773a512e039f7b38b6040f533035465ad899929f4cc52ce7a2a4`).

MetaCog follows Codex interaction semantics where the pinned stable protocol can
support them without weakening MetaCog governance. Unsupported or experimental
commands are omitted from completion instead of being displayed as inert menu
items. MetaCog-only operations remain under `/mcg` or `mcg <subcommand>`.

| Area | Commands and mechanics | MetaCog state |
| --- | --- | --- |
| Terminal | Persistent transcript, multiline composer, resize-safe palettes, history, search, copy, steering, queueing, interrupt, reconnect/replay | Native |
| Sessions | `/new`, `/clear`, `/resume`, `/rename`, `/fork`, `/side`/`/btw`, `/archive`, `/delete`, `/exit`/`/quit` | Native; delete is exact-confirmed and recursive |
| Work | `/review`, `/diff`, `/compact`, `/copy`, `/status`, `/usage`, `/raw` | Native |
| Execution | `/model`, `/fast`, `/personality`, `/permissions`, `/approve`, `/plan`, `/goal` | Native and capability-filtered |
| Context | `@`, `/mention`, `$`, `/skills`, `/plugins`, `/mcp`, `/memories`, `/agent`, `/subagents` | Native; `$` exposes stable enabled skills and apps only when stable discovery exists |
| Preferences | `/keymap`, `/vim`, `/statusline`, `/title`, `/theme`, `/pets`/`/pet`, `/debug-config` | Native and persisted outside organization policy |
| Integration | `/init`, `/hooks`, `/logout`, `/feedback` | Native |
| Governed shell | `!command`, `!command &` | Native foreground/background execution through the daemon's pinned Codex sandbox contract |
| Background process control | `/ps`, `/stop` | Native; client-owned process IDs are scoped to the active conversation |
| Desktop handoff | `/app` | Hidden until a stable authenticated desktop-navigation contract exists |
| Apps | `/apps`, `$app` | Hidden while `app/list` remains experimental in the pinned schema |
| IDE/import/experiments | `/ide`, `/import`, `/experimental` | Hidden unless capability discovery explicitly enables them |
| Windows sandbox setup | `/setup-default-sandbox`, `/sandbox-add-read-dir` | Hidden until the stable protocol exposes the required mutation contract |

## Safety invariants

- A mention supplies bounded context or a pinned plugin capability; it never
  expands filesystem, network, agent, plugin, or approval authority.
- Agent-session replies use the target session's existing Codex thread and
  pinned delegation. They cannot add roles, tools, grants, or organization
  capacity.
- Conversation deletion refuses active turns and active goals, requires the
  exact conversation ID, deletes descendants transactionally, retains only
  content-free policy tombstones, and garbage-collects unreferenced captured
  mention artifacts.
- `!command` is never executed by the unrestricted CLI process. It uses the
  daemon-side Codex command executor with the selected project, permissions,
  sandbox, timeout, and bounded output.
- Reasoning display contains only model-provided summaries. Hidden reasoning is
  never requested or reconstructed.

## Drift gate

When the pinned Codex binary or generated schema digest changes, regenerate the
protocol bindings, review newly stable request methods, update this matrix, and
rerun CLI registry, PTY, runtime, API, desktop, and packaging gates before
exposing additional commands.
