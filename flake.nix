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
      # x86_64-darwin dropped: nixpkgs-unstable (26.11+) no longer supports it.
      systems = [ "aarch64-darwin" "x86_64-linux" "aarch64-linux" ];

      # home-manager module (see nix/hm-module.nix); consumers import this.
      flake.homeManagerModules.default = import ./nix/hm-module.nix inputs.self;

      perSystem = { pkgs, system, ... }:
        let
          fenix = inputs.fenix.packages.${system};
          toolchain = fenix.combine [
            fenix.stable.rustc
            fenix.stable.cargo
            fenix.stable.clippy
            fenix.stable.rustfmt
            fenix.targets.wasm32-wasip1.stable.rust-std
          ];
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };

          wasm = rustPlatform.buildRustPackage {
            pname = "zellij-agent-tabs";
            version = "0.0.1";
            # Only files that affect the wasm build, with Cargo.toml's version pinned
            # to a constant. This keeps the store path STABLE across version-only
            # release bumps and doc/CI edits, so FlakeHub Cache hits stay valid across
            # releases (the real version lives in the git tag).
            src =
              let
                lib = pkgs.lib;
                buildFiles = lib.fileset.toSource {
                  root = ./.;
                  fileset = lib.fileset.unions [
                    ./Cargo.lock
                    ./src
                    ./activity
                    ./.cargo
                    ./claude-plugin/scripts/emit.sh
                  ];
                };
                cargoToml = pkgs.writeText "Cargo.toml" (
                  lib.concatStringsSep "\n" (
                    map (l: if lib.hasPrefix "version = " l then ''version = "0.0.0"'' else l)
                      (lib.splitString "\n" (builtins.readFile ./Cargo.toml))
                  )
                );
              in
              pkgs.runCommand "zellij-agent-tabs-src" { } ''
                cp -r ${buildFiles}/. "$out"
                chmod -R +w "$out"
                cp ${cargoToml} "$out/Cargo.toml"
              '';

            cargoLock = {
              lockFile = ./Cargo.lock;
              # zellij-tile (and siblings) come from a git branch; pin the source hash.
              outputHashes = {
                "zellij-tile-0.44.0" = "sha256-Dk7UUYF1j8gTaDpAbNECe10DjkqMgi3JXm+iK62JmCs=";
                "zellij-utils-0.44.0" = "sha256-Dk7UUYF1j8gTaDpAbNECe10DjkqMgi3JXm+iK62JmCs=";
              };
            };

            # A transitive dep (zellij-utils -> openssl-sys) probes for OpenSSL on the
            # host during its build script; provide it so the wasm build can proceed.
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];

            # buildRustPackage would otherwise build for the host; force the wasm target.
            doCheck = false;
            buildPhase = ''
              runHook preBuild
              cargo build --release --target wasm32-wasip1 --offline
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p "$out/share/zellij-agent-tabs"
              cp target/wasm32-wasip1/release/zellij-agent-tabs.wasm \
                 "$out/share/zellij-agent-tabs/zellij-agent-tabs.wasm"
              cp claude-plugin/scripts/emit.sh "$out/share/zellij-agent-tabs/claude-emit.sh"
              chmod +x "$out/share/zellij-agent-tabs/claude-emit.sh"
              runHook postInstall
            '';

            meta = {
              description = "Agent-aware vertical tab bar plugin for Zellij";
              license = pkgs.lib.licenses.mit;
            };
          };
        in
        {
          packages.default = wasm;
          packages.zellij-agent-tabs = wasm;

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
