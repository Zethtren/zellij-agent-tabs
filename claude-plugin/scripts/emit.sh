#!/usr/bin/env bash
# zellij-agent-tabs — Claude Code adapter.
# Called by the plugin's hooks with a state arg; reads the hook's JSON on stdin
# and reports state for the current Zellij pane via `zellij pipe`.
#
# Usage (from hooks.json): emit.sh <working|waiting|done|error|idle|stop>
#
# The special arg "stop" resolves to $ZAT_STOP_STATE (default "waiting"), so a
# finished-but-idle turn shows as a priority notification unless you override it:
#   export ZAT_STOP_STATE=done
set -u

state="${1:-idle}"
if [ "$state" = "stop" ]; then
    state="${ZAT_STOP_STATE:-waiting}"
fi

# Only meaningful inside a Zellij pane, and only if zellij is on PATH.
[ -z "${ZELLIJ_PANE_ID:-}" ] && exit 0
command -v zellij >/dev/null 2>&1 || exit 0

json="$(cat 2>/dev/null || true)"

label="claude"
if command -v jq >/dev/null 2>&1 && [ -n "$json" ]; then
    cwd="$(printf '%s' "$json" | jq -r '.cwd // empty' 2>/dev/null)"
    base="$(basename "$cwd" 2>/dev/null)"
    tool="$(printf '%s' "$json" | jq -r '.tool_name // empty' 2>/dev/null)"
    cmd="$(printf '%s' "$json" | jq -r '.tool_input.command // empty' 2>/dev/null)"
    detail="$cmd"
    [ -z "$detail" ] && detail="$tool"
    if [ -n "$base" ] && [ -n "$detail" ]; then
        label="claude $base: $detail"
    elif [ -n "$base" ]; then
        label="claude $base"
    fi
fi

# Protocol: "<pane_id>\x1f<state>\x1f<agent>\x1f<label>"
payload="$(printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" "$state" claude "$label")"
zellij pipe --name agent_state -- "$payload" >/dev/null 2>&1 || true
exit 0
