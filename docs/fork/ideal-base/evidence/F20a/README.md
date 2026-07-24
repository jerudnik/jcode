# F20a evidence — nix-native + update-inert

**Node:** F20a (implement / deterministic / W3), depends on F16 + F19. Issue #28.

## Gap

`should_auto_update()` gated only on `is_release_build()` and not-in-a-git-repo.
A nix-installed jcode is a release build living at a read-only `/nix/store/`
path (not a git repo), so it would attempt GitHub self-update: download a
release tarball into `~/.jcode/builds/` and move channel symlinks. Meanwhile
`is_externally_managed()` (which the launcher/paths layer already respects) only
checked the `JCODE_NIX_MANAGED` env var, which **is set nowhere** in the package,
flake, or home-manager module. So all the nix-managed protection was dormant and
a nix binary behaved exactly like a `curl | sh` install.

## Change (store-residence detection, no wrapper)

- **`crates/jcode-build-support/src/paths.rs`**: `is_externally_managed()` now
  returns true when the running binary resolves inside `/nix/store/` (via the new
  pure, unit-tested `running_from_nix_store`, which canonicalizes so a
  profile/`~/.local/bin` symlink into the store still counts), OR when
  `JCODE_NIX_MANAGED` is set (kept as an explicit override). A packaged jcode is
  the real ELF at `/nix/store/.../bin/jcode` (no wrapper), so it self-declares
  managed purely by where it lives — inherited automatically by home-manager and
  `nix profile` installs with **zero nix-file changes**.
- **`crates/jcode-app-core/src/update.rs`**: `should_auto_update()` returns false
  for externally-managed installs.
- **`src/cli/hot_exec.rs`**: `jcode update` on a managed binary prints honest
  package-manager guidance (home-manager rebuild / `nix profile upgrade` / flake
  update) and performs no GitHub download.

Self-dev builds (`~/.jcode/builds/`) and dev-shell cargo builds (`target/`) are
NOT store-resident, so `jcode selfdev` and the hot-reload loop are unaffected.

## Acceptance gates (both proven)

1. **Managed binary declines self-update without network** — `update-decline.txt`:
   the store binary's `jcode update` prints the nix guidance and exits 0 with no
   download. Unit tests (`running_from_nix_store_*`) prove the classification.
2. **Reports externally-managed** — `doctor-origin.txt`: the store binary's own
   doctor reports `origin=nix` for the same `/nix/store/` residence that drives
   `is_externally_managed()`.

## Evidence artifacts

- `update-decline.txt` — `jcode update` from the store binary (empty CWD,
  throwaway `JCODE_HOME`): nix guidance, no download, exit 0.
- `doctor-origin.txt` — doctor classifies the store binary `origin=nix`.
- Unit tests: `cargo test -p jcode-build-support -- running_from_nix_store` → 3 passed.

## Reproduce (Mac, aarch64-darwin)

```
nix build .#packages.aarch64-darwin.jcode --accept-flake-config -o result-f20a
PKG=$(readlink -f result-f20a)
JCODE_HOME=$(mktemp -d) "$PKG/bin/jcode" update   # -> "managed by nix; self-update is disabled"
```
