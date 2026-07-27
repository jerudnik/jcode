# Releasing Jcode

Jcode is an independent hard fork. `main` is the sole product authority and the
immutable `fork-point` tag remains the quality-gate anchor. Repository-owned
end-user distribution is exclusively Nix-based: `flake.nix` defines the package
and the public `jerudnik-jcode` Cachix cache serves trusted build outputs.

GitHub releases are metadata-only. They contain release notes and source/tag
metadata, never executable archives, checksums, installers, or package-manager
payloads.

## Release prerequisites

- The release commit is on `main` and the worktree is clean.
- The version in the root `Cargo.toml` is final.
- `Cargo.lock` and `flake.lock` are committed and coherent.
- User-facing changes are represented in the versioned changelog input expected
  by `scripts/generate_release_notes.sh`, when applicable.
- Relevant Rust, policy, workflow, documentation, and Nix gates pass.
- The public Cachix cache and its signing configuration are healthy.

## Prepare and validate

1. Create a topic branch from `main` for version, changelog, and release-document
   changes.
2. Run the narrow checks while iterating, then the maintained final gates from
   `docs/agent-workflows.md`.
3. At minimum, validate:

   ```bash
   nix develop -c python3 tests/test_nix_distribution_policy.py
   nix flake check --accept-flake-config --all-systems
   nix build .#packages.$(nix eval --raw --impure --expr builtins.currentSystem).jcode
   ./result/bin/jcode --version
   ```

4. Merge the release preparation through the normal review path. Do not tag a
   topic branch or an unmerged commit.

## Tag and publish

From an up-to-date, clean `main`:

```bash
git fetch --all --prune
git switch main
git pull --ff-only
git tag -s vX.Y.Z -m "Jcode vX.Y.Z"
git push <fork-remote> vX.Y.Z
```

Discover the configured fork remote with `git remote -v`; do not assume it is
named `origin`.

A `v*` tag triggers `.github/workflows/nix.yml`, which evaluates, builds,
smoke-tests, and pushes the tagged flake outputs to Cachix. Only after every
required Nix/Cachix job succeeds does it call `.github/workflows/release.yml`
to verify that the tagged commit belongs to authoritative `main`, render notes,
and publish a metadata-only GitHub release. Publication fails if assets are
attached.

The flake and Cachix are the only repository-owned binary authority. Retired
channels must not be recreated, including shell or PowerShell installers,
Homebrew, AUR, GitHub executable assets, checksum manifests for those assets,
the native iOS application, signed app-store/TestFlight delivery, or Cargo
registry publication.

## Verify after tagging

- Confirm the Nix workflow succeeded for every maintained release system.
- Confirm Cachix serves the tagged package closure.
- Run the tagged package through Nix and check its version:

  ```bash
  nix run github:jerudnik/jcode/vX.Y.Z --accept-flake-config -- version
  ```

- Confirm the GitHub release is public, contains the expected notes, and has zero
  assets.
- Confirm `main` and the immutable `fork-point` tag were not moved or rewritten.

## Updating an installation

Updates are owned by the consumer's Nix configuration:

```bash
nix profile upgrade jcode
# or update the pinned flake input, then rebuild the owning configuration
nix flake update jcode
```

`jcode update` and `/update` only print Nix guidance. They never fetch, build,
replace, or mutate the running executable. Explicit self-development rebuilds
remain developer workflows and are not end-user distribution channels.
