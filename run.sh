#!/usr/bin/env bash
# Launch a Zellij demo of zellij-agent-tabs, building the plugin if needed.
# Uses the repo's `nix develop` shell, which provides both the Rust toolchain
# and Zellij — so this works even though Zellij isn't installed system-wide yet.
#
#   ./run.sh
#
# On first launch: focus the LEFT tab-bar pane and press `y` to grant permissions.
set -euo pipefail
cd "$(dirname "$0")"

exec nix develop --command bash -c '
  set -e
  WASM=target/wasm32-wasip1/release/zellij-agent-tabs.wasm
  if [ ! -f "$WASM" ]; then
    echo "Building plugin (first run, ~30s)…"
    cargo build --release
  fi
  echo
  echo "==================================================================="
  echo " zellij-agent-tabs demo"
  echo " 1. Focus the LEFT pane and press  y  to grant permissions (once)."
  echo " 2. In the right pane, try the NATIVE path (no adapter needed):"
  echo "        zellij run -- sleep 3      # working → done (green)"
  echo "        zellij run -- false        # error (red flash)"
  echo " 3. For typed-command / manual tests, source the shell adapter:"
  echo "        source adapters/shell/agent-state.fish   # (or .bash/.zsh)"
  echo "        agent-state working \"cargo build\""
  echo "        agent-state waiting \"needs input\""
  echo "        agent-state error \"boom\""
  echo "        agent-state done \"ok\""
  echo "        agent-state idle"
  echo " 4. Open more tabs with Alt-t to see multi-tab aggregation."
  echo "==================================================================="
  echo
  exec zellij --layout examples/agent-tabs-left.kdl
'
