# Contributing / maintainer notes

## Build & dev

```sh
nix develop --command cargo build --release   # → target/wasm32-wasip1/release/zellij-agent-tabs.wasm
nix build .#default                           # the packaged wasm (same as consumers get)
```

`.cargo/config.toml` pins `target = wasm32-wasip1`, so plain `cargo build --release`
targets wasm.

## ⚠️ Do NOT change these without understanding why

### The flake `src` is filtered and the version is pinned — on purpose
`packages.default` in `flake.nix` does **not** use `src = ./.`. Instead it:
- filters `src` to build inputs only (`Cargo.toml`, `Cargo.lock`, `src/`, `activity/`,
  `.cargo/`, the Claude emit script), and
- rewrites `Cargo.toml`'s `version` to a constant **inside the derivation**.

This makes the wasm store path depend only on the **code**, not on the version or on
doc/CI edits. It exists so **FlakeHub Cache hits survive releases**: the auto-version
bump (below) changes `Cargo.toml` every release, and if that (or a `.github/` edit)
changed the store path, the macOS runner would cache one path while consumers built
another → every `darwin-rebuild` would recompile the wasm. If you "simplify" this back
to `src = ./.`, you reintroduce that cache miss. Verify with:
```sh
nix eval --raw .#packages.aarch64-darwin.default.outPath   # bump the version, re-run: path must be identical
```

### The dependency hash
`outputHashes` in `flake.nix` pins the `zellij-tile`/`zellij-utils` git source. It only
changes if you `cargo update` that git dependency (new rev in `Cargo.lock`). When it
does, `nix build` fails with the correct `got:` hash — paste it into both entries.

## CI / releases (`.github/workflows/build.yml`)

- **Build runs on `ubuntu-latest` AND `macos-latest`** so both the `x86_64-linux` and
  `aarch64-darwin` derivations are pushed to FlakeHub Cache. Without the macOS job,
  Apple Silicon users always recompile.
- **Build + release run ONLY on `push` to master** (never on PRs). The build job has
  `id-token: write` + FlakeHub Cache *write*; running it on a fork PR would let untrusted
  code poison the cache the release trusts — a supply-chain injection path. Keep it
  master-only, and turn on GitHub's "require approval for outside collaborators" for
  Actions.
- **Versioning is automatic**: a push to master patch-bumps from the latest `v*` tag
  (updates `Cargo.toml` + creates the tag); push `vX.Y.Z` yourself for minor/major.
  The bump commit + tag use `GITHUB_TOKEN` (doesn't retrigger CI) plus `[skip ci]`.
- **Release** creates the GitHub release (via `gh`) with the wasm, then publishes the
  flake to FlakeHub.

## Adapters

- `claude-plugin/` — Claude Code adapter (a Claude plugin; hooks → `emit.sh` → `zellij pipe`).
- `adapters/shell/` — shell adapters (only `fish` is tested).
- The plugin itself is agent-agnostic; adapters just emit the [protocol](PROTOCOL.md).
