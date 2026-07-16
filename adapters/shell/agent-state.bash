# zellij-agent-tabs — shell adapter (bash). OPTIONAL: only needed to track
# commands typed at an interactive prompt. Agents and command *panes* are
# handled without any shell adapter.
#
# Enable by sourcing from .bashrc:
#   source /path/to/agent-state.bash
#
# Bash has no native preexec, so this emulates it with a DEBUG trap +
# PROMPT_COMMAND. For heavy customisation, the bash-preexec project is more
# robust; this is a self-contained best-effort.

agent-state() {
    [ -z "$ZELLIJ_PANE_ID" ] && return 0
    local state="$1"; shift
    local label="$*"
    # Protocol: "<pane_id>\x1f<state>\x1f<agent>\x1f<label>"
    zellij pipe --name agent_state -- \
        "$(printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" "$state" shell "$label")"
}

__agent_state_at_prompt=1
__agent_state_last=""

__agent_state_preexec() {
    [ -n "$COMP_LINE" ] && return          # skip completion
    [ "$__agent_state_at_prompt" = 1 ] || return  # only the user's command
    __agent_state_at_prompt=0
    __agent_state_last="$BASH_COMMAND"
    [ -n "$ZELLIJ_PANE_ID" ] && agent-state working "$BASH_COMMAND"
}

__agent_state_precmd() {
    local st=$?                            # MUST be first line
    __agent_state_at_prompt=1
    [ -z "$ZELLIJ_PANE_ID" ] && return
    if [ -n "$__agent_state_last" ]; then
        if [ $st -eq 0 ]; then
            agent-state done "$__agent_state_last"
        else
            agent-state error "exit $st: $__agent_state_last"
        fi
        __agent_state_last=""
    fi
}

trap '__agent_state_preexec' DEBUG
case ";${PROMPT_COMMAND};" in
    *";__agent_state_precmd;"*) ;;
    *) PROMPT_COMMAND="__agent_state_precmd;${PROMPT_COMMAND}" ;;
esac
