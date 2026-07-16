{
  description = "zellij-agent-tabs — agent-aware vertical tab bar plugin for Zellij (fork of cfal/zellij-vertical-tabs)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      perSystem = { pkgs, system, ... }:
        let
          fenix = inputs.fenix.packages.${system};
          # Rust toolchain with the wasm32-wasip1 target Zellij plugins compile to.
          toolchain = fenix.combine [
            fenix.stable.rustc
            fenix.stable.cargo
            fenix.stable.clippy
            fenix.stable.rustfmt
            fenix.targets.wasm32-wasip1.stable.rust-std
          ];
        in
        {
          devShells.default = pkgs.mkShell {
            packages = [ toolchain pkgs.zellij ];
            shellHook = ''
              echo "zellij-agent-tabs dev shell — build with:"
              echo "  cargo build --release --target wasm32-wasip1"
            '';
          };
        };
    };
}
