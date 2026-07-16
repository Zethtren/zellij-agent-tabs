# Agent State Protocol

`zellij-agent-tabs` is **agent-agnostic**. The plugin only consumes state messages
over Zellij's `pipe` mechanism and renders them. Anything that can run a shell
command — a Claude Code hook, an Aider callback, a shell `postexec`, a cron job —
can be an *adapter* by emitting one line.

## The message

```
zellij pipe --name agent_state -- "<pane_id>\x1f<state>\x1f<agent>\x1f<label>"
```

- Fields are separated by the ASCII **Unit Separator** (`\x1f`, decimal 31) so that
  `<label>` may contain spaces, `:`, `/`, etc. without ambiguity.
- Emit with `printf`, e.g.:
  ```sh
  printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" working claude "$PWD: npm test" \
    | xargs -0 -I{} zellij pipe --name agent_state -- {}
  ```
  (or simpler — see adapter snippets below.)

### Fields

| Field      | Required | Meaning |
|------------|----------|---------|
| `pane_id`  | yes      | The Zellij pane the state belongs to. Adapters read `$ZELLIJ_PANE_ID` (Zellij exports it into every pane's env). The plugin matches it against `PaneInfo.id`. |
| `state`    | yes      | One of the state keywords below. |
| `agent`    | no       | Free tag identifying the source (`claude`, `aider`, `codex`, `shell`, …). Used for optional per-agent glyph/label. Empty is fine. |
| `label`    | no       | Display text for the tab, e.g. `~/proj: cargo build`. Empty ⇒ plugin falls back to pane title. |

### States

The colour and animation for each state are **defaults** — every one is
overridable in plugin config (see below). The protocol only defines the *state
keywords*; how they look is presentation.

| Keyword   | Default colour | Default animation | Meaning |
|-----------|----------------|-------------------|---------|
| `working` | green          | scroll            | Agent/command actively running. |
| `waiting` | orange         | flash             | Blocked on human input / permission. |
| `done`    | green          | solid (no anim)   | Finished successfully. |
| `error`   | red            | flash             | Failed / errored. |
| `idle`    | default        | none              | Cleared / no active agent (default). |

State is **edge-driven**: an adapter sends a new message whenever the state
changes. The plugin holds the last state per pane until the next message or until
the pane closes.

### Multiple panes per tab — aggregation

A tab can contain several panes, each reporting its own state (e.g. Claude in one
split, a dev server in another). The tab shows a **single** aggregated state: the
**least-complete / most-attention-needed** one among its panes.

Default priority (first match wins):

```
error  >  waiting  >  working  >  done  >  idle
```

So a tab with one errored pane flashes red even if its siblings are done; a tab
with a waiting pane and a working pane flashes orange. Per-pane states are kept
individually and re-aggregated on every update, so when the errored/waiting pane
clears, the tab drops to the next-highest state.

The priority order is **configurable** (`state_priority`) if you want, say,
`waiting` to outrank `error`.

## Configuration (plugin block in the layout KDL)

All presentation is set in the `plugin { … }` block — nothing about colours or
animation is hard-coded. Keys (all optional; defaults shown):

```kdl
plugin location="…/zellij-agent-tabs.wasm" {
    // per-state colour (any format the color parser accepts: name / 256 / #hex / rgb())
    color_working "green"
    color_waiting "orange"
    color_done    "green"
    color_error   "red"
    color_idle    "default"

    // per-state animation: "scroll" | "flash" | "solid" | "none"
    anim_working "scroll"
    anim_waiting "flash"
    anim_done    "solid"
    anim_error   "flash"

    anim_interval_ms 500   // timer tick driving flash/scroll
    state_priority "error waiting working done idle"
}
```

## Who produces state

**1. Native (no adapter, any shell).** The plugin reads Zellij's `PaneInfo` directly:
- **Command panes** (things launched as a command — `zellij run -- cargo test`, a
  layout `command`, an editor pane): running → `working`, exited/held → `done` or
  `error` by exit status. This needs **no adapter and no shell integration**.

Note the hard limit this *doesn't* cover: a command **typed at an interactive shell
prompt** is invisible to Zellij — it never sees the command or its exit code (the
shell consumes `$?`). This is inherent to terminal multiplexers (tmux is the same).
Covering that case is the *only* reason a shell adapter exists.

**2. Adapters (shipped in `adapters/`).**
- **Claude Code** (`adapters/claude/`): `settings.json` hooks mapping
  `PreToolUse`→working, `Stop`→done, `Notification`→waiting, tool-failure→error.
  Shell-agnostic.
- **Shell** (`adapters/shell/`, **optional**): `agent-state.{bash,zsh,fish}` — only for
  commands typed at an interactive prompt. Reports `working` on run, `done`/`error`
  from the exit code. Source the one matching your shell from its rc file.
- Additional agents (Aider, Codex CLI, Gemini CLI, Goose, opencode, …) land as their
  own adapter dirs **where the agent exposes a usable lifecycle hook**. Agents without
  hooks are documented as unsupported until upstream adds events.

### Precedence & pane identity

- Adapter-reported state (via the pipe) **overrides** native `PaneInfo` detection for
  the same pane — the adapter knows more than Zellij does.
- Panes are keyed by `$ZELLIJ_PANE_ID`, which Zellij sets on the pane's PTY and which
  is **inherited by every child process**. So a nested shell (`bash` launched from
  `fish`) reports to the *same* pane. Each shell only reports while it is the one
  reading input; load the adapter in each shell's rc for per-command reporting inside
  nested shells (otherwise the nested shell shows a single `working` for its session).

## Why a plugin, not zjstatus

zjstatus is a single-line horizontal bar with no animation and no external state
channel. Vertical placement, multi-line rounded tabs, timer-driven animation, and a
`pipe`-fed state model all require a custom plugin — which is what this is (forked
from the excellent [cfal/zellij-vertical-tabs](https://github.com/cfal/zellij-vertical-tabs),
MIT, which already provides vertical tabs + pipe-fed activity rows as the foundation).
