# zellij-agent-tabs

Agent-aware **vertical** tab bar for [Zellij](https://zellij.dev). Tabs render as
rounded, multi-line boxes down the left (or right) edge where the **border shows
which tab you're in** and the **fill shows what that tab is doing** — an AI agent
or a shell command: green while working, orange when it needs you, red on error.

Built to answer one question at a glance: *which of my tabs needs me right now?*

> Fork of the excellent [cfal/zellij-vertical-tabs](https://github.com/cfal/zellij-vertical-tabs)
> (MIT). This fork adds the agent/command state model, theme-derived colours,
> animation, a glyph channel, cross-tab state sync, and pluggable adapters
> (Claude Code + shells). See [Credits](#credits).

---

## States

| State | Default look | Meaning |
|-------|--------------|---------|
| `working` | green fill, scrolling | agent/command actively running |
| `waiting` | orange fill, flashing | needs you (a question, a permission, idle) |
| `done`    | green fill, solid     | finished cleanly |
| `error`   | red fill, flashing    | failed / non-zero exit |
| `idle`    | no fill               | nothing running |

Colours default to your **Zellij theme** (focus/non-focus frame colours for the
border; success/error colours for the fill) and are fully overridable. Where the
state and focus are drawn (`fill` / `border` / `glyph`) is configurable too — see
[Configuration](#configuration).

---

## 0 → Claude notifications

### 1. Get the plugin `.wasm`

Download `zellij-agent-tabs.wasm` from the [latest release](https://github.com/Zethtren/zellij-agent-tabs/releases/latest)
and drop it somewhere stable:

```sh
mkdir -p ~/.config/zellij/plugins
curl -fsSL -o ~/.config/zellij/plugins/zellij-agent-tabs.wasm \
  https://github.com/Zethtren/zellij-agent-tabs/releases/latest/download/zellij-agent-tabs.wasm
```

<details><summary>…or build it from source (Rust or Nix)</summary>

```sh
# Rust: needs the wasm target
rustup target add wasm32-wasip1
cargo build --release            # .cargo/config.toml pins the target
# → target/wasm32-wasip1/release/zellij-agent-tabs.wasm

# Nix (flake dev shell with the toolchain + zellij):
nix develop --command cargo build --release
```
</details>

### 2. Add the layout

Save this as `~/.config/zellij/layouts/agent-tabs.kdl`:

<details><summary>agent-tabs.kdl (left sidebar)</summary>

```kdl
// Run:  zellij -l agent-tabs
layout {
    pane split_direction="vertical" {
        pane size=26 borderless=true {
            plugin location="file:~/.config/zellij/plugins/zellij-agent-tabs.wasm" {
                tab_height 3
                // All optional — defaults derive from your Zellij theme:
                // state_style "fill"      // fill | border | glyph | both | all | none
                // focus_style "border"
                // state_glyph "●"
                // color_working "green"   // omit => theme
                // anim_working "scroll"   // scroll | flash | solid | none
                // anim_interval_ms 400
                // state_priority "error waiting working done idle"
            }
        }
        pane focus=true
    }
    pane size=1 borderless=true {
        plugin location="zellij:status-bar"
    }
}
```
</details>

Launch it (grant permissions on first run — focus the sidebar, press `y`):

```sh
zellij -l agent-tabs
```

At this point tabs already react to **command panes** (`zellij run -- cargo build`)
with no further setup. For typed shell commands, add a [shell adapter](#shell-adapters).

### 3. Install the Claude Code adapter

This is a small Claude Code **plugin** that ships hooks → it reports Claude's state
to the tab bar. Enabling it touches nothing in your `settings.json` by hand.

```sh
# clone this repo somewhere (it doubles as a Claude plugin marketplace)
git clone https://github.com/Zethtren/zellij-agent-tabs ~/src/zellij-agent-tabs
```

Then inside Claude Code:

```
/plugin marketplace add ~/src/zellij-agent-tabs
/plugin install zellij-agent-tabs@zellij-agent-tabs
```

### 4. Run Claude in a Zellij pane

Start `claude` inside a pane of your `agent-tabs` session. Its tab now goes:

- **green (working)** while Claude runs tools, labelled `claude <dir>: <command>`
- **orange (waiting)** when Claude stops / needs your input
- **red (error)** on a failed tool
- clears when Claude exits

> `Stop` maps to **waiting** by default (a finished turn is your cue to look). Prefer
> solid-green "done" instead? `export ZAT_STOP_STATE=done`.

> **Requirement:** Claude must run inside a Zellij pane (the adapter keys off
> `$ZELLIJ_PANE_ID`, which Zellij exports into every pane).

---

## Shell adapters

Optional — only needed to light up **commands you type at a prompt** (agents and
`zellij run` panes work without these). Each adapter defines an `agent-state`
helper and reports `working` before a command and `done`/`error` after, keyed by
`$ZELLIJ_PANE_ID`. Paste the block for your shell into its rc file.

> ⚠️ **Only the `fish` adapter has been tested so far.** The others are best-effort
> from each shell's documented hooks and may need tweaks — reports and PRs welcome.

<details><summary><b>fish</b> ✅ tested — <code>~/.config/fish/config.fish</code></summary>

```fish
if set -q ZELLIJ
    function agent-state
        test -z "$ZELLIJ_PANE_ID"; and return 0
        zellij pipe --name agent_state -- \
            (printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" "$argv[1]" shell (string join ' ' -- $argv[2..-1]))
    end
    function __zat_pre --on-event fish_preexec
        agent-state working $argv
    end
    function __zat_post --on-event fish_postexec
        set -l st $status
        test $st -eq 0; and agent-state done $argv; or agent-state error "exit $st: $argv"
    end
end
```
</details>

<details><summary><b>bash</b> (untested) — <code>~/.bashrc</code></summary>

```bash
agent-state() {
  [ -z "$ZELLIJ_PANE_ID" ] && return 0
  zellij pipe --name agent_state -- \
    "$(printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" "$1" shell "${*:2}")"
}
__zat_at_prompt=1; __zat_last=""
__zat_pre() {
  [ -n "$COMP_LINE" ] && return
  [ "$__zat_at_prompt" = 1 ] || return
  __zat_at_prompt=0; __zat_last="$BASH_COMMAND"
  agent-state working "$BASH_COMMAND"
}
__zat_post() {
  local st=$?; __zat_at_prompt=1
  [ -n "$__zat_last" ] || return
  [ $st -eq 0 ] && agent-state done "$__zat_last" || agent-state error "exit $st: $__zat_last"
  __zat_last=""
}
trap '__zat_pre' DEBUG
case ";${PROMPT_COMMAND};" in *";__zat_post;"*) ;; *) PROMPT_COMMAND="__zat_post;${PROMPT_COMMAND}";; esac
```
</details>

<details><summary><b>zsh</b> (untested) — <code>~/.zshrc</code></summary>

```zsh
agent-state() {
  [ -z "$ZELLIJ_PANE_ID" ] && return 0
  zellij pipe --name agent_state -- \
    "$(printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" "$1" shell "${*:2}")"
}
typeset -g __zat_last=""
__zat_pre() { __zat_last="$1"; agent-state working "$1"; }
__zat_post() {
  local st=$?
  [ -n "$__zat_last" ] || return
  [ $st -eq 0 ] && agent-state done "$__zat_last" || agent-state error "exit $st: $__zat_last"
  __zat_last=""
}
autoload -Uz add-zsh-hook
add-zsh-hook preexec __zat_pre
add-zsh-hook precmd  __zat_post
```
</details>

<details><summary><b>nushell</b> (untested) — <code>config.nu</code></summary>

```nu
let us = (char -i 31)
$env.config.hooks.pre_execution = ($env.config.hooks.pre_execution | default [] | append {||
  if 'ZELLIJ_PANE_ID' in $env {
    zellij pipe --name agent_state -- $"($env.ZELLIJ_PANE_ID)($us)working($us)shell($us)(commandline)"
  }
})
$env.config.hooks.pre_prompt = ($env.config.hooks.pre_prompt | default [] | append {||
  if 'ZELLIJ_PANE_ID' in $env {
    let s = (if $env.LAST_EXIT_CODE == 0 { "done" } else { "error" })
    zellij pipe --name agent_state -- $"($env.ZELLIJ_PANE_ID)($us)($s)($us)shell($us)"
  }
})
```
</details>

<details><summary><b>PowerShell</b> (untested; done/error only) — <code>$PROFILE</code></summary>

```powershell
# PowerShell has no native preexec; this reports done/error each prompt.
$us = [char]0x1f
function prompt {
  if ($env:ZELLIJ_PANE_ID) {
    $state = if ($?) { 'done' } else { 'error' }
    zellij pipe --name agent_state -- "$($env:ZELLIJ_PANE_ID)$us$state${us}shell$us" | Out-Null
  }
  "PS $($executionContext.SessionState.Path.CurrentLocation)$('>' * ($nestedPromptLevel + 1)) "
}
```
</details>

<details><summary><b>elvish</b> (untested) — <code>~/.config/elvish/rc.elv</code></summary>

```elvish
set edit:before-readline = [$@edit:before-readline {
  if (has-env ZELLIJ_PANE_ID) {
    zellij pipe --name agent_state -- $E:ZELLIJ_PANE_ID"\x1fdone\x1fshell\x1f"
  }
}]
set edit:after-readline = [$@edit:after-readline {|cmd|
  if (has-env ZELLIJ_PANE_ID) {
    zellij pipe --name agent_state -- $E:ZELLIJ_PANE_ID"\x1fworking\x1fshell\x1f"$cmd
  }
}]
```
</details>

<details><summary><b>xonsh</b> (untested) — <code>~/.xonshrc</code></summary>

```python
import os
US = "\x1f"
def _zat(state, cmd=""):
    pid = os.environ.get("ZELLIJ_PANE_ID")
    if pid:
        os.system(f"zellij pipe --name agent_state -- '{pid}{US}{state}{US}shell{US}{cmd}'")
@events.on_precommand
def _zat_pre(cmd, **_): _zat("working", cmd.strip())
@events.on_postcommand
def _zat_post(cmd, rtn=0, **_): _zat("done" if rtn == 0 else "error", cmd.strip())
```
</details>

> Shells without `preexec`/`precmd` (plain POSIX `sh`, `tcsh`) can't auto-report,
> but the `agent-state` helper still works if you call it manually.

**Nested shells** (e.g. `bash` launched from `fish`): `$ZELLIJ_PANE_ID` is inherited
by child processes, so whichever shell is reading input reports to the *same* tab.
Load the adapter in each shell's rc for per-command reporting inside nested shells.

---

## Configuration

Set these in the layout's `plugin { … }` block. Everything is optional; omitted
colours derive from your Zellij theme.

| Key | Default | Notes |
|-----|---------|-------|
| `tab_height` | `3` | rows per tab box (≥ 2) |
| `state_style` | `fill` | where state is drawn: `fill` `border` `glyph` `both` `all` `none` |
| `focus_style` | `border` | where focus is drawn (same tokens) |
| `state_glyph` | `●` | glyph used when the `glyph` channel is on |
| `color_working` / `color_waiting` / `color_done` / `color_error` | theme | any of: name / 256 / `#hex` / `rgb(r,g,b)` |
| `color_active_border` / `color_inactive_border` | theme | idle border colours |
| `anim_working` / `anim_waiting` / `anim_done` / `anim_error` | scroll / flash / solid / flash | `scroll` `flash` `solid` `none` |
| `anim_interval_ms` | `500` | animation tick |
| `state_priority` | `error waiting working done idle` | which state wins when a tab has several panes |

`state_style` and `focus_style` must not claim the same channel — if they do, the
plugin renders an obvious red config banner instead of tabs.

Full annotated example with every option at its default: [docs/config-example.kdl](docs/config-example.kdl).
Protocol (for writing your own adapter): [PROTOCOL.md](PROTOCOL.md).

---

## How it works

- The plugin is agent-agnostic: it consumes `agent_state` messages over `zellij pipe`
  and renders them. Anything that can run a shell command can be an adapter.
- Zellij runs one copy of the plugin **per tab**; a copy that loads late (new tab)
  broadcasts a sync request and peers resend their state, so tabs stay consistent.

---

## Roadmap

Tracked in [issues](https://github.com/Zethtren/zellij-agent-tabs/issues): more
agent adapters (OpenAI/Codex, Grok, Gemini, …), a Nix module (auto-install +
per-agent enable flags), right-side placement, square edges, tab gap / gapless
layouts, hide/shrink keybinds, per-state text & cursor colours, and a
`packages.default` flake output.

---

## Credits

**Forked from [cfal/zellij-vertical-tabs](https://github.com/cfal/zellij-vertical-tabs)**
by Alex Lau (MIT) — the vertical tab bar this project is built on. Huge thanks; go
star the original.

Built on [Zellij](https://zellij.dev) and [zellij-tile](https://crates.io/crates/zellij-tile).

## License

MIT — see [LICENSE](LICENSE). Original portions © 2026 Alex Lau; modifications
© 2026 Zethtren. The upstream MIT license text is retained verbatim in
[LICENSE](LICENSE).
