# zellij-agent-tabs — shell adapter (zsh). OPTIONAL: only needed to track
# commands typed at an interactive prompt. Agents and command *panes* are
# handled without any shell adapter.
#
# Enable by sourcing from .zshrc:
#   source /path/to/agent-state.zsh

agent-state() {
    [ -z "$ZELLIJ_PANE_ID" ] && return 0
    local state="$1"; shift
    local label="$*"
    # Protocol: "<pane_id>\x1f<state>\x1f<agent>\x1f<label>"
    zellij pipe --name agent_state -- \
        "$(printf '%s\x1f%s\x1f%s\x1f%s' "$ZELLIJ_PANE_ID" "$state" shell "$label")"
}

# ----- auto-reporting of interactive commands -----
typeset -g __agent_state_last=""

__agent_state_preexec() {
    [ -z "$ZELLIJ_PANE_ID" ] && return
    __agent_state_last="$1"
    agent-state working "$1"
}

__agent_state_precmd() {
    local st=$?              # MUST be first line
    [ -z "$ZELLIJ_PANE_ID" ] && return
    [ -z "$__agent_state_last" ] && return
    if [ $st -eq 0 ]; then
        agent-state done "$__agent_state_last"
    else
        agent-state error "exit $st: $__agent_state_last"
    fi
    __agent_state_last=""
}

autoload -Uz add-zsh-hook 2>/dev/null
if (( $+functions[add-zsh-hook] )); then
    add-zsh-hook preexec __agent_state_preexec
    add-zsh-hook precmd  __agent_state_precmd
else
    preexec_functions+=(__agent_state_preexec)
    precmd_functions+=(__agent_state_precmd)
fi
