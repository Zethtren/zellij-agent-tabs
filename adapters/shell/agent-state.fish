# zellij-agent-tabs — shell adapter (fish). OPTIONAL: only needed to track
# commands typed at an interactive prompt. Agents and command *panes* are
# handled without any shell adapter.
#
# Source this in an interactive fish inside Zellij:
#   source /path/to/agent-state.fish
#
# Provides `agent-state <state> [label...]` to report the current pane's state,
# and (optionally) auto-reports running commands: a command becomes `working`
# while it runs, then `done` (exit 0) or `error` (non-zero) when it finishes.

function agent-state --description 'Report agent state for the current zellij pane'
    test -z "$ZELLIJ_PANE_ID"; and return 0
    set -l state $argv[1]
    set -l label (string join ' ' -- $argv[2..-1])
    # Protocol: "<pane_id>\x1f<state>\x1f<agent>\x1f<label>"
    zellij pipe --name agent_state -- \
        (printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" "$state" shell "$label")
end

# ----- optional auto-reporting of interactive commands -----
# Enable by leaving these defined; remove the two functions to disable.

function __agent_state_preexec --on-event fish_preexec
    test -z "$ZELLIJ_PANE_ID"; and return 0
    agent-state working "$argv"
end

function __agent_state_postexec --on-event fish_postexec
    set -l st $status # MUST be first line
    test -z "$ZELLIJ_PANE_ID"; and return 0
    if test $st -eq 0
        agent-state done "$argv"
    else
        agent-state error "exit $st: $argv"
    end
end
