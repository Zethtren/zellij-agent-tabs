# home-manager module for zellij-agent-tabs.
# Consumers: add this flake as an input and import
#   inputs.zellij-agent-tabs.homeManagerModules.default
# then set `programs.zellij-agent-tabs.enable = true;` (and optionally enableClaude).
self:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.zellij-agent-tabs;
  pkg = self.packages.${pkgs.system}.default;
  wasm = "${pkg}/share/zellij-agent-tabs/zellij-agent-tabs.wasm";
  emit = "${pkg}/share/zellij-agent-tabs/claude-emit.sh";
in
{
  options.programs.zellij-agent-tabs = {
    enable = lib.mkEnableOption "zellij-agent-tabs vertical tab bar plugin";

    installZellij = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Install the zellij package via home.packages.";
    };

    installLayout = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Symlink an 'agent-tabs' layout into ~/.config/zellij/layouts/ (run: zellij -l agent-tabs).";
    };

    enableClaude = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Wire the Claude Code adapter: on activation, merge state-reporting hooks
        into ~/.claude/settings.json (preserving your other keys). Claude then
        reports working/waiting/done/error to the tab bar.
      '';
    };

    stopState = lib.mkOption {
      type = lib.types.enum [ "waiting" "done" ];
      default = "waiting";
      description = "State reported when Claude finishes a turn (the Stop hook).";
    };

    enableGrok = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "(stub) Grok adapter — not implemented yet (issue #9).";
    };

    enableOpenAI = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "(stub) OpenAI/Codex adapter — not implemented yet (issue #9).";
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    {
      home.packages = lib.optional cfg.installZellij pkgs.zellij;
      home.file.".config/zellij/plugins/zellij-agent-tabs.wasm".source = wasm;
    }

    (lib.mkIf cfg.installLayout {
      home.file.".config/zellij/layouts/agent-tabs.kdl".source = ./agent-tabs.kdl;
    })

    (lib.mkIf (cfg.enableGrok || cfg.enableOpenAI) {
      warnings = [
        "programs.zellij-agent-tabs: enableGrok/enableOpenAI are not implemented yet (issue #9) — ignored."
      ];
    })

    (lib.mkIf cfg.enableClaude {
      # jq is available (nix), so we can safely merge into the Claude-owned file.
      home.activation.zellijAgentTabsClaude = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        settings="$HOME/.claude/settings.json"
        run mkdir -p "$HOME/.claude"
        [ -f "$settings" ] || echo '{}' > "$settings"
        tmp="$(mktemp)"
        ${pkgs.jq}/bin/jq \
          --arg emit "${emit}" \
          --arg stop "${cfg.stopState}" '
            .hooks = (.hooks // {})
            | .hooks.UserPromptSubmit    = [ { hooks: [ { type: "command", command: ($emit + " working") } ] } ]
            | .hooks.PreToolUse          = [ { hooks: [ { type: "command", command: ($emit + " working") } ] } ]
            | .hooks.Notification        = [ { hooks: [ { type: "command", command: ($emit + " waiting") } ] } ]
            | .hooks.Stop                = [ { hooks: [ { type: "command", command: ($emit + " " + $stop) } ] } ]
            | .hooks.PostToolUseFailure  = [ { hooks: [ { type: "command", command: ($emit + " error") } ] } ]
            | .hooks.StopFailure         = [ { hooks: [ { type: "command", command: ($emit + " error") } ] } ]
            | .hooks.SessionEnd          = [ { hooks: [ { type: "command", command: ($emit + " idle") } ] } ]
          ' "$settings" > "$tmp" && run mv "$tmp" "$settings"
      '';
    })
  ]);
}
