# Nix-backed selfdev build and reload proposal

## Summary

Explore letting `selfdev build-reload` build a jcode binary from a configurable Nix flake source, then hot-reload the current session into that binary.

The long-term goal is a config option that chooses where hot-reload binaries come from:

- Current self-dev worktree and coordinated Cargo build.
- The repository the session is editing.
- The fork's GitHub flake on `main`.
- The reusable packaging rail on `distro/nix`.
- Exact upstream jcode, when useful for comparison or recovery.

This is mostly a hygiene and reproducibility proposal. It should make 4nix and jcode agree about where the active binary came from.

## Motivation

Today, selfdev reload is optimized for editing jcode itself. That is good for dogfooding, but it leaves a few gaps:

- A session may be running a local binary that is not the same as the flake-pinned binary in 4nix.
- It is easy to forget whether a reload came from Cargo, a local checkout, or an installed package.
- 4nix can pin a jcode flake, but selfdev reload does not yet treat that flake as a first-class source.
- Recovery and comparison workflows would benefit from one command that reloads into a known-good fork or upstream build.

## Proposed source modes

```toml
[selfdev.reload]
source = "selfdev-build"
# source = "editing-repo-flake"
# source = "github-fork-main"
# source = "github-distro-nix"
# source = "upstream"
# source = "flake-url"
# flake_url = "github:jerudnik/jcode/main"
```

### `selfdev-build`

Current behavior. Use the coordinated selfdev build queue and reload into the produced binary.

Best for active jcode development.

### `editing-repo-flake`

If the session is editing a repository with a jcode-compatible flake output, build from that repo:

```sh
nix build <repo>#packages.${system}.jcode
```

Best for dogfooding a local branch through the same package surface consumed by 4nix.

### `github-fork-main`

Build from:

```sh
nix build github:jerudnik/jcode/main#packages.${system}.jcode
```

Best for reloading into the stable custom fork binary that 4nix should normally consume.

### `github-distro-nix`

Build from:

```sh
nix build github:jerudnik/jcode/distro/nix#packages.${system}.jcode
```

Best for packaging-only validation, cache checks, and isolating whether a breakage is caused by fork behavior or packaging.

### `upstream`

Build exact upstream when upstream has a flake, or build the fork's `vendor/upstream` branch if it carries only upstream content and packaging is not needed.

Open design question: exact upstream currently does not necessarily expose jcode's flake packaging. If upstream has no flake, this mode may need either:

1. A Cargo fallback using upstream source.
2. A generated temporary flake wrapper.
3. A comparison-only mode that refuses reload and explains why no flake package exists.

### `flake-url`

Build any explicit flake URL. This is the escape hatch for testing PRs, temporary branches, local paths, or release candidates.

## Candidate command behavior

```sh
selfdev build-reload source=github-fork-main
selfdev build-reload source=editing-repo-flake
selfdev build-reload source=flake-url flake_url=github:jerudnik/jcode/main
```

The tool should:

1. Resolve the source to a flake URL or local build plan.
2. Build in the background with parseable progress.
3. Record provenance before reload.
4. Verify the result contains a runnable `bin/jcode`.
5. Reload with existing selfdev reload handoff.
6. Show the post-reload binary provenance in status/debug output.

## Provenance metadata

Every reload candidate should produce a sidecar like:

```json
{
  "source_mode": "github-fork-main",
  "flake_url": "github:jerudnik/jcode/main",
  "locked_rev": "14239a278c27bc96f425d3e750ce992415b770e2",
  "nar_hash": "sha256-KboVC8SupreS0j/4VNwqKkmFhfQ0OV7S6oTIBLUtFlk=",
  "store_path": "/nix/store/...-jcode-...",
  "binary_path": "/nix/store/.../bin/jcode",
  "built_at": "2026-07-01T22:43:00Z"
}
```

This should be visible from:

- `selfdev status`
- `selfdev find-config`
- debug socket runtime identity commands
- reload recovery messages when a session resumes after reload

## 4nix hygiene angle

4nix should own host policy and pinning. Jcode should own reload mechanics.

A clean split could be:

- 4nix pins `inputs.jcode.url = "github:jerudnik/jcode/main"`.
- 4nix optionally writes a jcode config value like `selfdev.reload.source = "github-fork-main"` or `flake-url` with the pinned URL.
- Jcode resolves and builds that source without embedding 4nix-specific policy.
- Jcode records exactly what was built and reloaded.

This makes it possible to ask: "Is my running jcode binary the one my host config would install?"

## Safety rules

- Never reload into an unverified build output.
- Never silently switch from local source to remote source.
- Treat network builds as live operations and surface the source URL before reload.
- Preserve existing selfdev reload interruption/recovery behavior.
- Refuse ambiguous upstream mode until exact semantics are clear.
- Keep branch rails intact: `main` for normal fork behavior, `distro/nix` for packaging-only checks, `vendor/upstream` for exact upstream mirror.

## Validation plan

1. Unit-test source resolution from config and command input.
2. Add a fake flake fixture that produces a tiny `bin/jcode` shim for fast tests.
3. Test sidecar provenance parsing and runtime identity display.
4. Test build failure, missing binary, and non-executable binary cases.
5. Test reload handoff remains prompt and recoverable.
6. Run real smoke commands:

```sh
nix build github:jerudnik/jcode/main#packages.$(nix eval --impure --raw --expr builtins.currentSystem).jcode
nix build github:jerudnik/jcode/distro/nix#packages.$(nix eval --impure --raw --expr builtins.currentSystem).jcode
```

## Open questions

- Should this be part of `selfdev build-reload`, or a new action like `selfdev reload-from`?
- Should config be global, per-worktree, or overridable per session?
- Should 4nix write an explicit flake URL, or should jcode know symbolic source names like `github-fork-main`?
- How should upstream mode work while upstream lacks the fork's reusable Nix packaging?
- Should Nix builds use Cachix by default and expose cache-hit/miss information?
- Should reload provenance be included in the prompt context so agents know what binary they are running?

## Initial recommendation

Start with `flake-url` and `github-fork-main` only. They are enough to prove the architecture, align with 4nix, and avoid upstream ambiguity.

Then add `editing-repo-flake` for local dogfooding. Add `github-distro-nix` for packaging diagnosis. Delay `upstream` until exact semantics are settled.
